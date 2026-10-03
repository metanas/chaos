use super::*;

#[test]
fn legacy_import_is_explicit_and_rejects_unsupported_handlers() {
    let text =
        r#"{"hooks":{"BeforeTurn":[{"hooks":[{"type":"command","command":"printf hello"}]}]}}"#;
    assert_eq!(parse_legacy_hooks(text).unwrap()[0].command, "printf hello");
    assert!(
        parse_legacy_hooks(&text.replace("\"command\",\"command\"", "\"agent\",\"command\""))
            .is_err()
    );
    assert!(parse_legacy_hooks(r#"{"hooks":{"Unknown":[]}}"#).is_err());
}
