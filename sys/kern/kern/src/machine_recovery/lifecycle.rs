use super::Phase;
use state_machines::state_machine;

state_machine! {
    name: RecoveryObservation,
    dynamic: true,
    initial: Healthy,
    states: [
        superstate Observed {
            state Healthy,
            superstate Episode {
                state Warning, state Stabilizing, state Recovered,
                state Unavailable, state Blocked,
            }
        }
    ],
    events {
        warn { transition: { from: Observed, to: Warning } }
        healthy { transition: { from: Healthy, to: Healthy } }
        unavailable { transition: { from: Episode, to: Unavailable } }
        blocked { transition: { from: Episode, to: Blocked } }
        stable { transition: { from: Episode, to: Recovered } }
        headroom { transition: { from: Episode, to: Stabilizing } }
    }
}

impl Phase {
    pub(super) fn observe(&mut self, event: RecoveryObservationEvent) {
        let state = match self {
            Self::Healthy => RecoveryObservationState::Healthy,
            Self::Warning => RecoveryObservationState::Warning,
            Self::Stabilizing => RecoveryObservationState::Stabilizing,
            Self::Recovered => RecoveryObservationState::Recovered,
            Self::Unavailable => RecoveryObservationState::Unavailable,
            Self::Blocked => RecoveryObservationState::Blocked,
        };
        let mut machine = DynamicRecoveryObservation::new_init_state((), state);
        assert!(
            machine.handle(event).is_ok(),
            "observation belongs to the current recovery episode"
        );
        *self = match machine.current_state() {
            RecoveryObservationState::Healthy => Self::Healthy,
            RecoveryObservationState::Warning => Self::Warning,
            RecoveryObservationState::Stabilizing => Self::Stabilizing,
            RecoveryObservationState::Recovered => Self::Recovered,
            RecoveryObservationState::Unavailable => Self::Unavailable,
            RecoveryObservationState::Blocked => Self::Blocked,
        };
    }
}
