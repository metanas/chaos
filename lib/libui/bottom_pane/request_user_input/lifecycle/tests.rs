use super::*;

#[test]
fn queued_requests_reopen_editing_but_finished_requests_cannot_submit_twice() {
    let mut request = RequestLifecycle::default();
    assert!(request.apply(InputRequestEvent::Confirm));
    assert!(request.confirming());
    assert!(request.apply(InputRequestEvent::Back));
    assert!(request.apply(InputRequestEvent::Submit));
    assert!(!request.apply(InputRequestEvent::Submit));
    assert!(request.apply(InputRequestEvent::Next));
    assert!(request.apply(InputRequestEvent::Submit));
    assert!(request.apply(InputRequestEvent::Finish));
    assert!(request.done());
    assert!(!request.apply(InputRequestEvent::Submit));
    assert!(!request.apply(InputRequestEvent::Next));
}
