use super::*;
use chaos_ipc::ProcessId;

#[test]
fn only_interactive_exec_and_spawned_children_may_clamp() {
    assert!(clamp_eligible_source(&SessionSource::Cli));
    assert!(clamp_eligible_source(&SessionSource::Exec));
    assert!(clamp_eligible_source(&SessionSource::SubAgent(
        SubAgentSource::ProcessSpawn {
            parent_process_id: ProcessId::new(),
            depth: 1,
            agent_nickname: None,
            agent_role: None,
        }
    )));
    for source in [
        SessionSource::SubAgent(SubAgentSource::Review),
        SessionSource::SubAgent(SubAgentSource::Compact),
        SessionSource::SubAgent(SubAgentSource::MemoryConsolidation),
        SessionSource::Mcp,
        SessionSource::Api,
        SessionSource::VSCode,
    ] {
        assert!(!clamp_eligible_source(&source), "{source:?} must not clamp");
    }
}
