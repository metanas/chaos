use std::collections::HashMap;
use std::collections::VecDeque;

use chaos_ipc::models::ResponseItem;
use serde_json::Value;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;

fn is_goal_tool(name: &str) -> bool {
    ["start_goal", "get_goal", "check_goal", "pause_goal"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

/// Host observations, independent of compaction and never trusted proof.
#[derive(Default)]
pub(super) struct Evidence {
    calls: HashMap<String, (String, String)>,
    entries: VecDeque<Value>,
    pub(super) truncated: bool,
}

impl Evidence {
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn entries(&self) -> &VecDeque<Value> {
        &self.entries
    }

    pub(super) fn digest(&self) -> String {
        Sha256::digest(serde_json::to_vec(&self.entries).unwrap_or_default())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub(super) fn record(&mut self, item: &ResponseItem) -> bool {
        match item {
            ResponseItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            }
            | ResponseItem::CustomToolCall {
                call_id,
                name,
                input: arguments,
                ..
            } => {
                if self.calls.len() >= 128 {
                    self.calls.clear();
                    self.truncated = true;
                }
                let goal_resource = name.ends_with("read_mcp_resource")
                    && serde_json::from_str::<Value>(arguments)
                        .is_ok_and(|v| v["uri"] == "goal://current");
                let arguments = if is_goal_tool(name) || goal_resource {
                    // Claims must not count as evidence or truncation.
                    String::new()
                } else {
                    self.truncated |= arguments.len() > 2048;
                    super::bounded(arguments, 2048)
                };
                self.calls.insert(
                    call_id.clone(),
                    (
                        if goal_resource {
                            "get_goal".into()
                        } else {
                            name.clone()
                        },
                        arguments,
                    ),
                );
                false
            }
            ResponseItem::FunctionCallOutput {
                call_id,
                output,
                tool_name,
            }
            | ResponseItem::CustomToolCallOutput {
                call_id,
                output,
                tool_name,
            } => {
                let call = self.calls.remove(call_id);
                let name = call
                    .as_ref()
                    .map(|(name, _)| name.as_str())
                    .or(tool_name.as_deref());
                let Some(name) = name else { return false };
                // Goal status and the worker's claims are never evidence.
                if is_goal_tool(name) {
                    return false;
                }
                let Some(text) = output.body.to_text() else {
                    return false;
                };
                self.truncated |= text.len() > 6144;
                let entry = json!({
                    "tool":name, "arguments":call.as_ref().map(|(_, args)| args),
                    "output":super::bounded(&text, 6144), "success":output.success,
                });
                // Call IDs and repeated identical observations are not progress.
                if self.entries.contains(&entry) {
                    return false;
                }
                self.entries.push_back(entry);
                while self.entries.len() > 24
                    || serde_json::to_vec(&self.entries).is_ok_and(|bytes| bytes.len() > 49_152)
                {
                    self.entries.pop_front();
                    self.truncated = true;
                }
                true
            }
            _ => false,
        }
    }
}
