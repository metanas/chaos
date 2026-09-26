use super::*;
use chaos_ipc::models::FunctionCallOutputPayload;
use chaos_ipc::protocol::{GoalCheckpointItem, RolloutItem};

mod checks;

fn snapshot() -> Snapshot {
    Snapshot {
        id: "goal".into(),
        turn_id: "turn".into(),
        revision: 1,
        objective: "Fix the regression".into(),
        criteria: vec!["Regression test passes".into()],
        status: Status::Active,
        checks: 0,
        reason: "awaiting_checkpoint".into(),
        attempt_id: None,
        evidence_digest: None,
        verdict_ref: None,
    }
}

fn gate() -> GoalGate {
    let mut gate = GoalGate::default();
    gate.begin_turn("turn", "Fix the regression and test it");
    gate.initial_input = false;
    gate.permit = Some(Permit {
        token: "test".into(),
        server: "driver".into(),
        turn_id: "turn".into(),
        tool: "start_goal".into(),
        arguments: json!({"objective":snapshot().objective,"criteria":snapshot().criteria}),
    });
    gate
}

fn observe(gate: &mut GoalGate, text: &str) {
    gate.record(&ResponseItem::FunctionCallOutput {
        call_id: "test".into(),
        tool_name: Some("exec_command".into()),
        output: FunctionCallOutputPayload::from_text(text.into()),
    });
}

fn record(conversation_id: chaos_ipc::ProcessId, goal: &Snapshot) -> RolloutItem {
    RolloutItem::GoalCheckpoint(GoalCheckpointItem {
        conversation_id,
        server: "driver".into(),
        endpoint: "stdio:test".into(),
        version: VERSION,
        snapshot: json!(goal),
        verdict: None,
    })
}

fn checking(gate: &mut GoalGate, old: &Snapshot) -> Snapshot {
    gate.permit.as_mut().unwrap().tool = "check_goal".into();
    let mut next = old.clone();
    next.revision += 1;
    next.checks += 1;
    next.status = Status::Checking;
    next.attempt_id = Some(format!("attempt-{}", next.checks));
    next.evidence_digest = Some(gate.evidence.digest());
    next.verdict_ref = None;
    next
}

#[test]
fn start_requires_exact_approved_route_arguments_and_current_turn() {
    let mut gate = gate();
    let journal = journal::Journal::default();
    let mut next = snapshot();
    assert!(journal.validate(&gate, "driver", 0, &next).is_ok());
    assert!(journal.validate(&gate, "other", 0, &next).is_err());
    next.objective = "weaker scope".into();
    assert!(journal.validate(&gate, "driver", 0, &next).is_err());
    gate.end_turn("turn", "cancelled");
    assert!(journal.validate(&gate, "driver", 0, &snapshot()).is_err());
}

#[test]
fn pause_cannot_reset_budget_or_definition() {
    let gate = gate();
    let mut journal = journal::Journal::default();
    journal.goal = Some(snapshot());
    let mut next = snapshot();
    next.revision += 1;
    next.status = Status::Paused;
    assert!(journal.validate(&gate, "driver", 1, &next).is_ok());
    journal.goal = Some(next.clone());
    next.id = "replacement".into();
    next.revision += 1;
    next.status = Status::Active;
    assert!(journal.validate(&gate, "driver", 2, &next).is_err());
    next.id = "goal".into();
    next.criteria.clear();
    assert!(journal.validate(&gate, "driver", 2, &next).is_err());
}

#[test]
fn evaluation_requires_new_evidence_and_persisted_consumption() {
    let mut gate = gate();
    let mut journal = journal::Journal::default();
    journal.goal = Some(snapshot());
    let next = checking(&mut gate, &snapshot());
    assert!(journal.validate(&gate, "driver", 1, &next).is_err());
    observe(&mut gate, "test passed");
    let mut next = checking(&mut gate, &snapshot());
    assert!(journal.validate(&gate, "driver", 1, &next).is_ok());
    next.checks = 0;
    assert!(journal.validate(&gate, "driver", 1, &next).is_err());
    let mut old = snapshot();
    old.checks = 4;
    journal.goal = Some(old.clone());
    let next = checking(&mut gate, &old);
    assert!(journal.validate(&gate, "driver", 1, &next).is_err());
}

