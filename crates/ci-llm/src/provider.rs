//! Core LLM strategy trait (`LlmProvider`), request/response payloads, capabilities, and token estimation (D13, D14).

use anyhow::Result;
use ci_core::config::{ModelKind, RoleKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Role identifier alias used across LLM requests, routing, and agent work packages (D13, D16).
pub type RoleName = RoleKind;

/// Work package returned over MCP when a role resolves to `ModelKind::Agent` (D14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentWorkPackage {
    /// Deterministic or unique work package identifier.
    pub package_id: String,
    /// Target execution role (`init`, `update`, `task`, `search`).
    pub role: RoleName,
    /// Grounded repository facts / AST payload for the harness model.
    pub facts: serde_json::Value,
    /// Role-specific prompt instruction for the harness model.
    pub instruction: String,
    /// Expected JSON Schema that the `context_submit` payload must satisfy.
    pub json_schema: serde_json::Value,
}

/// Validated submission produced by `validate_agent_submission` for `context_submit` (D14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValidatedAgentOutput {
    /// Identifier of the validated work package.
    pub package_id: String,
    /// Execution role associated with the package.
    pub role: RoleName,
    /// Validated JSON value conforming to `package.json_schema`.
    pub payload: serde_json::Value,
    /// Canonical JSON string representation of `payload`.
    pub serialized: String,
}

/// Standardized input request passed to any `LlmProvider` implementation (D13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LlmRequest {
    /// Execution role (`init`, `update`, `task`, `search`).
    pub role: RoleName,
    /// Primary user/task prompt text.
    pub prompt: String,
    /// Optional system prompt instructions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Optional JSON Schema that the response must conform to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
    /// Optional per-request maximum input token budget override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<usize>,
    /// Optional per-request execution timeout in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Optional structured facts payload (used especially by `AgentProvider`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<serde_json::Value>,
}

impl LlmRequest {
    /// Creates a new `LlmRequest` for `role` and `prompt` with optional fields set to `None`.
    pub fn new(role: RoleName, prompt: impl Into<String>) -> Self {
        Self {
            role,
            prompt: prompt.into(),
            system_prompt: None,
            json_schema: None,
            max_input_tokens: None,
            timeout_ms: None,
            facts: None,
        }
    }

    /// Sets the optional `system_prompt`.
    pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(system_prompt.into());
        self
    }

    /// Sets the optional `json_schema`.
    pub fn with_json_schema(mut self, json_schema: serde_json::Value) -> Self {
        self.json_schema = Some(json_schema);
        self
    }

    /// Sets the optional `max_input_tokens` limit.
    pub fn with_max_input_tokens(mut self, max_input_tokens: usize) -> Self {
        self.max_input_tokens = Some(max_input_tokens);
        self
    }

    /// Sets the optional `timeout_ms` budget.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Sets the optional structured `facts` payload.
    pub fn with_facts(mut self, facts: serde_json::Value) -> Self {
        self.facts = Some(facts);
        self
    }
}

/// Standardized completion response returned by an `LlmProvider` (D13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LlmResponse {
    /// Generated text or JSON string content.
    pub content: String,
    /// Underlying model identifier (e.g. `"llama3"`, `"claude"`, `"agent-handoff"`).
    pub model: String,
    /// Provider kind that produced this response.
    pub provider_kind: ModelKind,
    /// Configured model alias name (e.g. `"local"`).
    pub alias: String,
    /// Estimated or provider-reported prompt/input token count.
    pub tokens_in: usize,
    /// Estimated or provider-reported completion/output token count.
    pub tokens_out: usize,
    /// Execution latency in milliseconds.
    pub latency_ms: u64,
    /// Number of retry attempts performed prior to succeeding.
    pub retries: u32,
    /// Whether a fallback alias in the role chain was used to produce this response.
    pub fallback_used: bool,
    /// Optional work package when `provider_kind == ModelKind::Agent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_package: Option<AgentWorkPackage>,
}

