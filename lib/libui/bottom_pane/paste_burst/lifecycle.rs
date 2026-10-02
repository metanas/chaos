//! Classification state. Text and the longer Enter-suppression deadline are
//! independent data: clearing classification must not silently drop either.
use state_machines::state_machine;

state_machine! {
    name: BurstClassification,
    dynamic: true,
    initial: Idle,
    states: [
        superstate Transient {
            state Idle, state Holding, state Buffering, state BufferingWithHeld,
        }
    ],
    events {
        hold {
            transition: { from: Idle, to: Holding }
            transition: { from: Holding, to: Holding }
        }
        capture_held { transition: { from: Holding, to: Buffering } }
        begin {
            transition: { from: Idle, to: Buffering }
            transition: { from: Holding, to: BufferingWithHeld }
            transition: { from: Buffering, to: Buffering }
            transition: { from: BufferingWithHeld, to: BufferingWithHeld }
        }
        flush_buffer {
            transition: { from: Idle, to: Idle }
            transition: { from: Holding, to: Holding }
            transition: { from: Buffering, to: Idle }
            transition: { from: BufferingWithHeld, to: Holding }
        }
        flush_held { transition: { from: Holding, to: Idle } }
        reset { transition: { from: Transient, to: Idle } }
    }
}

pub(super) struct Classification {
    machine: DynamicBurstClassification<()>,
}

impl Default for Classification {
    fn default() -> Self {
        Self {
            machine: DynamicBurstClassification::new(()),
        }
    }
}

impl Classification {
    pub(super) fn buffering(&self) -> bool {
        matches!(
            self.machine.current_state(),
            BurstClassificationState::Buffering | BurstClassificationState::BufferingWithHeld
        )
    }

    pub(super) fn apply(&mut self, event: BurstClassificationEvent) {
        assert!(
            self.machine.handle(event).is_ok(),
            "paste classification event matches held data"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flushing_buffer_preserves_a_separately_held_character() {
        let mut state = Classification::default();
        state.apply(BurstClassificationEvent::Hold);
        state.apply(BurstClassificationEvent::Begin);
        assert!(state.buffering());
        state.apply(BurstClassificationEvent::FlushBuffer);
        assert_eq!(
            state.machine.current_state(),
            BurstClassificationState::Holding
        );
        state.apply(BurstClassificationEvent::FlushHeld);
        assert_eq!(
            state.machine.current_state(),
            BurstClassificationState::Idle
        );
        state.apply(BurstClassificationEvent::Reset);
        assert!(!state.buffering());
    }
}
