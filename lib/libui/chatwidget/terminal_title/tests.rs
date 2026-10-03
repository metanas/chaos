#![allow(clippy::expect_used)]

use super::*;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::chatwidget::tests::drain_terminal_titles;
use crate::chatwidget::tests::make_chatwidget_manual;
use chaos_ipc::approvals::ElicitationRequest;
use chaos_ipc::approvals::ElicitationRequestEvent;
use chaos_ipc::mcp::RequestId;
use chaos_ipc::protocol::ApplyPatchApprovalRequestEvent;
use chaos_ipc::protocol::Event;
use chaos_ipc::protocol::EventMsg;
use chaos_ipc::protocol::ExecApprovalRequestEvent;
use chaos_ipc::protocol::ExecCommandBeginEvent;
use chaos_ipc::protocol::ExecCommandSource;
use chaos_ipc::protocol::McpStartupCompleteEvent;
use chaos_ipc::protocol::McpStartupStatus;
use chaos_ipc::protocol::McpStartupUpdateEvent;
use chaos_ipc::protocol::ProcessNameUpdatedEvent;
use chaos_ipc::request_permissions::RequestPermissionsEvent;
use chaos_ipc::request_user_input::RequestUserInputEvent;
use chaos_ipc::request_user_input::RequestUserInputQuestion;
use chaos_ipc::request_user_input::RequestUserInputQuestionOption;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use serde_json::json;

#[test]
fn title_without_icons_preserves_existing_behavior() {
    assert_eq!(
        terminal_title_text(
            Some("Compaction Control"),
            TerminalTitleState::Idle,
            None,
            0
        ),
        "Compaction Control"
    );
    assert_eq!(
        terminal_title_text(None, TerminalTitleState::Idle, None, 0),
        "new session"
    );
}

fn configure_icons(chat: &mut ChatWidget) {
    chat.config.tui_terminal_title_icon = Some("✦".to_string());
    chat.process_name = Some("Terminal Icons".to_string());
}

fn exec_request(call_id: &str) -> ExecApprovalRequestEvent {
    ExecApprovalRequestEvent {
        call_id: call_id.to_string(),
        approval_id: None,
        turn_id: "turn-1".to_string(),
        command: vec!["echo".to_string(), "hello".to_string()],
        cwd: std::env::current_dir().expect("cwd"),
        reason: None,
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
        parsed_cmd: vec![],
    }
}

fn user_input_request() -> RequestUserInputEvent {
    RequestUserInputEvent {
        call_id: "input-1".to_string(),
        turn_id: "turn-1".to_string(),
        questions: vec![RequestUserInputQuestion {
            id: "choice".to_string(),
            header: "Choice".to_string(),
            question: "Continue?".to_string(),
            is_other: false,
            is_secret: false,
            options: Some(vec![RequestUserInputQuestionOption {
                label: "Yes".to_string(),
                description: "Continue the task.".to_string(),
            }]),
        }],
    }
}

fn form_request(
    schema: serde_json::Value,
    meta: Option<serde_json::Value>,
) -> ElicitationRequestEvent {
    ElicitationRequestEvent {
        turn_id: Some("turn-1".to_string()),
        server_name: "server-1".to_string(),
        id: RequestId::String("request-1".to_string()),
        request: ElicitationRequest::Form {
            meta,
            message: "Continue?".to_string(),
            requested_schema: schema,
        },
    }
}

pub(in crate::chatwidget) async fn attention_lifecycle_tests() {
    Box::pin(activity_uses_fixed_markers_without_identity_icon()).await;
    Box::pin(attention_overrides_working_until_last_approval_resolves()).await;
    Box::pin(interactive_requests_enter_and_cancel_attention()).await;
    Box::pin(user_input_and_mcp_responses_leave_attention()).await;
    Box::pin(clearing_a_form_draft_does_not_clear_attention()).await;
    Box::pin(deferred_requests_require_attention_before_display()).await;
    Box::pin(background_requests_and_stacked_menus_preserve_attention()).await;
}

