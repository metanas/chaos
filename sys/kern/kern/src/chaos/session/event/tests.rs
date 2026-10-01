use super::*;
use crate::ChaosAuth;
use crate::Process;
use crate::ProcessTable;
use crate::chaos::make_session_and_context_with_home;
use crate::minions::AgentStatus;
use chaos_ipc::protocol::ErrorEvent;
use chaos_ipc::protocol::SessionConfiguredEvent;
use chaos_ipc::protocol::TurnAbortReason;
use chaos_ipc::protocol::TurnAbortedEvent;
use chaos_ipc::protocol::TurnCompleteEvent;
use chaos_ipc::protocol::TurnStartedEvent;
use chaos_ipc::protocol::WarningEvent;
use futures::FutureExt;
use std::sync::Arc;
use tempfile::TempDir;

struct LifecycleHarness {
    _home: TempDir,
    manager: ProcessTable,
    parent: Arc<Process>,
    child: Session,
    parent_events: async_channel::Receiver<Event>,
    child_events: async_channel::Receiver<Event>,
}

impl LifecycleHarness {
    async fn new() -> Self {
        let home = TempDir::new().expect("test home");
        let (mut parent, turn) = make_session_and_context_with_home(home.path()).await;
        let manager = ProcessTable::with_models_provider_and_home_for_tests(
            ChaosAuth::from_api_key("dummy"),
            turn.config.model_provider.clone(),
            home.path().to_path_buf(),
        );
        let (parent_tx, parent_events) = async_channel::unbounded();
        parent.tx_event = parent_tx;
        let parent_status = parent.agent_status.subscribe();
        let parent = manager
            .insert_session_for_tests(parent, &turn, parent_status)
            .await;
        let (mut child, _) = make_session_and_context_with_home(home.path()).await;
        let (child_tx, child_events) = async_channel::unbounded();
        child.tx_event = child_tx;
        child.services.agent_control = manager.agent_control();
        child
            .state
            .lock()
            .await
            .session_configuration
            .session_source = SessionSource::SubAgent(SubAgentSource::ProcessSpawn {
            parent_process_id: parent.chaos.session.conversation_id,
            depth: 1,
            agent_nickname: Some("Ada".into()),
            agent_role: Some("worker".into()),
        });
        Self {
            _home: home,
            manager,
            parent,
            child,
            parent_events,
            child_events,
        }
    }

    async fn emit(&self, msg: EventMsg) {
        self.child
            .deliver_event_raw(Event {
                id: "child-turn".into(),
                msg,
            })
            .await;
    }

    fn next_update(&self, status: CollabAgentStatus) -> CollabAgentStatusChangedEvent {
        let event = self.parent_events.try_recv().expect("live parent update");
        assert!(
            event.id.is_empty(),
            "not correlated with a parent tool call"
        );
        let EventMsg::CollabAgentStatusChanged(update) = event.msg else {
            panic!("expected child lifecycle update");
        };
        assert_eq!(
            update.parent_process_id,
            self.parent.chaos.session.conversation_id
        );
        assert_eq!(update.child_process_id, self.child.conversation_id);
        assert_eq!(update.agent_nickname.as_deref(), Some("Ada"));
        assert_eq!(update.agent_role.as_deref(), Some("worker"));
        assert_eq!(update.status, status);
        update
    }

    async fn configured(&self) -> EventMsg {
        let state = self.child.state.lock().await;
        let config = &state.session_configuration;
        EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: self.child.conversation_id,
            forked_from_id: None,
            process_name: None,
            model: config.collaboration_mode.model().to_string(),
            model_provider_id: "test".into(),
            service_tier: None,
            approval_policy: config.approval_policy.value(),
            approvals_reviewer: config.approvals_reviewer,
            vfs_policy: config.vfs_policy.clone(),
            socket_policy: config.socket_policy,
            cwd: config.cwd.clone(),
            reasoning_effort: config.collaboration_mode.reasoning_effort(),
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
        })
    }
}

fn started() -> EventMsg {
    EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: "child-turn".into(),
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    })
}

fn completed(message: Option<&str>) -> EventMsg {
    EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: "child-turn".into(),
        last_agent_message: message.map(str::to_string),
    })
}

#[tokio::test]
async fn initial_fast_completion_reactivation_and_close_publish_without_waiting() {
    let harness = LifecycleHarness::new().await;
    let receivers = harness.child.agent_status.receiver_count();
    harness.emit(harness.configured().await).await;
    let initial = harness.next_update(CollabAgentStatus::PendingInit);
    let effective_model = harness
        .child
        .state
        .lock()
        .await
        .session_configuration
        .collaboration_mode
        .model()
        .to_string();
    assert_eq!(initial.model.as_deref(), Some(effective_model.as_str()));

    // Do not read the parent's stream until the fast child turn has finished.
    harness.emit(started()).await;
    harness.emit(completed(Some("private child output"))).await;
    harness.next_update(CollabAgentStatus::Running);
    let completion = harness.next_update(CollabAgentStatus::Completed);
    assert!(
        !serde_json::to_string(&completion)
            .expect("serialize")
            .contains("private")
    );

    // Completion is not process termination; later input starts another turn.
    harness.emit(started()).await;
    harness.next_update(CollabAgentStatus::Running);
    harness.emit(completed(None)).await;
    harness.next_update(CollabAgentStatus::Completed);
    harness.emit(EventMsg::ShutdownComplete).await;
    harness.next_update(CollabAgentStatus::Shutdown);
    assert!(harness.parent_events.is_empty());
    assert_eq!(harness.child.agent_status.receiver_count(), receivers);
    assert!(
        harness
            .parent
            .chaos
            .session
            .services
            .internal_task_store
            .list()
            .await
            .is_empty()
    );
    assert_eq!(
        harness.parent.agent_status().await,
        AgentStatus::PendingInit
    );
}

