use super::*;

#[test]
fn terminal_results_cannot_be_reopened_or_overwritten() {
    for terminal in [
        ResultState::Completed,
        ResultState::Failed,
        ResultState::Cancelled,
    ] {
        let mut state = terminal;
        assert!(!state.apply(SynopsisJobEvent::Start));
        assert!(!state.apply(SynopsisJobEvent::Complete));
        assert!(!state.apply(SynopsisJobEvent::Fail));
        assert!(!state.apply(SynopsisJobEvent::Cancel));
        assert_eq!(state, terminal);
    }
    let mut state = ResultState::Pending;
    assert!(state.apply(SynopsisJobEvent::Fail)); // Spawn failures precede Running.
    assert_eq!(state, ResultState::Failed);
}
