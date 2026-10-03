use super::SynopsisJobState as ResultState;
use state_machines::state_machine;

state_machine! {
    name: SynopsisJob,
    dynamic: true,
    initial: Pending,
    states: [Pending, Running, Completed, Failed, Cancelled],
    events {
        start { transition: { from: Pending, to: Running } }
        complete { transition: { from: Running, to: Completed } }
        fail {
            transition: { from: Pending, to: Failed }
            transition: { from: Running, to: Failed }
        }
        cancel {
            transition: { from: Pending, to: Cancelled }
            transition: { from: Running, to: Cancelled }
        }
    }
}

impl ResultState {
    /// Validate against the existing result, without keeping a second state copy.
    pub(super) fn apply(&mut self, event: SynopsisJobEvent) -> bool {
        let state = match self {
            Self::Pending => SynopsisJobStateMachine::Pending,
            Self::Running => SynopsisJobStateMachine::Running,
            Self::Completed => SynopsisJobStateMachine::Completed,
            Self::Failed => SynopsisJobStateMachine::Failed,
            Self::Cancelled => SynopsisJobStateMachine::Cancelled,
        };
        let mut machine = DynamicSynopsisJob::new_init_state((), state);
        if machine.handle(event).is_err() {
            return false;
        }
        *self = match machine.current_state() {
            SynopsisJobStateMachine::Pending => Self::Pending,
            SynopsisJobStateMachine::Running => Self::Running,
            SynopsisJobStateMachine::Completed => Self::Completed,
            SynopsisJobStateMachine::Failed => Self::Failed,
            SynopsisJobStateMachine::Cancelled => Self::Cancelled,
        };
        true
    }
}

// Avoid confusing the generated state enum with the serialized result enum.
use self::SynopsisJobState as SynopsisJobStateMachine;

#[cfg(test)]
mod tests;
