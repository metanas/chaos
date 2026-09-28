//! Explicit legacy import only. Runtime hook discovery never reads files.
use crate::engine::config::{HookHandlerConfig, HooksFile};
use chaos_ipc::hooks::HookDefinition;
use chaos_ipc::protocol::HookEventName;

pub fn parse_legacy_hooks(contents: &str) -> anyhow::Result<Vec<HookDefinition>> {
    let file: HooksFile = serde_json::from_str(contents)?;
    let mut definitions = Vec::new();
    for (event, groups) in [
        (HookEventName::SessionStart, file.hooks.session_start),
        (HookEventName::BeforeTurn, file.hooks.before_turn),
        (HookEventName::Stop, file.hooks.stop),
    ] {
        for group in groups {
            for handler in group.hooks {
                let HookHandlerConfig::Command {
                    command,
                    timeout_sec,
                    r#async,
                    status_message,
                } = handler
                else {
                    anyhow::bail!("only command hooks can be imported");
                };
                anyhow::ensure!(!r#async, "async hooks are not supported");
                definitions.push(HookDefinition {
                    event,
                    command,
                    project: None,
                    matcher: group.matcher.clone(),
                    timeout_sec: timeout_sec.unwrap_or(600),
                    order: definitions.len() as i64,
                    status_message,
                });
            }
        }
    }
    anyhow::ensure!(
        !definitions.is_empty() && definitions.len() <= 128,
        "import requires 1..128 hooks"
    );
    Ok(definitions)
}

#[cfg(test)]
mod tests {
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
}
