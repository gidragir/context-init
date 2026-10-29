//! Parameter schemas and tool definitions with concise English descriptions (D20, R2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `context_check` tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpCheckParams {
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
    /// Pagination cursor for continuing output.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Result of `context_check` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpCheckResult {
    /// Indicates whether architectural drift was detected.
    pub has_drift: bool,
    /// Files or subsystems deleted on disk but present in the index.
    pub missing_in_fs: Vec<String>,
    /// New directories or files on disk missing from the index.
    pub untracked_in_knowledge: Vec<String>,
    /// Count of subsystems verified up-to-date.
    pub up_to_date_count: usize,
    /// Indicates whether underlying OKF cards are stale relative to Git.
    pub git_stale: bool,
    /// Human-readable summary message.
    pub message: String,
}

/// Parameters for `context_get_subsystems` tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpGetSubsystemsParams {
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
    /// Pagination cursor for continuing output.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Parameters for `context_get_subsystem` tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpGetSubsystemParams {
    /// Identifier of the subsystem to retrieve.
    pub subsystem_id: String,
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
    /// Pagination cursor for continuing output.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Parameters for `context_pack` tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpPackParams {
    /// Identifier of the subsystem, or 'all' to pack entire project.
    pub subsystem_id: String,
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
    /// Compress AST by stripping function bodies (default: true).
    #[serde(default)]
    pub compress: Option<bool>,
    /// Output format: 'xml' or 'markdown' (default: 'xml').
    #[serde(default)]
    pub style: Option<String>,
    /// Pagination cursor for continuing output.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Parameters for `context_submit` tool (D14, D29, R4).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpSubmitParams {
    /// Work package identifier issued for this task.
    pub package_id: String,
    /// Completed work payload conforming to the role JSON schema.
    pub submission: serde_json::Value,
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
}

/// Result of `context_submit` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubmitResult {
    /// Indicates whether the submission passed JSON schema validation.
    pub valid: bool,
    /// Work package identifier.
    pub package_id: String,
    /// Role for which the work package was evaluated.
    pub role: String,
    /// Structured list of validation errors if validation failed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    /// Human-readable validation summary.
    pub message: String,
}

/// Parameters for `context_feedback` tool (D29, R4).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpFeedbackParams {
    /// Name of the tool being evaluated.
    pub tool: String,
    /// Feedback verdict: helpful, unhelpful, incorrect, or stale.
    pub verdict: String,
    /// Optional contextual notes explaining the verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
}

/// Result of `context_feedback` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackResult {
    /// Status confirmation ('ok').
    pub status: String,
    /// Tool evaluated.
    pub tool: String,
    /// Verdict recorded.
    pub verdict: String,
    /// Indicates whether telemetry sink successfully recorded the signal.
    pub recorded: bool,
}

/// Parameters for `context_init` tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpInitParams {
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
    /// Perform deep AST inspection.
    #[serde(default)]
    pub deep: Option<bool>,
    /// Model alias overriding the `init` role chain (D16).
    #[serde(default)]
    pub model: Option<String>,
    /// Overwrite existing knowledge base.
    #[serde(default)]
    pub force: Option<bool>,
    /// Generate root AGENTS.md file.
    #[serde(default)]
    pub with_agents_md: Option<bool>,
}

/// Parameters for `context_update` tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpUpdateParams {
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
    /// Perform deep AST inspection.
    #[serde(default)]
    pub deep: Option<bool>,
    /// Model alias overriding the `update` role chain (D16).
    #[serde(default)]
    pub model: Option<String>,
}

/// Parameters for `context_scan` tool (D9).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpScanParams {
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
    /// Pagination cursor for continuing output.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Parameters for `context_workspace` tool (D6).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpWorkspaceParams {
    /// Workspace directory containing several Git repositories (default: the resolved root).
    #[serde(default)]
    pub path: Option<String>,
    /// Pagination cursor for continuing output.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Parameters for `context_task` tool (D4, D5).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpTaskParams {
    /// Specific file paths or subsystems relevant to the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    /// Repository path or identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Path to project root directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Base Git reference to compute diff against (e.g. main, HEAD~1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_ref: Option<String>,
    /// L2 drill-down: a method or metadata object name; returns only its changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Pagination cursor for continuing output.
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Parameters for `context_search` tool (D18).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpSearchParams {
    /// Search query terms or symbols.
    pub query: String,
    /// Path to project root directory.
    #[serde(default)]
    pub path: Option<String>,
    /// Project identifier or directory path.
    #[serde(default)]
    pub project: Option<String>,
    /// Pagination cursor for continuing output.
    #[serde(default)]
    pub cursor: Option<String>,
}
