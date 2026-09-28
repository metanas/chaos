//! Persisted hook definitions. Registration is not execution authorization.
use crate::protocol::HookEventName;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HookDefinition {
    pub event: HookEventName,
    pub command: String,
    /// None means global; otherwise a canonical project root.
    #[serde(default)]
    pub project: Option<PathBuf>,
    #[serde(default)]
    pub matcher: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_sec: u64,
    #[serde(default)]
    pub order: i64,
    #[serde(default)]
    pub status_message: Option<String>,
}

fn default_timeout() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookRegistration {
    pub id: String,
    pub revision: i64,
    pub definition: HookDefinition,
    pub enabled: bool,
    /// Computed from the current installation's approval, never accepted on writes.
    pub approved: bool,
}
