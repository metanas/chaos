use std::collections::HashMap;

use serde_json::json;

use super::*;
use crate::protocol::ClientCapabilities;
use crate::protocol::ServerCapabilities;
use crate::protocol::ToolInfo;

fn listing(meta: Value) -> Result<ResourceInfo, GuestError> {
    Ok(serde_json::from_value(json!({
        "uri": "ui://example/view",
        "name": "view",
        "mimeType": MIME_TYPE,
        "_meta": meta,
    }))?)
}

fn contents(meta: Value) -> Result<ResourceContents, GuestError> {
    Ok(serde_json::from_value(json!({
        "uri": "ui://example/view",
        "mimeType": MIME_TYPE,
        "text": "<!doctype html><p>Hello</p>",
        "_meta": meta,
    }))?)
}

#[test]
fn client_apps_support_is_opt_in_and_preserves_other_extensions() -> Result<(), GuestError> {
    let mut capabilities = ClientCapabilities::default();
    assert!(!capabilities.supports_mcp_apps());
    assert_eq!(serde_json::to_value(&capabilities)?, json!({}));
    capabilities.extensions = Some(HashMap::from([(
        "vendor/extension".to_string(),
        json!({"version": 1}),
    )]));
    let capabilities = capabilities.with_mcp_apps();
    assert!(capabilities.supports_mcp_apps());
    assert_eq!(
        capabilities.extension("vendor/extension"),
        Some(&json!({"version": 1}))
    );
    let wire = serde_json::to_value(&capabilities)?;
    assert_eq!(
        wire["extensions"][EXTENSION_ID],
        json!({"mimeTypes": [MIME_TYPE]})
    );
    let host: mcp_host::protocol::capabilities::ClientCapabilities = serde_json::from_value(wire)?;
    assert!(host.supports_mcp_apps());
    Ok(())
}

#[test]
fn server_extensions_round_trip_separately_from_unknown_capabilities() -> Result<(), GuestError> {
    let wire = json!({
        "extensions": {
            EXTENSION_ID: {"mimeTypes": [MIME_TYPE], "vendor/setting": true},
            "vendor/extension": {"enabled": false}
        },
        "vendor/capability": {"version": 3}
    });
    let capabilities: ServerCapabilities = serde_json::from_value(wire.clone())?;
    assert!(capabilities.supports_mcp_apps());
    assert!(!capabilities.extra.contains_key("extensions"));
    assert_eq!(
        capabilities.extra["vendor/capability"],
        wire["vendor/capability"]
    );
    assert_eq!(serde_json::to_value(capabilities)?, wire);
    Ok(())
}

#[test]
fn malformed_or_experimental_declarations_are_not_apps_support() -> Result<(), GuestError> {
    for settings in [
        json!(true),
        json!({}),
        json!({"mimeTypes": MIME_TYPE}),
        json!({"mimeTypes": ["text/html"]}),
        json!({"mimeTypes": [42]}),
    ] {
        let capabilities: ServerCapabilities = serde_json::from_value(json!({
            "extensions": {EXTENSION_ID: settings}
        }))?;
        assert!(!capabilities.supports_mcp_apps());
    }
    let capabilities: ClientCapabilities = serde_json::from_value(json!({
        "experimental": {EXTENSION_ID: {"mimeTypes": [MIME_TYPE]}}
    }))?;
    assert!(!capabilities.supports_mcp_apps());
    Ok(())
}

#[test]
fn host_tool_ui_is_consumed_without_losing_raw_metadata() -> Result<(), GuestError> {
    let mut meta = mcp_host::protocol::apps::ToolUi::new("ui://example/view")
        .map_err(|error| GuestError::Protocol(error.to_string()))?
        .with_visibility([UiVisibility::App])
        .into_meta();
    meta.insert("vendor/data".to_string(), json!({"opaque": true}));
    let tool: ToolInfo = serde_json::from_value(json!({
        "name": "view",
        "inputSchema": {"type": "object"},
        "_meta": meta
    }))?;
    let ui = tool
        .ui()?
        .ok_or_else(|| GuestError::Protocol("missing ui".to_string()))?;
    assert_eq!(ui.resource_uri.as_deref(), Some("ui://example/view"));
    assert!(ui.is_visible_to(UiVisibility::App));
    assert!(!ui.is_visible_to(UiVisibility::Model));
    assert_eq!(serde_json::to_value(tool)?["_meta"], json!(meta));
    Ok(())
}

