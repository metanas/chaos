use super::{SessionStatus, TurnStatus};
use state_machines::state_machine;

mod session {
    use super::*;
    state_machine! {
        name: FrontendSession,
        dynamic: true,
        initial: Booting,
        states: [Booting, Ready, Shutdown],
        events {
            configure {
                transition: { from: Booting, to: Ready }
                transition: { from: Ready, to: Ready }
            }
            shutdown {
                transition: { from: Booting, to: Shutdown }
                transition: { from: Ready, to: Shutdown }
                transition: { from: Shutdown, to: Shutdown }
            }
        }
    }

    impl SessionStatus {
        pub(super) fn apply(&mut self, event: FrontendSessionEvent) {
            let state = match self {
                Self::Booting => FrontendSessionState::Booting,
                Self::Ready => FrontendSessionState::Ready,
                Self::Shutdown => FrontendSessionState::Shutdown,
            };
            let mut machine = DynamicFrontendSession::new_init_state((), state);
            // Late configuration cannot resurrect a shutdown frontend.
            if machine.handle(event).is_ok() {
                *self = match machine.current_state() {
                    FrontendSessionState::Booting => Self::Booting,
                    FrontendSessionState::Ready => Self::Ready,
                    FrontendSessionState::Shutdown => Self::Shutdown,
                };
            }
        }
    }
}

mod turn {
    use super::*;
    state_machine! {
        name: FrontendTurn,
        dynamic: true,
        initial: Idle,
        states: [superstate Turn { state Idle, state InFlight }],
        events {
            submit { transition: { from: Turn, to: InFlight } }
            finish { transition: { from: Turn, to: Idle } }
        }
    }

    impl TurnStatus {
        pub(super) fn apply(&mut self, event: FrontendTurnEvent) {
            let state = match self {
                Self::Idle => FrontendTurnState::Idle,
                Self::InFlight => FrontendTurnState::InFlight,
            };
            let mut machine = DynamicFrontendTurn::new_init_state((), state);
            assert!(
                machine.handle(event).is_ok(),
                "frontend turn event is valid"
            );
            *self = match machine.current_state() {
                FrontendTurnState::Idle => Self::Idle,
                FrontendTurnState::InFlight => Self::InFlight,
            };
        }
    }
}

impl SessionStatus {
    pub(super) fn configure(&mut self) {
        self.apply(session::FrontendSessionEvent::Configure);
    }
    pub(super) fn shutdown(&mut self) {
        self.apply(session::FrontendSessionEvent::Shutdown);
    }
}

impl TurnStatus {
    pub(super) fn submit(&mut self) {
        self.apply(turn::FrontendTurnEvent::Submit);
    }
    pub(super) fn finish(&mut self) {
        self.apply(turn::FrontendTurnEvent::Finish);
    }
}
