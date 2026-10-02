//! Transition validation over persisted records; restoring does not replay events.
use chaos_ipc::background_tasks::{BackgroundTask, TaskSource, TaskState, WakePolicy};
use state_machines::state_machine;

state_machine! {
    name: TaskExecution,
    dynamic: true,
    initial: Submitting,
    states: [
        superstate Live { state Submitting, state Running, state InputRequired },
        Succeeded, Failed, Cancelled, Lost, SubmissionUnknown,
    ],
    events {
        run { transition: { from: Live, to: Running } }
        input { transition: { from: Live, to: InputRequired } }
        succeed { transition: { from: Live, to: Succeeded } }
        fail { transition: { from: Live, to: Failed } }
        cancel { transition: { from: Live, to: Cancelled } }
        lost { transition: { from: Live, to: Lost } }
        unknown { transition: { from: Live, to: SubmissionUnknown } }
        withdraw_recovery {
            payload: bool,
            guards: [session_local_undelivered],
            transition: { from: Succeeded, to: Running }
        }
        revoke_recovery {
            payload: bool,
            guards: [session_local_undelivered],
            transition: { from: Succeeded, to: Cancelled }
        }
    }
}

impl<C, S> TaskExecution<C, S> {
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "state-machines guards receive event payloads by reference"
    )]
    fn session_local_undelivered(&self, _ctx: &C, permitted: &bool) -> bool {
        *permitted
    }
}

pub(super) fn transition(task: &mut BackgroundTask, next: TaskState) -> bool {
    if task.state == next {
        return true;
    }
    let state = match task.state {
        TaskState::Submitting => TaskExecutionState::Submitting,
        TaskState::Running => TaskExecutionState::Running,
        TaskState::InputRequired => TaskExecutionState::InputRequired,
        TaskState::Succeeded => TaskExecutionState::Succeeded,
        TaskState::Failed => TaskExecutionState::Failed,
        TaskState::Cancelled => TaskExecutionState::Cancelled,
        TaskState::Lost => TaskExecutionState::Lost,
        TaskState::SubmissionUnknown => TaskExecutionState::SubmissionUnknown,
    };
    let recovery = task.source == Some(TaskSource::MachineRecovery);
    let event = match next {
        TaskState::Running if task.state == TaskState::Succeeded => {
            TaskExecutionEvent::WithdrawRecovery(recovery && !task.delivered)
        }
        TaskState::Cancelled if task.state == TaskState::Succeeded => {
            TaskExecutionEvent::RevokeRecovery(recovery)
        }
        TaskState::Running => TaskExecutionEvent::Run,
        TaskState::InputRequired => TaskExecutionEvent::Input,
        TaskState::Succeeded => TaskExecutionEvent::Succeed,
        TaskState::Failed => TaskExecutionEvent::Fail,
        TaskState::Cancelled => TaskExecutionEvent::Cancel,
        TaskState::Lost => TaskExecutionEvent::Lost,
        TaskState::SubmissionUnknown => TaskExecutionEvent::Unknown,
        TaskState::Submitting => return false,
    };
    if DynamicTaskExecution::new_init_state((), state)
        .handle(event)
        .is_err()
    {
        return false;
    }
    task.state = next;
    true
}

state_machine! {
    name: WakeAdmission,
    dynamic: true,
    initial: Enabled,
    states: [Enabled, Interrupted, Closed],
    events {
        enable {
            transition: { from: Enabled, to: Enabled }
            transition: { from: Interrupted, to: Enabled }
        }
        interrupt {
            transition: { from: Enabled, to: Interrupted }
            transition: { from: Interrupted, to: Interrupted }
        }
        close {
            transition: { from: Enabled, to: Closed }
            transition: { from: Interrupted, to: Closed }
            transition: { from: Closed, to: Closed }
        }
    }
}

pub(super) fn set_policy(policy: &mut WakePolicy, next: WakePolicy) -> bool {
    let state = match policy {
        WakePolicy::Enabled => WakeAdmissionState::Enabled,
        WakePolicy::Interrupted => WakeAdmissionState::Interrupted,
        WakePolicy::Closed => WakeAdmissionState::Closed,
    };
    let event = match next {
        WakePolicy::Enabled => WakeAdmissionEvent::Enable,
        WakePolicy::Interrupted => WakeAdmissionEvent::Interrupt,
        WakePolicy::Closed => WakeAdmissionEvent::Close,
    };
    if DynamicWakeAdmission::new_init_state((), state)
        .handle(event)
        .is_err()
    {
        return false;
    }
    *policy = next;
    true
}
