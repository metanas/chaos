use super::*;
use chaos_ipc::ProcessId;

#[test]
fn session_in_use_survives_init_context_and_io_wrapping() {
    let process_id = ProcessId::new();
    let error = anyhow::Error::new(std::io::Error::other(ChaosErr::SessionInUse(process_id)))
        .context("failed to create rollout recorder");
    let mapped = map_session_init_error(&error, Path::new("/unused"));
    std::assert_matches!(mapped, ChaosErr::SessionInUse(id) if id == process_id);
    assert!(!mapped.is_retryable());
}

#[test]
fn session_init_does_not_classify_errors_by_message_text() {
    let error = anyhow::anyhow!("LeaseConflict: unrelated storage failure")
        .context("failed to create rollout recorder");
    let mapped = map_session_init_error(&error, Path::new("/unused"));
    std::assert_matches!(mapped, ChaosErr::Fatal(_));
    assert!(mapped.to_string().contains("unrelated storage failure"));
}
