use super::*;
use serde::Deserialize;
use std::assert_matches;

#[derive(Deserialize, Debug, PartialEq)]
struct TuiTomlTest {
    #[serde(default)]
    notifications: Notifications,
    #[serde(default)]
    notification_method: NotificationMethod,
}

#[derive(Deserialize, Debug, PartialEq)]
struct RootTomlTest {
    tui: TuiTomlTest,
}

#[test]
fn test_tui_notifications_true() {
    let toml = r#"
            [tui]
            notifications = true
        "#;
    let parsed: RootTomlTest = toml::from_str(toml).expect("deserialize notifications=true");
    assert_matches!(parsed.tui.notifications, Notifications::Enabled(true));
}

#[test]
fn test_tui_notifications_custom_array() {
    let toml = r#"
            [tui]
            notifications = ["foo"]
        "#;
    let parsed: RootTomlTest = toml::from_str(toml).expect("deserialize notifications=[\"foo\"]");
    assert_matches!(
        parsed.tui.notifications,
        Notifications::Custom(ref v) if v == &vec!["foo".to_string()]
    );
}

#[test]
fn test_tui_notification_method() {
    let toml = r#"
            [tui]
            notification_method = "bel"
        "#;
    let parsed: RootTomlTest =
        toml::from_str(toml).expect("deserialize notification_method=\"bel\"");
    assert_eq!(parsed.tui.notification_method, NotificationMethod::Bel);
}

#[test]
fn tui_output_requires_frontend_opt_in_not_saved_tui_settings() {
    let home = tempfile::tempdir().unwrap();
    let saved: ConfigToml = toml::from_str("[tui]\nanimations = true").unwrap();
    let config = Config::load_from_base_config_with_overrides(
        saved.clone(),
        ConfigOverrides::default(),
        home.path().to_path_buf(),
    )
    .unwrap();
    assert!(!config.tui_output);
    let config = Config::load_from_base_config_with_overrides(
        saved,
        ConfigOverrides {
            tui_output: true,
            ..Default::default()
        },
        home.path().to_path_buf(),
    )
    .unwrap();
    assert!(config.tui_output);
}
