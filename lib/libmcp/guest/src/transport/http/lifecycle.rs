//! Runtime transitions are serialized by the transport's short-lived state
//! mutex. Network I/O never runs while it is held; atomics still gate admission.
use state_machines::state_machine;

mod session {
    use super::*;
    state_machine! {
        name: HttpSession,
        dynamic: true,
        initial: Fresh,
        states: [
            superstate Open {
                state Fresh, state Initializing, state Negotiated, state Ready,
                state RecoveringFresh, state RecoveringReady,
            },
            Closed,
        ],
        events {
            initialize {
                transition: { from: Fresh, to: Initializing }
                transition: { from: Initializing, to: Initializing }
                transition: { from: Negotiated, to: Initializing }
                transition: { from: Ready, to: Ready }
                transition: { from: RecoveringFresh, to: RecoveringFresh }
                transition: { from: RecoveringReady, to: RecoveringReady }
            }
            negotiated {
                transition: { from: Fresh, to: Negotiated }
                transition: { from: Initializing, to: Negotiated }
                transition: { from: Negotiated, to: Negotiated }
                transition: { from: Ready, to: Ready }
                transition: { from: RecoveringFresh, to: RecoveringFresh }
                transition: { from: RecoveringReady, to: RecoveringReady }
            }
            initialized {
                transition: { from: Fresh, to: Ready }
                transition: { from: Initializing, to: Ready }
                transition: { from: Negotiated, to: Ready }
                transition: { from: Ready, to: Ready }
                transition: { from: RecoveringFresh, to: RecoveringReady }
                transition: { from: RecoveringReady, to: RecoveringReady }
            }
            recover {
                transition: { from: Fresh, to: RecoveringFresh }
                transition: { from: Initializing, to: RecoveringFresh }
                transition: { from: Negotiated, to: RecoveringFresh }
                transition: { from: Ready, to: RecoveringReady }
                transition: { from: RecoveringFresh, to: RecoveringFresh }
                transition: { from: RecoveringReady, to: RecoveringReady }
            }
            restored {
                transition: { from: RecoveringFresh, to: Negotiated }
                transition: { from: RecoveringReady, to: Ready }
            }
            close {
                transition: { from: Open, to: Closed }
                transition: { from: Closed, to: Closed }
            }
        }
    }
}

mod sse {
    use super::*;
    state_machine! {
        name: HttpSse,
        dynamic: true,
        initial: Idle,
        states: [
            superstate Enabled { state Idle, state Connecting, state Streaming, state Backoff },
            Disabled, Stopped,
        ],
        events {
            connect { transition: { from: Enabled, to: Connecting } }
            connected { transition: { from: Connecting, to: Streaming } }
            retry { transition: { from: Enabled, to: Backoff } }
            disable { transition: { from: Enabled, to: Disabled } }
            stop {
                transition: { from: Enabled, to: Idle }
                transition: { from: Disabled, to: Disabled }
                transition: { from: Stopped, to: Stopped }
            }
            close {
                transition: { from: Enabled, to: Stopped }
                transition: { from: Disabled, to: Stopped }
                transition: { from: Stopped, to: Stopped }
            }
        }
    }
}

pub(super) use session::HttpSessionEvent;
pub(super) use sse::HttpSseEvent;

pub(super) struct Lifecycle {
    session: session::DynamicHttpSession<()>,
    sse: sse::DynamicHttpSse<()>,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            session: session::DynamicHttpSession::new(()),
            sse: sse::DynamicHttpSse::new(()),
        }
    }
}

impl Lifecycle {
    pub(super) fn session(&mut self, event: HttpSessionEvent) -> bool {
        self.session.handle(event).is_ok()
    }
    pub(super) fn sse(&mut self, event: HttpSseEvent) -> bool {
        self.sse.handle(event).is_ok()
    }
    pub(super) fn initialized(&self) -> bool {
        matches!(
            self.session.current_state(),
            session::HttpSessionState::Ready | session::HttpSessionState::RecoveringReady
        )
    }
    pub(super) fn sse_enabled(&self) -> bool {
        !matches!(
            self.sse.current_state(),
            sse::HttpSseState::Disabled | sse::HttpSseState::Stopped
        )
    }
    pub(super) fn close(&mut self) {
        assert!(self.session(HttpSessionEvent::Close));
        assert!(self.sse(HttpSseEvent::Close));
    }
}

#[cfg(test)]
mod tests;
