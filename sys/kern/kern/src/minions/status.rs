use chaos_ipc::protocol::AgentStatus;
use chaos_ipc::protocol::EventMsg;
use state_machines::state_machine;

state_machine! {
    name: AgentLifecycle,
    dynamic: true,
    initial: PendingInit,
    states: [
        superstate Open {
            superstate Healthy {
                state PendingInit,
                state Running,
                state Completed,
            }
            state Interrupted,
            state Errored,
        },
        Shutdown,
    ],
    events {
        start {
            transition: { from: Open, to: Running }
        }
        complete {
            transition: { from: Open, to: Completed }
        }
        complete_empty {
            transition: { from: Healthy, to: Completed }
        }
        interrupt {
            transition: { from: Open, to: Interrupted }
        }
        fail {
            transition: { from: Open, to: Errored }
        }
        shutdown {
            transition: { from: Open, to: Shutdown }
        }
    }
}

/// Derive the next agent status from a single emitted event.
/// Returns `None` when the event does not affect status tracking.
pub(crate) fn agent_status_from_event(msg: &EventMsg) -> Option<AgentStatus> {
    match msg {
        EventMsg::TurnStarted(_) => Some(AgentStatus::Running),
        EventMsg::TurnComplete(ev) => Some(AgentStatus::Completed(ev.last_agent_message.clone())),
        EventMsg::TurnAborted(ev) => match ev.reason {
            chaos_ipc::protocol::TurnAbortReason::Interrupted => Some(AgentStatus::Interrupted),
            _ => Some(AgentStatus::Errored(format!("{:?}", ev.reason))),
        },
        EventMsg::Error(ev) => Some(AgentStatus::Errored(ev.message.clone())),
        EventMsg::ShutdownComplete => Some(AgentStatus::Shutdown),
        _ => None,
    }
}

pub(crate) fn is_final(status: &AgentStatus) -> bool {
    !matches!(
        status,
        AgentStatus::PendingInit | AgentStatus::Running | AgentStatus::Interrupted
    )
}

/// Validate a status update without keeping a second copy of session state.
///
/// Empty completions cannot erase failures/interruption, and late events cannot
/// reopen a shutdown process. Resume creates a new session in PendingInit.
/// NotFound is a lookup result, never a live process transition.
pub(crate) fn transition_agent_status(current: &AgentStatus, next: AgentStatus) -> AgentStatus {
    let state = match current {
        AgentStatus::PendingInit => AgentLifecycleState::PendingInit,
        AgentStatus::Running => AgentLifecycleState::Running,
        AgentStatus::Completed(_) => AgentLifecycleState::Completed,
        AgentStatus::Interrupted => AgentLifecycleState::Interrupted,
        AgentStatus::Errored(_) => AgentLifecycleState::Errored,
        AgentStatus::Shutdown => AgentLifecycleState::Shutdown,
        AgentStatus::NotFound => return current.clone(),
    };
    let event = match &next {
        AgentStatus::Running => AgentLifecycleEvent::Start,
        AgentStatus::Completed(Some(_)) => AgentLifecycleEvent::Complete,
        AgentStatus::Completed(None) => AgentLifecycleEvent::CompleteEmpty,
        AgentStatus::Interrupted => AgentLifecycleEvent::Interrupt,
        AgentStatus::Errored(_) => AgentLifecycleEvent::Fail,
        AgentStatus::Shutdown => AgentLifecycleEvent::Shutdown,
        AgentStatus::PendingInit | AgentStatus::NotFound => return current.clone(),
    };
    if DynamicAgentLifecycle::new_init_state((), state)
        .handle(event)
        .is_ok()
    {
        next
    } else {
        current.clone()
    }
}

#[cfg(test)]
mod tests;
