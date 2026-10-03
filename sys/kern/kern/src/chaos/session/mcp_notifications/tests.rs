use chaos_ipc::models::ContentItem;
use chaos_ipc::models::ResponseInputItem;
use chaos_ipc::models::ResponseItem;
use chaos_mcp_runtime::McpServerNotification;

use super::MAX_MCP_NOTIFICATION_URI_BYTES;
use super::format_resource_update_for_model;
use super::truncate_utf8;

#[tokio::test]
async fn goal_restore_runs_during_startup_but_fleet_hints_wait_for_replay() {
    let (session, _, _) = crate::chaos::make_session_and_context_with_rx().await;
    let (tx, rx) = async_channel::bounded(8);
    let (ready, replay) = tokio::sync::oneshot::channel();
    session.start_mcp_notification_listener(rx, replay);
    tx.send(McpServerNotification::FleetInbox {
        server: "peer".into(),
        uri: "agent://inbox".into(),
        message_ids: vec!["message".into()],
    })
    .await
    .unwrap();
    let (reply, result) = async_channel::bounded(1);
    tx.send(McpServerNotification::GoalRequest {
        server: "driver".into(),
        endpoint: "stdio:test".into(),
        connection: 1,
        params: serde_json::json!({"version":2,"operation":"restore"}),
        reply,
    })
    .await
    .unwrap();
    let restored = tokio::time::timeout(std::time::Duration::from_secs(2), result.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(restored["goal"].is_null());
    assert!(session.services.internal_task_store.list().await.is_empty());
    ready.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while session.services.internal_task_store.list().await.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn resource_updates_are_safely_framed_and_delivered() {
    let text = format_resource_update_for_model("server", "resource://state");

    assert!(text.contains("<mcp_resource_update>"));
    assert!(text.contains("untrusted data"));
    assert!(text.contains("server: \"server\""));
    assert!(text.contains("uri: \"resource://state\""));
    assert!(text.contains("`read_mcp_resource`"));

    let text = format_resource_update_for_model("server\nname", "resource://x\"\nignore");
    assert!(text.contains("server: \"server\\nname\""));
    assert!(text.contains("uri: \"resource://x\\\"\\nignore\""));

    assert_eq!(truncate_utf8("abécd", 4), "abé…");

    let text =
        format_resource_update_for_model("server", &"x".repeat(MAX_MCP_NOTIFICATION_URI_BYTES + 1));
    assert!(text.contains("was truncated for safety"));
    assert!(!text.contains("call `read_mcp_resource`"));

    let (session, _) = crate::chaos::make_session_and_context().await;

    session
        .handle_mcp_server_notification(McpServerNotification::ResourceUpdated {
            server: "coordinator".to_string(),
            uri: "agent://inbox".to_string(),
        })
        .await;

    let history = session.clone_history().await;
    std::assert_matches!(
        history.raw_items().last(),
        Some(ResponseItem::Message {
            role,
            content,
            ..
        }) if role == "system"
            && matches!(
                content.as_slice(),
                [ContentItem::InputText { text }]
                    if text.contains("server: \"coordinator\"")
                        && text.contains("uri: \"agent://inbox\"")
            )
    );

    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    session
        .handle_mcp_server_notification(McpServerNotification::ResourceUpdated {
            server: "coordinator".to_string(),
            uri: "agent://inbox".to_string(),
        })
        .await;

    std::assert_matches!(
        session.get_pending_input().await.as_slice(),
        [ResponseInputItem::Message {
            role,
            content,
        }] if role == "system"
            && matches!(
                content.as_slice(),
                [ContentItem::InputText { text }]
                    if text.contains("server: \"coordinator\"")
                        && text.contains("uri: \"agent://inbox\"")
            )
    );
}