async fn activity_uses_fixed_markers_without_identity_icon() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(None).await;
    chat.config.tui_terminal_title_icon = None;
    chat.process_name = Some("Terminal Icons".to_string());
    chat.on_task_started();
    assert_eq!(drain_terminal_titles(&mut rx), ["◰ Terminal Icons"]);
    chat.handle_exec_approval_now(exec_request("exec-1"));
    assert_eq!(drain_terminal_titles(&mut rx), ["◻ Terminal Icons"]);
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    assert_eq!(drain_terminal_titles(&mut rx), ["◰ Terminal Icons"]);
    chat.on_task_complete(None, false);
    assert_eq!(drain_terminal_titles(&mut rx), ["Terminal Icons"]);
}

async fn attention_overrides_working_until_last_approval_resolves() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(None).await;
    configure_icons(&mut chat);
    let process_id = chaos_ipc::ProcessId::new();
    chat.process_id = Some(process_id);
    chat.on_task_started();
    chat.on_mcp_startup_update(McpStartupUpdateEvent {
        server: "browser".to_string(),
        status: McpStartupStatus::Starting,
    });
    drain_terminal_titles(&mut rx);

    chat.on_exec_approval_request("event-1".to_string(), exec_request("exec-1"));
    chat.on_exec_approval_request("event-2".to_string(), exec_request("exec-2"));
    assert_eq!(drain_terminal_titles(&mut rx), ["◻ Terminal Icons"]);

    chat.on_process_name_updated(ProcessNameUpdatedEvent {
        process_id,
        process_name: Some("Renamed".to_string()),
    });
    assert_eq!(drain_terminal_titles(&mut rx), ["◻ Renamed"]);

    chat.handle_key_event(KeyEvent::from(KeyCode::Char('y')));
    assert!(chat.bottom_pane.has_active_view());
    assert!(drain_terminal_titles(&mut rx).is_empty());
    chat.handle_key_event(KeyEvent::from(KeyCode::Char('y')));
    assert!(!chat.bottom_pane.has_active_view());
    assert_eq!(drain_terminal_titles(&mut rx), ["◰ Renamed"]);

    chat.on_task_complete(None, false);
    assert_eq!(drain_terminal_titles(&mut rx), ["◰ Renamed"]);
    chat.on_mcp_startup_complete(McpStartupCompleteEvent::default());
    assert_eq!(drain_terminal_titles(&mut rx), ["✦ Renamed"]);
}

async fn interactive_requests_enter_and_cancel_attention() {
    let boolean_schema = json!({
        "type": "object",
        "properties": { "confirmed": { "type": "boolean" } },
        "required": ["confirmed"],
    });
    let requests = [
        EventMsg::ExecApprovalRequest(exec_request("exec-1")),
        EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id: "patch-1".to_string(),
            turn_id: "turn-1".to_string(),
            changes: Default::default(),
            reason: None,
            grant_root: None,
        }),
        EventMsg::RequestPermissions(RequestPermissionsEvent {
            call_id: "permissions-1".to_string(),
            turn_id: "turn-1".to_string(),
            reason: None,
            permissions: Default::default(),
        }),
        EventMsg::RequestUserInput(user_input_request()),
        EventMsg::ElicitationRequest(form_request(boolean_schema, None)),
        EventMsg::ElicitationRequest(form_request(
            json!({ "type": "object", "properties": {} }),
            Some(json!({
                "codex_approval_kind": "tool_suggestion",
                "tool_type": "connector",
                "suggest_type": "install",
                "suggest_reason": "Use this app",
                "tool_id": "connector_1",
                "tool_name": "Calendar",
                "install_url": "https://example.test/install",
            })),
        )),
        EventMsg::ElicitationRequest(ElicitationRequestEvent {
            turn_id: Some("turn-1".to_string()),
            server_name: "server-1".to_string(),
            id: RequestId::String("url-1".to_string()),
            request: ElicitationRequest::Url {
                meta: None,
                message: "Sign in".to_string(),
                url: "https://example.test/login".to_string(),
                elicitation_id: "url-1".to_string(),
            },
        }),
    ];
    let (mut chat, mut rx, _) = make_chatwidget_manual(None).await;
    configure_icons(&mut chat);
    for (index, msg) in requests.into_iter().enumerate() {
        chat.handle_codex_event(Event {
            id: format!("event-{index}"),
            msg,
        });
        assert_eq!(drain_terminal_titles(&mut rx), ["◻ Terminal Icons"]);
        let cancel = if index % 2 == 0 {
            KeyEvent::from(KeyCode::Esc)
        } else {
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        };
        chat.handle_key_event(cancel);
        assert!(!chat.bottom_pane.has_active_view(), "request {index}");
        assert_eq!(drain_terminal_titles(&mut rx), ["✦ Terminal Icons"]);
    }
}

