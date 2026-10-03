use super::model_presets_from_init;

#[test]
fn presets_come_from_init_values_and_skip_malformed_entries() {
    let init = serde_json::json!([
        {"value": "default", "displayName": "Default"},
        {"value": "haiku", "displayName": "Haiku", "supportedEffortLevels": ["low", "high", "max"]},
        {"displayName": "no value"}
    ]);
    let presets = model_presets_from_init(&init);
    let values: Vec<&str> = presets.iter().map(|p| p.model.as_str()).collect();
    assert_eq!(values, vec!["default", "haiku"]);
    assert!(presets[0].is_default);
    assert_eq!(presets[1].supported_reasoning_efforts.len(), 2);
}

#[test]
fn non_array_init_yields_no_presets() {
    assert!(model_presets_from_init(&serde_json::json!({"models": 1})).is_empty());
}
