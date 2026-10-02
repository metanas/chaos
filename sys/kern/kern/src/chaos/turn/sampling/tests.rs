use super::*;

#[test]
fn appends_labeled_mcp_server_instructions() {
    let mut base = chaos_ipc::models::BaseInstructions {
        text: "Base instructions.".to_string(),
    };

    append_mcp_server_instructions(
        &mut base,
        &[
            McpServerInstructions {
                server_name: "alpha & \"tools\"".to_string(),
                server_version: "1.2.3 & \"preview\"".to_string(),
                capabilities: serde_json::json!({
                    "resources": {"subscribe": true},
                    "tools": {"listChanged": false}
                }),
                instructions: "Use Vec<T> & preserve\n  ]]> literally.".to_string(),
            },
            McpServerInstructions {
                server_name: "beta".to_string(),
                server_version: "2.0.0".to_string(),
                capabilities: serde_json::json!({}),
                instructions: "Prefer beta resources.".to_string(),
            },
        ],
    )
    .unwrap();

    assert!(base.text.starts_with("Base instructions."));
    let xml = base.text.split_once("<mcp_server_instructions>").unwrap().1;
    assert_eq!(
        xml,
        r#"
  <server name="alpha &amp; &quot;tools&quot;" version="1.2.3 &amp; &quot;preview&quot;" capabilities="{&quot;resources&quot;:{&quot;subscribe&quot;:true},&quot;tools&quot;:{&quot;listChanged&quot;:false}}">
    <instructions><![CDATA[Use Vec<T> & preserve
  ]]]]><![CDATA[> literally.]]></instructions>
  </server>
  <server name="beta" version="2.0.0" capabilities="{}">
    <instructions><![CDATA[Prefer beta resources.]]></instructions>
  </server>
</mcp_server_instructions>"#
    );
}

#[test]
fn server_capability_xml_preserves_extensions_and_sorts_nested_objects() {
    let capabilities: serde_json::Value = serde_json::from_str(
        r#"{"vendor/extra":{"z":2,"a":1},"experimental":{"vendor/feature":{"z":[{"b":false,"a":true}],"a":"\"<&'>]]>\n"}}}"#,
    )
    .unwrap();
    let servers = [McpServerInstructions {
        server_name: "test".to_string(),
        server_version: "1.0.0".to_string(),
        capabilities: capabilities.clone(),
        instructions: "Use this server.".to_string(),
    }];
    let xml = McpInstructionsDocument::new(&servers).to_xml().unwrap();
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut server_count = 0;
    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Start(element) if element.name().as_ref() == "server" => {
                server_count += 1;
                let attribute = element.try_get_attribute("capabilities").unwrap().unwrap();
                let json = attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .unwrap();
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&json).unwrap(),
                    capabilities
                );
                assert_eq!(
                    json,
                    r#"{"experimental":{"vendor/feature":{"a":"\"<&'>]]>\n","z":[{"a":true,"b":false}]}},"vendor/extra":{"a":1,"z":2}}"#
                );
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    assert_eq!(server_count, 1);
}

#[test]
fn leaves_base_instructions_unchanged_without_mcp_instructions() {
    let mut base = chaos_ipc::models::BaseInstructions {
        text: "Base instructions.".to_string(),
    };

    append_mcp_server_instructions(&mut base, &[]).unwrap();

    assert_eq!(base.text, "Base instructions.");
}

#[test]
fn runtime_guidance_is_request_local_and_preserves_literal_base() {
    let original = vec![DeveloperInstructions::new("saved context").into()];
    let mut prompt = Prompt {
        input: original.clone(),
        tools: vec![crate::client_common::tools::ToolSpec::LocalShell {}],
        base_instructions: chaos_ipc::models::BaseInstructions {
            text: "literal {{ not_a_template }}".to_owned(),
        },
        ..Prompt::default()
    };
    let catalog = crate::tools::groups::build_catalog().unwrap();
    let state = catalog.new_state();
    append_runtime_instructions(
        &mut prompt,
        ToolGroupFilter {
            catalog: &catalog,
            state: &state,
        },
        ModeCapabilities::default(),
        &SessionSource::Api,
        false,
        ApprovalPolicy::Interactive,
    );
    assert_eq!(&prompt.input[..original.len()], original.as_slice());
    assert_eq!(prompt.input.len(), original.len() + 1);
    assert_eq!(
        prompt.base_instructions.text,
        "literal {{ not_a_template }}"
    );
    let message = serde_json::to_value(prompt.input.last().unwrap()).unwrap();
    assert_eq!(message["role"], "system");
}

#[cfg(feature = "tui")]
#[test]
fn headless_policy_suppresses_console_guidance() {
    let catalog = crate::tools::groups::build_catalog().unwrap();
    let state = catalog.new_state();
    for (policy, expected_messages) in [
        (ApprovalPolicy::Interactive, 1),
        (ApprovalPolicy::Headless, 0),
    ] {
        let mut prompt = Prompt::default();
        append_runtime_instructions(
            &mut prompt,
            ToolGroupFilter {
                catalog: &catalog,
                state: &state,
            },
            ModeCapabilities::default(),
            &SessionSource::Cli,
            true,
            policy,
        );
        assert_eq!(prompt.input.len(), expected_messages);
    }
}