async fn user_input_and_mcp_responses_leave_attention() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(None).await;
    configure_icons(&mut chat);
    chat.handle_request_user_input_now(user_input_request());
    assert_eq!(drain_terminal_titles(&mut rx), ["◻ Terminal Icons"]);
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(!chat.bottom_pane.has_active_view());
    assert_eq!(drain_terminal_titles(&mut rx), ["✦ Terminal Icons"]);

    chat.handle_elicitation_request_now(form_request(
        json!({
            "type": "object",
            "properties": { "confirmed": { "type": "boolean" } },
            "required": ["confirmed"],
        }),
        None,
    ));
    assert_eq!(drain_terminal_titles(&mut rx), ["◻ Terminal Icons"]);
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(!chat.bottom_pane.has_active_view());
    assert_eq!(drain_terminal_titles(&mut rx), ["✦ Terminal Icons"]);
}

async fn clearing_a_form_draft_does_not_clear_attention() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(None).await;
    configure_icons(&mut chat);
    chat.handle_elicitation_request_now(form_request(
        json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
        }),
        None,
    ));
    assert_eq!(drain_terminal_titles(&mut rx), ["◻ Terminal Icons"]);
    chat.handle_paste("draft".to_string());
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(chat.bottom_pane.has_active_view());
    assert!(drain_terminal_titles(&mut rx).is_empty());
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(!chat.bottom_pane.has_active_view());
    assert_eq!(drain_terminal_titles(&mut rx), ["✦ Terminal Icons"]);
}

async fn deferred_requests_require_attention_before_display() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(None).await;
    configure_icons(&mut chat);
    chat.on_task_started();
    chat.handle_streaming_delta("Still streaming".to_string());
    drain_terminal_titles(&mut rx);
    assert!(chat.stream_controller.is_some());

    chat.interrupts.push_exec_begin(ExecCommandBeginEvent {
        call_id: "running-exec".to_string(),
        process_id: None,
        turn_id: "turn-1".to_string(),
        command: vec!["echo".to_string()],
        cwd: chat.config.cwd.clone(),
        parsed_cmd: vec![],
        source: ExecCommandSource::Agent,
        interaction_input: None,
    });
    chat.refresh_terminal_title_if_attention_changed();
    assert!(drain_terminal_titles(&mut rx).is_empty());
    chat.on_exec_approval_request("event-1".to_string(), exec_request("exec-1"));
    assert!(!chat.bottom_pane.has_active_view());
    assert_eq!(drain_terminal_titles(&mut rx), ["◻ Terminal Icons"]);
    chat.flush_interrupt_queue();
    assert!(chat.bottom_pane.has_active_view());
    assert!(drain_terminal_titles(&mut rx).is_empty());
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    assert_eq!(drain_terminal_titles(&mut rx), ["◰ Terminal Icons"]);
}

async fn background_requests_and_stacked_menus_preserve_attention() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(None).await;
    configure_icons(&mut chat);
    let menu = || SelectionViewParams {
        title: Some("Settings".to_string()),
        items: vec![SelectionItem {
            name: "Back".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    chat.show_selection_view(menu());
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    assert!(drain_terminal_titles(&mut rx).is_empty());

    chat.set_pending_process_approvals(vec!["Background agent".to_string()]);
    assert_eq!(drain_terminal_titles(&mut rx), ["◻ Terminal Icons"]);
    chat.handle_exec_approval_now(exec_request("exec-1"));
    chat.show_selection_view(menu());
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    assert!(chat.bottom_pane.has_active_view());
    assert!(drain_terminal_titles(&mut rx).is_empty());
    chat.set_pending_process_approvals(Vec::new());
    assert!(drain_terminal_titles(&mut rx).is_empty());
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    assert_eq!(drain_terminal_titles(&mut rx), ["✦ Terminal Icons"]);
    chat.set_pending_process_approvals(vec!["Background agent".to_string()]);
    assert_eq!(drain_terminal_titles(&mut rx), ["◻ Terminal Icons"]);
    chat.set_pending_process_approvals(Vec::new());
    assert_eq!(drain_terminal_titles(&mut rx), ["✦ Terminal Icons"]);
}