#[tokio::test]
async fn publication_uses_preserved_failure_and_configuration_never_resets_status() {
    let harness = LifecycleHarness::new().await;
    harness.emit(started()).await;
    harness.next_update(CollabAgentStatus::Running);
    harness
        .emit(EventMsg::Error(ErrorEvent {
            message: "private provider error".into(),
            chaos_error_info: None,
        }))
        .await;
    let failure = harness.next_update(CollabAgentStatus::Errored);
    assert!(
        !serde_json::to_string(&failure)
            .expect("serialize")
            .contains("private")
    );
    harness.emit(completed(None)).await;
    harness.next_update(CollabAgentStatus::Errored);
    assert_eq!(
        *harness.child.agent_status.borrow(),
        AgentStatus::Errored("private provider error".into())
    );

    harness.emit(started()).await;
    harness.next_update(CollabAgentStatus::Running);
    harness
        .emit(EventMsg::TurnAborted(TurnAbortedEvent {
            turn_id: Some("child-turn".into()),
            reason: TurnAbortReason::Interrupted,
        }))
        .await;
    harness.next_update(CollabAgentStatus::Interrupted);
    harness.emit(completed(None)).await;
    harness.next_update(CollabAgentStatus::Interrupted);
    harness.emit(started()).await;
    harness.next_update(CollabAgentStatus::Running);
    harness.emit(completed(Some("private result"))).await;
    harness.next_update(CollabAgentStatus::Completed);
    harness.emit(harness.configured().await).await;
    harness.next_update(CollabAgentStatus::Completed);
}

#[tokio::test]
async fn publication_is_ordered_without_serializing_unrelated_events() {
    let harness = LifecycleHarness::new().await;
    let publication = harness.child.agent_status_publication.lock().await;
    let mut running = Box::pin(harness.emit(started()));
    let mut completion = Box::pin(harness.emit(completed(Some("done"))));
    assert!(running.as_mut().now_or_never().is_none());
    assert!(completion.as_mut().now_or_never().is_none());
    assert!(
        harness
            .emit(EventMsg::Warning(WarningEvent {
                message: "unrelated".into(),
            }))
            .now_or_never()
            .is_some()
    );
    assert!(harness.parent_events.is_empty());
    drop(publication);
    futures::join!(running, completion);
    harness.next_update(CollabAgentStatus::Running);
    harness.next_update(CollabAgentStatus::Completed);
    assert!(harness.parent_events.is_empty());
}

#[tokio::test]
async fn missing_closed_or_shutdown_parent_does_not_invent_completion_or_subscribe() {
    let harness = LifecycleHarness::new().await;
    let child_receivers = harness.child.agent_status.receiver_count();
    let parent_receivers = harness.parent.chaos.session.agent_status.receiver_count();
    harness
        .parent
        .chaos
        .session
        .agent_status
        .send_replace(AgentStatus::Shutdown);
    harness.emit(started()).await;
    assert!(harness.parent_events.is_empty());

    harness
        .parent
        .chaos
        .session
        .agent_status
        .send_replace(AgentStatus::Running);
    harness.parent_events.close();
    harness.emit(completed(Some("done"))).await;
    harness.emit(EventMsg::ShutdownComplete).await;
    assert!(harness.parent_events.is_empty());
    assert_eq!(harness.parent.agent_status().await, AgentStatus::Running);
    assert_eq!(harness.child.agent_status.receiver_count(), child_receivers);
    assert_eq!(
        harness.parent.chaos.session.agent_status.receiver_count(),
        parent_receivers
    );

    harness
        .manager
        .remove_process(&harness.parent.chaos.session.conversation_id)
        .await
        .expect("registered parent");
    harness.emit(started()).await;
    assert!(harness.parent_events.is_empty());
    drop(harness.manager);
    harness
        .child
        .deliver_event_raw(Event {
            id: "child-turn".into(),
            msg: started(),
        })
        .await;
    assert_eq!(*harness.child.agent_status.borrow(), AgentStatus::Running);
}

#[tokio::test]
async fn only_process_spawn_children_publish_and_other_child_events_stay_private() {
    let harness = LifecycleHarness::new().await;
    harness
        .emit(EventMsg::Warning(WarningEvent {
            message: "private child warning".into(),
        }))
        .await;
    assert!(harness.parent_events.is_empty());
    assert_eq!(harness.child_events.len(), 1);
    harness
        .child
        .state
        .lock()
        .await
        .session_configuration
        .session_source = SessionSource::Exec;
    harness.emit(started()).await;
    harness.emit(completed(Some("private result"))).await;
    assert!(harness.parent_events.is_empty());
    assert_eq!(
        *harness.child.agent_status.borrow(),
        AgentStatus::Completed(Some("private result".into()))
    );
}