#[test]
fn visibility_only_and_default_visibility_are_consumed() -> Result<(), GuestError> {
    let ui = tool_ui(Some(&json!({"ui": {"visibility": ["model"]}})))?
        .ok_or_else(|| GuestError::Protocol("missing ui".to_string()))?;
    assert!(ui.resource_uri.is_none());
    assert!(ui.is_visible_to(UiVisibility::Model));
    assert!(!ui.is_visible_to(UiVisibility::App));

    let ui = ToolUi::default();
    assert!(ui.is_visible_to(UiVisibility::Model));
    assert!(ui.is_visible_to(UiVisibility::App));
    let ui = ToolUi {
        visibility: Some(vec![]),
        ..ui
    };
    assert!(!ui.is_visible_to(UiVisibility::Model));
    assert!(!ui.is_visible_to(UiVisibility::App));
    assert!(tool_ui(Some(&json!({"vendor/data": 42})))?.is_none());
    Ok(())
}

#[test]
fn tool_ui_rejects_invalid_uri_or_visibility() {
    for uri in [
        "https://example.test/view",
        "file:///tmp/view",
        "ui://",
        "ui://bad view",
        "ui://bad\nview",
        "ui://é",
    ] {
        assert!(tool_ui(Some(&json!({"ui": {"resourceUri": uri}}))).is_err());
    }
    for ui in [
        json!({"visibility": ["admin"]}),
        json!({"visibility": "app"}),
        json!({"resourceUri": 42}),
        json!(null),
    ] {
        assert!(tool_ui(Some(&json!({"ui": ui}))).is_err());
    }
}

#[test]
fn ui_parsers_reject_non_object_metadata() {
    for meta in [json!(true), json!(42), json!("invalid"), json!([])] {
        assert!(tool_ui(Some(&meta)).is_err());
        assert!(resource_ui(Some(&meta)).is_err());
    }
}

#[test]
fn host_app_contents_preserve_csp_permissions_and_unknown_metadata() -> Result<(), GuestError> {
    let ui = UiResourceMeta {
        csp: Some(UiCsp {
            connect_domains: Some(vec!["wss://api.example.test".to_string()]),
            resource_domains: Some(vec!["https://cdn.example.test".to_string()]),
            ..Default::default()
        }),
        permissions: Some(UiPermissions {
            clipboard_write: Some(Default::default()),
            ..Default::default()
        }),
        domain: Some("https://sandbox.example.test".to_string()),
        prefers_border: Some(true),
    };
    let mut meta = ui
        .clone()
        .into_meta()
        .map_err(|error| GuestError::Protocol(error.to_string()))?;
    meta.insert("vendor/data".to_string(), json!([1, 2, 3]));
    let host =
        mcp_host::content::resource::ResourceContent::mcp_app("ui://example/view", "<p>Hello</p>")
            .with_meta(meta);
    let wire = serde_json::to_value(host)?;
    let contents: ResourceContents = serde_json::from_value(wire.clone())?;
    assert!(contents.is_mcp_app());
    let view = UiResource::from_contents(contents, None)?;
    assert_eq!(view.ui, ui);
    assert_eq!(view.html()?, "<p>Hello</p>");
    assert_eq!(serde_json::to_value(view.contents)?, wire);
    Ok(())
}

#[test]
fn content_ui_replaces_listing_ui_instead_of_merging_permissions() -> Result<(), GuestError> {
    let listing = listing(json!({"ui": {
        "csp": {"connectDomains": ["https://listing.example.test"]},
        "permissions": {"camera": {}},
        "prefersBorder": true
    }}))?;
    let contents = contents(json!({"ui": {"prefersBorder": false}}))?;
    let view = UiResource::from_contents(contents, Some(&listing))?;
    assert_eq!(view.ui.prefers_border, Some(false));
    assert!(view.ui.csp.is_none());
    assert!(view.ui.permissions.is_none());
    Ok(())
}