#[test]
fn completion_requires_host_verdict_bound_to_attempt_revision_and_evidence() {
    let mut gate = gate();
    observe(&mut gate, "test passed");
    let current = checking(&mut gate, &snapshot());
    let mut journal = journal::Journal::default();
    journal.goal = Some(current.clone());
    let mut complete = current.clone();
    complete.revision += 1;
    complete.status = Status::Complete;
    complete.verdict_ref = Some("verdict".into());
    assert!(journal.validate(&gate, "driver", 2, &complete).is_err());
    journal.verdict = Some(VerdictRecord {
        revision: 2,
        attempt_id: current.attempt_id.unwrap(),
        evidence_digest: gate.evidence.digest(),
        reference: "verdict".into(),
        evaluation: Evaluation::Complete,
    });
    assert!(journal.validate(&gate, "driver", 2, &complete).is_ok());
    observe(&mut gate, "later test failed");
    assert!(journal.validate(&gate, "driver", 2, &complete).is_err());
    gate.pause("cancelled");
    assert!(journal.validate(&gate, "driver", 2, &complete).is_err());
}

#[test]
fn recovery_preserves_budget_and_isolates_conversations_and_routes() {
    let id = chaos_ipc::ProcessId::new();
    let mut goal = snapshot();
    goal.checks = 3;
    goal.status = Status::Checking;
    goal.attempt_id = Some("attempt".into());
    let item = record(id, &goal);
    let encoded = serde_json::to_value(&item).unwrap();
    let item = serde_json::from_value(encoded).unwrap();
    let mut journal = journal::Journal::default();
    journal.restore(id, &[item]).unwrap();
    assert_eq!(journal.goal, Some(goal.clone()));
    assert!(journal.interrupted);
    journal.bind("driver", "stdio:test", 2, true).unwrap();
    assert!(journal.bind("driver", "stdio:test", 1, true).is_err());
    assert!(journal.bind("other", "stdio:test", 3, true).is_err());
    assert!(journal.bind("driver", "other", 3, true).is_err());
    let mut fork = journal::Journal::default();
    fork.restore(chaos_ipc::ProcessId::new(), &[record(id, &goal)])
        .unwrap();
    assert!(fork.goal.is_none());
    goal.status = Status::Complete;
    let mut restarted = journal::Journal::default();
    restarted.restore(id, &[record(id, &goal)]).unwrap();
    assert!(!restarted.interrupted);
    assert_eq!(restarted.goal.unwrap().checks, 3);
}

#[test]
fn goal_status_and_claims_are_not_evidence() {
    let mut gate = gate();
    for (name, arguments) in [
        ("mcp__driver__check_goal", json!({"claim":"x".repeat(8192)})),
        (
            "read_mcp_resource",
            json!({"server":"driver","uri":"goal://current"}),
        ),
    ] {
        gate.record(&serde_json::from_value(json!({
            "type":"function_call","call_id":"status","name":name,"arguments":arguments.to_string(),
        })).unwrap());
        gate.record(&ResponseItem::FunctionCallOutput {
            call_id: "status".into(),
            tool_name: None,
            output: FunctionCallOutputPayload::from_text("Goal complete".into()),
        });
    }
    assert!(gate.evidence.is_empty());
    assert!(!gate.evidence.truncated);
    for i in 0..100 {
        observe(&mut gate, &format!("{i} {}", "λ".repeat(10000)));
    }
    assert!(gate.evidence.truncated);
    assert!(serde_json::to_vec(gate.evidence.entries()).unwrap().len() <= 49_152);
}

#[test]
fn original_user_input_is_not_an_interruption_but_later_input_is() {
    let mut gate = gate();
    gate.begin_turn("turn", "fix");
    let user: ResponseItem = serde_json::from_value(json!({
        "type":"message","role":"user","content":[{"type":"input_text","text":"fix"}],
    }))
    .unwrap();
    gate.record(&user);
    assert!(gate.context().eligible);
    gate.record(&user);
    assert_eq!(
        gate.context().pause_reason.as_deref(),
        Some("user_input_changed")
    );
    assert!(!gate.context().eligible);
}

#[test]
fn terminal_outcomes_leave_room_for_a_user_facing_reply() {
    let mut goal = snapshot();
    for status in [Status::Complete, Status::Paused] {
        goal.status = status;
        assert!(
            outcome(&goal)
                .follow_up_prompt()
                .unwrap()
                .contains("turn remains open")
        );
    }
    assert!(Outcome::None.follow_up_prompt().is_none());
}

#[test]
fn recovery_invalidates_a_pass_followed_by_new_tool_evidence() {
    let id = chaos_ipc::ProcessId::new();
    let mut complete = snapshot();
    complete.status = Status::Complete;
    let items = vec![
        record(id, &complete),
        RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput {
            call_id: "later".into(),
            tool_name: Some("exec_command".into()),
            output: FunctionCallOutputPayload::from_text("changed the artifact".into()),
        }),
    ];
    let mut journal = journal::Journal::default();
    journal.restore(id, &items).unwrap();
    journal.bind("driver", "stdio:test", 1, true).unwrap();
    assert!(journal.interrupted);
    let mut paused = complete;
    paused.revision += 1;
    paused.status = Status::Paused;
    assert!(journal.validate(&gate(), "driver", 1, &paused).is_ok());
}
