//! Client harness identification for telemetry: normalized ids from MCP `clientInfo.name` or the process environment.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Environment variables whose presence marks an Antigravity-hosted process (populated alongside setup support).
pub const ANTIGRAVITY_ENV_MARKERS: &[&str] = &[];

/// Normalized client harness identifier. Raw `clientInfo.name` values are never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessId {
    ClaudeCode,
    Antigravity,
    Cursor,
    Codex,
    GeminiCli,
    Terminal,
    Other,
}

impl HarnessId {
    /// Stable string form, identical to the serde representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            HarnessId::ClaudeCode => "claude-code",
            HarnessId::Antigravity => "antigravity",
            HarnessId::Cursor => "cursor",
            HarnessId::Codex => "codex",
            HarnessId::GeminiCli => "gemini-cli",
            HarnessId::Terminal => "terminal",
            HarnessId::Other => "other",
        }
    }
}

/// Maps an MCP `clientInfo.name` to a [`HarnessId`] by case-insensitive substring; unknown or missing names yield `Other`.
pub fn harness_from_client_name(name: Option<&str>) -> HarnessId {
    let Some(name) = name else {
        return HarnessId::Other;
    };
    let n = name.to_ascii_lowercase();
    if n.contains("claude") {
        HarnessId::ClaudeCode
    } else if n.contains("antigravity") || n.contains("jetski") || n.contains("agy") {
        HarnessId::Antigravity
    } else if n.contains("cursor") {
        HarnessId::Cursor
    } else if n.contains("codex") {
        HarnessId::Codex
    } else if n.contains("gemini") {
        HarnessId::GeminiCli
    } else {
        HarnessId::Other
    }
}

/// Detects the harness of a CLI process from environment markers; `vars(name)` reports whether a variable is set.
pub fn harness_from_env(vars: impl Fn(&str) -> bool) -> HarnessId {
    if vars("CLAUDECODE") {
        HarnessId::ClaudeCode
    } else if ANTIGRAVITY_ENV_MARKERS.iter().any(|m| vars(m)) {
        HarnessId::Antigravity
    } else {
        HarnessId::Terminal
    }
}
