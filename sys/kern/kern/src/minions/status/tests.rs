use super::*;

#[test]
fn empty_completion_preserves_failure_but_next_turn_resets() {
    for failure in [
        AgentStatus::Errored("provider failed".into()),
        AgentStatus::Interrupted,
    ] {
        assert_eq!(
            transition_agent_status(&failure, AgentStatus::Completed(None)),
            failure
        );
        assert_eq!(
            transition_agent_status(&failure, AgentStatus::Running),
            AgentStatus::Running
        );
    }
    assert_eq!(
        transition_agent_status(&AgentStatus::Running, AgentStatus::Completed(None)),
        AgentStatus::Completed(None),
    );
}

#[test]
fn lifecycle_transition_matrix() {
    let states = [
        AgentStatus::PendingInit,
        AgentStatus::Running,
        AgentStatus::Completed(None),
        AgentStatus::Completed(Some("result".into())),
        AgentStatus::Interrupted,
        AgentStatus::Errored("failure".into()),
        AgentStatus::Shutdown,
        AgentStatus::NotFound,
    ];
    for current in &states {
        for next in &states {
            let rejected = matches!(current, AgentStatus::Shutdown | AgentStatus::NotFound)
                || matches!(next, AgentStatus::PendingInit | AgentStatus::NotFound)
                || (matches!(next, AgentStatus::Completed(None))
                    && matches!(current, AgentStatus::Interrupted | AgentStatus::Errored(_)));
            assert_eq!(
                transition_agent_status(current, next.clone()),
                if rejected { current } else { next }.clone(),
                "{current:?} -> {next:?}"
            );
        }
    }
}