/// Declarative capability flags exposed by an `LlmProvider` (D13, D15).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderCapabilities {
    /// Whether the provider natively enforces or validates JSON Schema output.
    pub supports_json_schema: bool,
    /// Whether the provider supports token streaming.
    pub supports_streaming: bool,
    /// Whether the provider delegates execution to an external harness/agent (`agent`, `sampling`).
    pub is_agent_handoff: bool,
    /// Optional maximum input context window in tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_context_tokens: Option<usize>,
}

/// Strategy trait implemented by all LLM providers and decorators (D13).
pub trait LlmProvider: Send + Sync {
    /// Executes a synchronous completion request.
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse>;

    /// Returns static capability metadata for this provider.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Returns the provider kind (`ollama`, `command`, `agent`, etc.).
    fn kind(&self) -> ModelKind;

    /// Returns the configured model alias identifier.
    fn alias(&self) -> &str;
}

impl LlmProvider for Box<dyn LlmProvider> {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse> {
        (**self).complete(req)
    }

    fn capabilities(&self) -> ProviderCapabilities {
        (**self).capabilities()
    }

    fn kind(&self) -> ModelKind {
        (**self).kind()
    }

    fn alias(&self) -> &str {
        (**self).alias()
    }
}

impl LlmProvider for Arc<dyn LlmProvider> {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse> {
        (**self).complete(req)
    }

    fn capabilities(&self) -> ProviderCapabilities {
        (**self).capabilities()
    }

    fn kind(&self) -> ModelKind {
        (**self).kind()
    }

    fn alias(&self) -> &str {
        (**self).alias()
    }
}

/// Structured error metadata propagated across decorators so telemetry accurately captures
/// retry counts, schema validation errors, timeouts, and token budgets even on failure.
#[derive(Debug)]
pub struct LlmExecutionError {
    /// Human-readable error summary.
    pub message: String,
    /// Number of retry attempts executed before failing.
    pub retries: u32,
    /// Whether the failure was caused by a JSON / JSON Schema validation error.
    pub schema_error: bool,
    /// Whether the failure was caused by exceeding `timeout_ms`.
    pub timed_out: bool,
    /// Whether the failure was caused by exceeding `max_input_tokens`.
    pub budget_exceeded: bool,
    /// Estimated input tokens at the time of failure.
    pub tokens_in: usize,
}

impl fmt::Display for LlmExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LlmExecutionError {}

/// Extracts `LlmExecutionError` metadata from an `anyhow::Error` chain if present.
pub fn inspect_execution_error(err: &anyhow::Error) -> Option<&LlmExecutionError> {
    for cause in err.chain() {
        if let Some(meta) = cause.downcast_ref::<LlmExecutionError>() {
            return Some(meta);
        }
    }
    None
}

/// Estimates the token count of a text snippet using a fast, deterministic heuristic
/// (~4 characters per token or whitespace word count, whichever is larger).
pub fn estimate_tokens(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let char_estimate = trimmed.chars().count().div_ceil(4);
    let word_estimate = trimmed.split_whitespace().count();
    char_estimate.max(word_estimate).max(1)
}

/// Estimates the total input token count of an `LlmRequest` (prompt + system_prompt + facts + json_schema).
pub fn estimate_request_tokens(req: &LlmRequest) -> usize {
    let mut total = estimate_tokens(&req.prompt);
    if let Some(ref sys) = req.system_prompt {
        total = total.saturating_add(estimate_tokens(sys));
    }
    if let Some(ref facts) = req.facts {
        if !facts.is_null() {
            let facts_str = facts.to_string();
            total = total.saturating_add(estimate_tokens(&facts_str));
        }
    }
    if let Some(ref schema) = req.json_schema {
        if !schema.is_null() {
            let schema_str = schema.to_string();
            total = total.saturating_add(estimate_tokens(&schema_str));
        }
    }
    total
}