#[test]
fn listing_ui_is_used_only_when_content_has_no_ui() -> Result<(), GuestError> {
    let listing = listing(json!({"ui": {
        "csp": {"connectDomains": ["https://listing.example.test"]},
        "prefersBorder": true
    }}))?;
    let view = UiResource::from_contents(contents(json!({"vendor/data": 42}))?, Some(&listing))?;
    assert_eq!(Some(view.ui), listing.ui()?);
    let view = UiResource::from_contents(contents(json!({"ui": {}}))?, Some(&listing))?;
    assert_eq!(view.ui, UiResourceMeta::default());
    Ok(())
}

#[test]
fn absent_ui_metadata_defaults_to_no_permissions_or_network() -> Result<(), GuestError> {
    let view = UiResource::from_contents(contents(json!({}))?, None)?;
    assert_eq!(view.ui, UiResourceMeta::default());
    Ok(())
}

#[test]
fn malformed_content_policy_cannot_fall_back_to_valid_listing() -> Result<(), GuestError> {
    let listing = listing(json!({"ui": {"prefersBorder": true}}))?;
    for ui in [
        json!(null),
        json!({"csp": "allow-all"}),
        json!({"csp": {"connectDomains": ["javascript:evil"]}}),
        json!({"csp": {"resourceDomains": ["https://cdn.test; script-src *"]}}),
    ] {
        assert!(UiResource::from_contents(contents(json!({"ui": ui}))?, Some(&listing)).is_err());
    }
    Ok(())
}

#[test]
fn authoritative_content_policy_does_not_parse_overridden_listing() -> Result<(), GuestError> {
    let listing = listing(json!({"ui": {"csp": {"connectDomains": ["invalid"]}}}))?;
    let view = UiResource::from_contents(contents(json!({"ui": {}}))?, Some(&listing))?;
    assert_eq!(view.ui, UiResourceMeta::default());
    assert!(UiResource::from_contents(contents(json!({}))?, Some(&listing)).is_err());
    Ok(())
}

#[test]
fn listing_uri_must_match_contents() -> Result<(), GuestError> {
    let mut listing = listing(json!({}))?;
    listing.uri = "ui://different/view".to_string();
    assert!(UiResource::from_contents(contents(json!({}))?, Some(&listing)).is_err());
    Ok(())
}

#[test]
fn non_apps_resources_are_not_treated_as_views() -> Result<(), GuestError> {
    for (uri, mime) in [
        ("file:///tmp/view", MIME_TYPE),
        ("ui://example/view", "text/html"),
        ("ui://example/view", "text/plain"),
    ] {
        let contents: ResourceContents = serde_json::from_value(json!({
            "uri": uri, "mimeType": mime, "text": "<p>Hello</p>"
        }))?;
        assert!(!contents.is_mcp_app());
        assert!(UiResource::from_contents(contents, None).is_err());
    }
    Ok(())
}

#[test]
fn blob_views_decode_base64_utf8_and_preserve_raw_contents() -> Result<(), GuestError> {
    let html = "<p>こんにちは</p>";
    let blob = base64::engine::general_purpose::STANDARD.encode(html);
    let wire = json!({"uri": "ui://example/view", "mimeType": MIME_TYPE, "blob": blob});
    let contents: ResourceContents = serde_json::from_value(wire.clone())?;
    let view = UiResource::from_contents(contents, None)?;
    assert_eq!(view.html()?, html);
    assert_eq!(serde_json::to_value(view.contents)?, wire);
    Ok(())
}

#[test]
fn blob_views_reject_invalid_base64_or_non_utf8() -> Result<(), GuestError> {
    for blob in [
        "not-base64!".to_string(),
        base64::engine::general_purpose::STANDARD.encode([0xff]),
    ] {
        let contents: ResourceContents = serde_json::from_value(json!({
            "uri": "ui://example/view", "mimeType": MIME_TYPE, "blob": blob
        }))?;
        let view = UiResource::from_contents(contents, None)?;
        assert!(view.html().is_err());
    }
    Ok(())
}
