use super::*;
use chaos_ipc::protocol::TurnProgressEvent;

#[test]
fn model_catalog_responses_are_not_persisted() {
    let event = EventMsg::ListModelsResponse(chaos_ipc::protocol::ListModelsResponseEvent {
        models: Vec::new(),
    });
    for mode in [
        EventPersistenceMode::Limited,
        EventPersistenceMode::Extended,
    ] {
        assert!(!should_persist_event_msg(&event, mode));
    }
}

#[test]
fn turn_progress_is_not_persisted() {
    let event = EventMsg::TurnProgress(TurnProgressEvent {
        turn_id: "turn-1".to_string(),
        approx_reasoning_tokens: 10,
        approx_output_tokens: 5,
        approx_total_tokens: 15,
    });

    assert!(!should_persist_event_msg(
        &event,
        EventPersistenceMode::Extended
    ));
}
