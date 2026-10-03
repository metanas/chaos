use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use chaos_ipc::ProcessId;
use chaos_ipc::models::DeveloperInstructions;
use chaos_ipc::protocol::RolloutItem;
use chaos_ipc::protocol::SessionSource;
use chaos_ipc::protocol::SubAgentSource;

use super::hook_agent_context;
use super::write_stop_hook_transcript_snapshot;

#[test]
fn stop_hook_snapshot_is_fork_ready_and_ephemeral() {
    let item = DeveloperInstructions::new("snapshot marker").into();
    let snapshot = write_stop_hook_transcript_snapshot(&[item]).expect("write snapshot");
    let path = snapshot.path().to_path_buf();
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&path)
            .expect("snapshot metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    let rollout_items: Vec<RolloutItem> =
        serde_json::from_slice(&fs::read(&path).expect("read snapshot"))
            .expect("parse rollout snapshot");
    assert_eq!(rollout_items.len(), 1);
    std::assert_matches!(rollout_items[0], RolloutItem::ResponseItem(_));

    drop(snapshot);
    assert!(!path.exists());
}

#[test]
fn root_sessions_have_no_agent_identity() {
    let context = hook_agent_context(ProcessId::default(), &SessionSource::Cli);

    assert!(!context.is_subagent);
    assert!(context.agent_id.is_none());
    assert!(context.agent_type.is_none());
    assert!(context.parent_session_id.is_none());
    assert!(context.agent_depth.is_none());
}

#[test]
fn spawned_subagents_preserve_parent_role_and_depth() {
    let child_id = ProcessId::default();
    let parent_id = ProcessId::default();
    let context = hook_agent_context(
        child_id,
        &SessionSource::SubAgent(SubAgentSource::ProcessSpawn {
            parent_process_id: parent_id,
            depth: 2,
            agent_nickname: Some("swift-otter".to_string()),
            agent_role: Some("scout".to_string()),
        }),
    );
    let child_id = child_id.to_string();
    let parent_id = parent_id.to_string();

    assert!(context.is_subagent);
    assert_eq!(context.agent_id.as_deref(), Some(child_id.as_str()));
    assert_eq!(context.agent_type.as_deref(), Some("scout"));
    assert_eq!(
        context.parent_session_id.as_deref(),
        Some(parent_id.as_str())
    );
    assert_eq!(context.agent_depth, Some(2));
}
