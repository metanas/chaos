use super::*;
use crate::representer::SessionRepresenter;
use chaos_ipc::models::ReasoningItemReasoningSummary;

#[test]
fn kimi_endpoint_matching_uses_exact_hosts_and_paths() {
    for base_url in [
        "https://api.moonshot.ai/v1",
        "https://api.moonshot.cn/v1/",
        "https://api.kimi.ai/coding/v1",
        "https://api.kimi.com/coding/v1/",
    ] {
        assert!(is_kimi_endpoint(base_url), "{base_url}");
    }
    for base_url in [
        "https://api.moonshot.ai.evil.test/v1",
        "https://api.moonshot.ai@evil.test/v1",
        "https://gateway.test/api.moonshot.ai/v1",
        "https://api.moonshot.ai/anthropic",
        "https://api.kimi.ai/v1",
        "not a URL",
    ] {
        assert!(!is_kimi_endpoint(base_url), "{base_url}");
    }
}

#[test]
fn kimi_preserves_plaintext_reasoning_but_not_foreign_encrypted_state() {
    let reasoning = ResponseItem::Reasoning {
        id: "rs_kimi".into(),
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: "Check the repository before editing.".into(),
        }],
        content: None,
        encrypted_content: None,
    };
    let message = ResponseItem::Message {
        id: None,
        role: "system".into(),
        content: vec![],
        end_turn: None,
        phase: None,
    };
    let items = vec![
        message.clone(),
        reasoning.clone(),
        ResponseItem::Reasoning {
            id: "rs_openai".into(),
            summary: vec![],
            content: None,
            encrypted_content: Some("foreign-state".into()),
        },
        ResponseItem::CompactionTrigger {},
        ResponseItem::Compaction {
            encrypted_content: "foreign-compaction".into(),
        },
    ];
    let representer = SessionRepresenter::for_compatible_endpoint("https://api.moonshot.ai/v1");
    assert_eq!(representer.represent(items), vec![message, reasoning]);
}
