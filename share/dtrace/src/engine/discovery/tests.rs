use std::fs;
use std::path::Path;

use chaos_ipc::api::ConfigLayerSource;
use chaos_sysctl::ConfigLayerEntry;
use chaos_sysctl::ConfigLayerStack;
use tempfile::tempdir;

use super::discover_handlers;

fn write_hooks(folder: &Path, label: &str) -> anyhow::Result<()> {
    fs::create_dir_all(folder)?;
    let mut hooks = serde_json::Map::new();
    for event in ["SessionStart", "BeforeTurn", "Stop"] {
        hooks.insert(
            event.into(),
            serde_json::json!([{"hooks": [{"type": "command", "command": format!("{label}-{event}")}]}]),
        );
    }
    fs::write(
        folder.join("hooks.json"),
        serde_json::to_vec(&serde_json::json!({"hooks": hooks}))?,
    )?;
    Ok(())
}

#[test]
fn discovers_global_hooks_after_settings_migration_without_enabling_untrusted_projects()
-> anyhow::Result<()> {
    let temp = tempdir()?;
    let system = temp.path().join("system");
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    let untrusted = temp.path().join("untrusted");
    for (folder, label) in [
        (&system, "system"),
        (&home, "global"),
        (&project, "project"),
        (&untrusted, "untrusted"),
    ] {
        write_hooks(folder, label)?;
    }
    // Both forms must discover the same global file, once, in the same order.
    for migrated in [false, true] {
        let empty = || serde_json::from_str("{}");
        let mut layers = vec![ConfigLayerEntry::new(
            ConfigLayerSource::System {
                file: system.join("config.toml").try_into()?,
            },
            empty()?,
        )];
        if migrated {
            layers.push(ConfigLayerEntry::new(
                ConfigLayerSource::Bootstrap {
                    file: home.join("config.toml").try_into()?,
                },
                empty()?,
            ));
            layers.push(ConfigLayerEntry::new(
                ConfigLayerSource::UserDatabase { revision: 1 },
                empty()?,
            ));
        } else {
            layers.push(ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: home.join("config.toml").try_into()?,
                },
                empty()?,
            ));
        }
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::Project {
                dot_codex_folder: project.clone().try_into()?,
            },
            empty()?,
        ));
        layers.push(ConfigLayerEntry::new_disabled(
            ConfigLayerSource::Project {
                dot_codex_folder: untrusted.clone().try_into()?,
            },
            empty()?,
            "not trusted",
        ));
        let stack = ConfigLayerStack::new(layers, Default::default(), Default::default())?;
        let result = discover_handlers(Some(&stack));
        assert!(result.warnings.is_empty());
        let actual: Vec<_> = result.handlers.iter().map(|h| h.command.as_str()).collect();
        assert_eq!(
            actual,
            [
                "system-SessionStart",
                "system-BeforeTurn",
                "system-Stop",
                "global-SessionStart",
                "global-BeforeTurn",
                "global-Stop",
                "project-SessionStart",
                "project-BeforeTurn",
                "project-Stop",
            ],
            "migrated={migrated}",
        );
        for (index, handler) in result.handlers.iter().enumerate() {
            assert_eq!(handler.display_order, index as i64);
        }
    }
    Ok(())
}
