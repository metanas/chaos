use super::*;

#[tokio::test]
async fn absent_storage_never_acknowledges_or_publishes_a_goal() {
    let (session, _, _) = crate::chaos::make_session_and_context_with_rx().await;
    assert!(session.services.rollout.lock().await.is_none());
    let mut journal = journal::Journal::default();
    assert!(
        journal
            .commit(&session, "driver", "stdio:test", snapshot(), None)
            .await
            .is_err()
    );
    assert!(journal.goal.is_none());
    assert!(journal.reconcile(&session).await.is_err());
    assert!(journal.goal.is_none());
}

#[tokio::test]
async fn lost_ack_retry_is_idempotent_and_conflicts_cannot_overwrite() {
    let (session, _, _) = crate::chaos::make_session_and_context_with_rx().await;
    let goal = snapshot();
    recover(&session, &[record(session.conversation_id, &goal)])
        .await
        .unwrap();
    request(
        &session,
        "driver".into(),
        "stdio:test".into(),
        2,
        json!({"version":VERSION,"operation":"restore"}),
    )
    .await
    .unwrap();
    let duplicate =
        json!({"version":VERSION,"operation":"checkpoint","expected_revision":0,"goal":goal});
    assert_eq!(
        request(&session, "driver".into(), "stdio:test".into(), 2, duplicate)
            .await
            .unwrap(),
        json!(goal)
    );
    let mut conflict = goal.clone();
    conflict.objective = "weaker".into();
    assert!(request(&session, "driver".into(), "stdio:test".into(), 2,
        json!({"version":VERSION,"operation":"checkpoint","expected_revision":0,"goal":conflict})).await.is_err());
    assert_eq!(
        session.state.lock().await.goal.journal.lock().await.goal,
        Some(goal)
    );
}

#[tokio::test]
async fn restored_attempt_cannot_be_evaluated_in_a_new_or_missing_turn() {
    let (session, _, _) = crate::chaos::make_session_and_context_with_rx().await;
    let mut goal = snapshot();
    goal.status = Status::Checking;
    goal.checks = 2;
    goal.attempt_id = Some("attempt-2".into());
    goal.evidence_digest = Some("digest".into());
    recover(&session, &[record(session.conversation_id, &goal)])
        .await
        .unwrap();
    request(
        &session,
        "driver".into(),
        "stdio:test".into(),
        1,
        json!({"version":VERSION,"operation":"restore"}),
    )
    .await
    .unwrap();
    assert!(
        request(
            &session,
            "driver".into(),
            "stdio:test".into(),
            1,
            json!({"version":VERSION,"operation":"evaluate","revision":1,"claim":"done"})
        )
        .await
        .is_err()
    );
    assert_eq!(
        session
            .state
            .lock()
            .await
            .goal
            .journal
            .lock()
            .await
            .goal
            .as_ref()
            .unwrap()
            .checks,
        2
    );
}
