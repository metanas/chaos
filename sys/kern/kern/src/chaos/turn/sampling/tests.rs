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
