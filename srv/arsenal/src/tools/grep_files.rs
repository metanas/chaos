//! MCP tool: grep_files — local byte-regex content search.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use globset::Glob;
use ignore::WalkBuilder;
use mcp_host::prelude::*;
use regex::bytes::RegexBuilder;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::ChaosCtx;
use crate::ChaosServer;
use crate::tools::deserialize_tool_params;
use crate::tools::tool_json_result;

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 2000;
// Preserve the previous content-search bound, including if a file grows while
// being read. Paths-only search never needs to retain all file contents.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct GrepFilesParams {
    /// Regex pattern to search for.
    pattern: String,

    /// Optional glob filter for file names (e.g. "*.rs").
    #[serde(default)]
    include: Option<String>,

    /// Directory or file path to search in. Defaults to cwd.
    #[serde(default)]
    path: Option<String>,

    /// Maximum number of matching file paths to return.
    #[serde(default = "default_limit")]
    limit: usize,

    /// Case-sensitive matching when true, case-insensitive when false.
    /// Omit for smart case (case-insensitive unless the pattern contains uppercase).
    #[serde(default)]
    case_sensitive: Option<bool>,
}

impl ChaosServer {
    /// Search file contents with a regex and return matching file paths.
    #[mcp_tool(name = "grep_files", read_only = true, open_world = false)]
    async fn grep_files(
        &self,
        _ctx: ChaosCtx<'_>,
        params: Parameters<GrepFilesParams>,
    ) -> ToolResult {
        tool_json_result(execute_params_structured(params.0).await)
    }
}

/// Bridge for core's thin adapter — accepts raw JSON arguments.
pub async fn execute(arguments: &serde_json::Value) -> Result<String, String> {
    let params: GrepFilesParams = deserialize_tool_params(arguments)?;
    execute_params(params).await
}

pub async fn execute_structured(
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let params: GrepFilesParams = deserialize_tool_params(arguments)?;
    execute_params_structured(params).await
}

async fn execute_params(params: GrepFilesParams) -> Result<String, String> {
    let structured = execute_params_structured(params).await?;
    let matches = structured
        .get("matches")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if matches.is_empty() {
        Ok("No matches found.".to_string())
    } else {
        Ok(matches
            .into_iter()
            .filter_map(|value| value.as_str().map(ToString::to_string))
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

async fn execute_params_structured(params: GrepFilesParams) -> Result<serde_json::Value, String> {
    let pattern = params.pattern.trim();
    if pattern.is_empty() {
        return Err("pattern must not be empty".to_string());
    }

    if params.limit == 0 {
        return Err("limit must be greater than zero".to_string());
    }

    let limit = params.limit.min(MAX_LIMIT);

    let search_path = params.path.as_deref().unwrap_or(".");

    let search_path = Path::new(search_path);
    verify_path_exists(search_path).await?;

    let include = params
        .include
        .as_deref()
        .map(str::trim)
        .filter(|val| !val.is_empty());

    // Filesystem walking and regex search are sync — run on a blocking thread.
    let pattern = pattern.to_string();
    let search_path = search_path.to_path_buf();
    let include = include.map(String::from);

    let results = tokio::task::spawn_blocking(move || {
        run_grep_search(
            &pattern,
            include.as_deref(),
            &search_path,
            limit,
            params.case_sensitive,
        )
    })
    .await
    .map_err(|e| format!("search task failed: {e}"))??;

    let match_count = results.len();
    Ok(serde_json::json!({
        "matches": results,
        "match_count": match_count,
        "limit": limit,
    }))
}

async fn verify_path_exists(path: &Path) -> Result<(), String> {
    tokio::fs::metadata(path)
        .await
        .map_err(|err| format!("unable to access `{}`: {err}", path.display()))?;
    Ok(())
}

/// Search regular files with a byte regex.
///
/// Walks the directory respecting .gitignore, applies an optional glob filter,
/// searches each file for the pattern, collects matching file paths sorted by
/// modification time (newest first), and returns up to `limit` results.
/// `case_sensitive` overrides smart-case matching when provided.
pub fn run_grep_search(
    pattern: &str,
    include: Option<&str>,
    search_path: &Path,
    limit: usize,
    case_sensitive: Option<bool>,
) -> Result<Vec<String>, String> {
    let case_sensitive =
        case_sensitive.unwrap_or_else(|| pattern.chars().any(char::is_uppercase));
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .multi_line(true)
        .unicode(false)
        .build()
        .map_err(|e| format!("invalid regex pattern: {e}"))?;
    let include_matcher = include
        .map(|pattern| {
            Glob::new(pattern)
                .map(|glob| glob.compile_matcher())
                .map_err(|e| format!("invalid include glob: {e}"))
        })
        .transpose()?;
    let search_path = std::path::absolute(search_path)
        .map_err(|e| format!("unable to resolve search path: {e}"))?;
    std::fs::metadata(&search_path)
        .map_err(|e| format!("unable to access `{}`: {e}", search_path.display()))?;
    let mut walker = WalkBuilder::new(&search_path);
    walker
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .filter_entry(|entry| entry.file_name() != ".git");

    let mut files = Vec::new();
    for entry in walker.build() {
        // Ignore unreadable/vanished entries without losing other matches.
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path();
        if let Some(matcher) = &include_matcher {
            let match_path = if include.is_some_and(|pattern| pattern.contains('/')) {
                path.strip_prefix(&search_path).unwrap_or(path)
            } else {
                Path::new(entry.file_name())
            };
            if !matcher.is_match(match_path) {
                continue;
            }
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.len() == 0 || metadata.len() > MAX_FILE_SIZE {
            continue;
        }
        files.push((path.to_path_buf(), metadata.modified().ok()));
    }

    // Search newest files first so positive queries can stop at the limit
    // without reading every candidate. Path breaks modification-time ties.
    files.sort_by(|(left_path, left_time), (right_path, right_time)| {
        right_time.cmp(left_time).then_with(|| left_path.cmp(right_path))
    });
    let mut results = Vec::new();
    let mut contents = Vec::new();
    for (path, _) in files {
        if results.len() >= limit {
            break;
        }
        let Ok(file) = File::open(&path) else {
            continue;
        };
        if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
            continue;
        }
        contents.clear();
        if file.take(MAX_FILE_SIZE + 1).read_to_end(&mut contents).is_err()
            || contents.len() as u64 > MAX_FILE_SIZE
            || contents[..contents.len().min(8192)].contains(&0)
        {
            continue;
        }
        if regex.is_match(&contents) {
            results.push(path.to_string_lossy().into_owned());
        }
    }

    Ok(results)
}

/// Returns the auto-generated `ToolInfo` for schema extraction by core.
pub fn tool_info() -> mcp_host::prelude::ToolInfo {
    ChaosServer::grep_files_tool_info()
}

pub fn mount(
    router: mcp_host::registry::router::McpToolRouter<ChaosServer>,
) -> mcp_host::registry::router::McpToolRouter<ChaosServer> {
    router.with_tool(
        ChaosServer::grep_files_tool_info(),
        ChaosServer::grep_files_handler,
        None,
    )
}

#[cfg(test)]
#[path = "grep_files/tests.rs"]
mod tests;
