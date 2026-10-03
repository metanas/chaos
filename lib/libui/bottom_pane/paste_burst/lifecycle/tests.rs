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
