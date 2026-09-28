use super::*;

#[test]
fn database_snapshot_requires_activation_and_preserves_order() -> anyhow::Result<()> {
    let make = |id: &str, enabled, approved, project| -> anyhow::Result<HookRegistration> {
        Ok(HookRegistration {
            id: id.into(),
            revision: 1,
            enabled,
            approved,
            definition: serde_json::from_value(serde_json::json!({
                "event": "before_turn", "command": id, "project": project
            }))?,
        })
    };
    let result = discover_handlers(&[
        make("project", true, true, Some("/project"))?,
        make("disabled", false, true, None)?,
        make("pending", true, false, None)?,
        make("global", true, true, None)?,
    ]);
    assert!(result.warnings.is_empty());
    assert_eq!(
        result
            .handlers
            .iter()
            .map(|h| h.command.as_str())
            .collect::<Vec<_>>(),
        ["global", "project"]
    );
    assert_eq!(
        result.handlers[0].source_path.to_str(),
        Some("chaos://hooks/global")
    );
    Ok(())
}
