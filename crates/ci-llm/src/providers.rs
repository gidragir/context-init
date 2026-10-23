//! MVP LLM providers (`OllamaProvider`, `CommandProvider`, `AgentProvider`), v0.3 extension points
//! (`OpenAiCompatProvider`, `AnthropicProvider`, `GeminiProvider`, `SamplingProvider`),
//! `validate_agent_submission`, `ProviderFactory`, and `ProviderRegistry` (D13–D15).

use crate::decorators::{
    RetryDecorator, TelemetryDecorator, TimeoutDecorator, TokenBudgetDecorator,
};
use crate::provider::{
    estimate_request_tokens, estimate_tokens, AgentWorkPackage, LlmExecutionError, LlmProvider,
    LlmRequest, LlmResponse, ProviderCapabilities, ValidatedAgentOutput,
};
use crate::schema::{
    default_role_json_schema, extract_json_payload, parse_and_validate_json, validate_json_schema,
};
use anyhow::{bail, Context, Result};
use ci_core::config::{Config, ModelConfig, ModelKind};
use ci_core::storage::normalize_line_endings;
use ci_core::telemetry::TelemetrySink;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

static PACKAGE_SEQ: AtomicU64 = AtomicU64::new(1);

/// Default HTTP request timeout in milliseconds when none is configured.
pub const DEFAULT_PROVIDER_TIMEOUT_MS: u64 = 60_000;

/// Default health-check timeout in milliseconds for `is_ollama_online`.
pub const DEFAULT_HEALTHCHECK_TIMEOUT_MS: u64 = 1_500;

/// Structured subsystem summary item parsed from LLM JSON responses (generalized from legacy `src/ollama.rs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SubsystemAiSummary {
    /// Normalized subsystem identifier (e.g. `"core"`, `"detector"`).
    pub id: String,
    /// Human-readable subsystem title.
    pub name: String,
    /// Concise architectural summary (1–2 sentences).
    pub summary: String,
    /// Technical classification tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Parses `Vec<SubsystemAiSummary>` from flexible LLM JSON output shapes (generalized from legacy `src/ollama.rs`):
/// - Markdown code fences (` ```json ... ``` ` or ` ``` ... ``` `)
/// - Wrapper object `{"subsystems": [...]}` (or aliases `"items"`, `"modules"`)
/// - Direct JSON array `[{"id": ...}, ...]`
/// - Single JSON object `{"id": ...}`
pub fn parse_subsystem_summaries(raw: &str) -> Result<Vec<SubsystemAiSummary>> {
    let json_clean = extract_json_payload(raw);
    if json_clean.is_empty() {
        bail!("Empty LLM response when parsing subsystem summaries");
    }

    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(alias = "items", alias = "modules")]
        subsystems: Vec<SubsystemAiSummary>,
    }

    if let Ok(w) = serde_json::from_str::<Wrapper>(json_clean) {
        Ok(w.subsystems)
    } else if let Ok(arr) = serde_json::from_str::<Vec<SubsystemAiSummary>>(json_clean) {
        Ok(arr)
    } else if let Ok(single) = serde_json::from_str::<SubsystemAiSummary>(json_clean) {
        Ok(vec![single])
    } else {
        serde_json::from_str::<Vec<SubsystemAiSummary>>(json_clean).with_context(|| {
            format!("Failed to parse subsystem summaries JSON from response: {raw}")
        })
    }
}

/// Checks whether an Ollama endpoint is reachable by querying `<endpoint>/api/tags` via `ureq` (D14).
pub fn is_ollama_online(endpoint: &str, timeout_ms: Option<u64>) -> bool {
    let trimmed = endpoint.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return false;
    }
    let url = format!("{trimmed}/api/tags");
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_HEALTHCHECK_TIMEOUT_MS));
    ureq::get(&url)
        .timeout(timeout)
        .call()
        .map(|resp| resp.status() == 200)
        .unwrap_or(false)
}

#[derive(Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a str>,
}

#[derive(Deserialize, Debug)]
struct OllamaGenerateResponse {
    #[serde(default)]
    response: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
    #[serde(default)]
    eval_count: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaTagEntry>,
}

#[derive(Deserialize, Debug)]
struct OllamaTagEntry {
    name: String,
}

/// Ollama HTTP provider (`kind = "ollama"`, D14).
/// Contains no hardcoded URLs, model names, or natural languages.
#[derive(Debug, Clone)]
pub struct OllamaProvider {
    alias: String,
    endpoint: Option<String>,
    model: Option<String>,
    timeout_ms: Option<u64>,
    max_input_tokens: Option<usize>,
}

impl OllamaProvider {
    /// Creates an `OllamaProvider` with explicit `alias`, `endpoint`, and `model`.
    pub fn new(
        alias: impl Into<String>,
        endpoint: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            alias: alias.into(),
            endpoint: Some(endpoint.into()),
            model: Some(model.into()),
            timeout_ms: None,
            max_input_tokens: None,
        }
    }

    /// Creates an `OllamaProvider` from a `ModelConfig`.
    pub fn from_config(alias: impl Into<String>, cfg: &ModelConfig) -> Self {
        Self {
            alias: alias.into(),
            endpoint: cfg.endpoint.clone(),
            model: cfg.model.clone(),
            timeout_ms: cfg.timeout_ms,
            max_input_tokens: cfg.max_input_tokens,
        }
    }

    /// Sets the default timeout in milliseconds.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Sets the maximum input token budget.
    pub fn with_max_input_tokens(mut self, max_input_tokens: usize) -> Self {
        self.max_input_tokens = Some(max_input_tokens);
        self
    }

    /// Returns the configured endpoint URL, if any.
    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    /// Returns the configured model identifier, if any.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Checks whether the configured `<endpoint>/api/tags` responds with HTTP 200.
    pub fn is_online(&self) -> bool {
        let Some(ref ep) = self.endpoint else {
            return false;
        };
        is_ollama_online(ep, self.timeout_ms)
    }

    /// Queries `<endpoint>/api/tags` and returns the list of available model names.
    pub fn fetch_tags(&self) -> Result<Vec<String>> {
        let ep = self
            .endpoint
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .with_context(|| {
                format!(
                    "OllamaProvider '{}' has no configured 'endpoint'",
                    self.alias
                )
            })?
            .trim_end_matches('/');

        let url = format!("{ep}/api/tags");
        let timeout =
            Duration::from_millis(self.timeout_ms.unwrap_or(DEFAULT_HEALTHCHECK_TIMEOUT_MS));
        let resp = ureq::get(&url)
            .timeout(timeout)
            .call()
            .with_context(|| format!("Failed to reach Ollama tags endpoint at {url}"))?;
        let parsed: OllamaTagsResponse = resp
            .into_json()
            .with_context(|| format!("Failed to decode Ollama tags response from {url}"))?;
        Ok(parsed.models.into_iter().map(|m| m.name).collect())
    }
}

impl LlmProvider for OllamaProvider {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse> {
        let ep = self
            .endpoint
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .with_context(|| {
                format!(
                    "OllamaProvider '{}' is missing required 'endpoint' configuration",
                    self.alias
                )
            })?
            .trim_end_matches('/');

        let model_name = self
            .model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .with_context(|| {
                format!(
                    "OllamaProvider '{}' is missing required 'model' configuration",
                    self.alias
                )
            })?;

        let timeout_ms = req
            .timeout_ms
            .or(self.timeout_ms)
            .unwrap_or(DEFAULT_PROVIDER_TIMEOUT_MS);
        let est_tokens_in = estimate_request_tokens(req);

        if timeout_ms == 0 {
            return Err(anyhow::Error::new(LlmExecutionError {
                message: format!(
                    "OllamaProvider '{}' timed out: timeout_ms budget is 0 ms",
                    self.alias
                ),
                retries: 0,
                schema_error: false,
                timed_out: true,
                budget_exceeded: false,
                tokens_in: est_tokens_in,
            }));
        }

        let payload = OllamaGenerateRequest {
            model: model_name,
            prompt: &req.prompt,
            system: req.system_prompt.as_deref(),
            stream: false,
            format: if req.json_schema.is_some() {
                Some("json")
            } else {
                None
            },
        };

        let url = format!("{ep}/api/generate");
        let start = Instant::now();
        let res = match ureq::post(&url)
            .timeout(Duration::from_millis(timeout_ms))
            .send_json(&payload)
        {
            Ok(resp) => resp,
            Err(err) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                let err_str = err.to_string();
                let err_lower = err_str.to_ascii_lowercase();
                if elapsed_ms >= timeout_ms
                    || err_lower.contains("timed out")
                    || err_lower.contains("timeout")
                {
                    return Err(anyhow::Error::new(LlmExecutionError {
                        message: format!(
                            "OllamaProvider '{}' timed out after {} ms at {url}: {err_str}",
                            self.alias, timeout_ms
                        ),
                        retries: 0,
                        schema_error: false,
                        timed_out: true,
                        budget_exceeded: false,
                        tokens_in: est_tokens_in,
                    }));
                }
                return Err(err).with_context(|| {
                    format!("Ollama request failed for alias '{}' at {url}", self.alias)
                });
            }
        };

        let body: OllamaGenerateResponse = res.into_json().map_err(|err| {
            anyhow::Error::new(LlmExecutionError {
                message: format!(
                    "Failed to decode Ollama JSON envelope from '{}': {}",
                    self.alias, err
                ),
                retries: 0,
                schema_error: true,
                timed_out: false,
                budget_exceeded: false,
                tokens_in: est_tokens_in,
            })
        })?;

        if let Some(err_msg) = body.error.filter(|s| !s.trim().is_empty()) {
            bail!("Ollama error from '{}': {}", self.alias, err_msg);
        }

        let Some(raw_response) = body.response else {
            return Err(anyhow::Error::new(LlmExecutionError {
                message: format!(
                    "Ollama JSON envelope from '{}' is missing 'response' field",
                    self.alias
                ),
                retries: 0,
                schema_error: true,
                timed_out: false,
                budget_exceeded: false,
                tokens_in: est_tokens_in,
            }));
        };

        let latency_ms = start.elapsed().as_millis() as u64;
        let raw_trimmed = normalize_line_endings(&raw_response).trim().to_string();

        let final_content = if let Some(ref schema) = req.json_schema {
            let validated_val = parse_and_validate_json(&raw_trimmed, Some(schema), est_tokens_in)?;
            serde_json::to_string(&validated_val)
                .unwrap_or_else(|_| extract_json_payload(&raw_trimmed).to_string())
        } else {
            raw_trimmed
        };

        let tokens_in = body
            .prompt_eval_count
            .filter(|&c| c > 0)
            .unwrap_or(est_tokens_in);
        let tokens_out = body
            .eval_count
            .filter(|&c| c > 0)
            .unwrap_or_else(|| estimate_tokens(&final_content));

        Ok(LlmResponse {
            content: final_content,
            model: body.model.unwrap_or_else(|| model_name.to_string()),
            provider_kind: ModelKind::Ollama,
            alias: self.alias.clone(),
            tokens_in,
            tokens_out,
            latency_ms,
            retries: 0,
            fallback_used: false,
            work_package: None,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_json_schema: true,
            supports_streaming: true,
            is_agent_handoff: false,
            max_context_tokens: self.max_input_tokens,
        }
    }

    fn kind(&self) -> ModelKind {
        ModelKind::Ollama
    }

    fn alias(&self) -> &str {
        &self.alias
    }
}

/// CLI command provider (`kind = "command"`, D14).
/// Executes an external command (e.g. `claude -p`, `gemini -p`, `agy -p`, `codex exec`),
/// passing the prompt via `stdin` or substituting `{prompt}` in arguments.
#[derive(Debug, Clone)]
pub struct CommandProvider {
    alias: String,
    command: Vec<String>,
    model: Option<String>,
    timeout_ms: Option<u64>,
    max_input_tokens: Option<usize>,
}

impl CommandProvider {
    /// Creates a `CommandProvider` from an explicit command list.
    pub fn new(alias: impl Into<String>, command: Vec<String>) -> Result<Self> {
        let alias_str = alias.into();
        if command.is_empty() || command[0].trim().is_empty() {
            bail!(
                "CommandProvider '{}' requires a non-empty command executable",
                alias_str
            );
        }
        Ok(Self {
            alias: alias_str,
            command,
            model: None,
            timeout_ms: None,
            max_input_tokens: None,
        })
    }

    /// Creates a `CommandProvider` from a `ModelConfig`.
    pub fn from_config(alias: impl Into<String>, cfg: &ModelConfig) -> Result<Self> {
        let alias_str = alias.into();
        let cmd = cfg.command.clone().unwrap_or_default();
        if cmd.is_empty() || cmd[0].trim().is_empty() {
            bail!(
                "CommandProvider '{}' requires a non-empty 'command' list in ModelConfig",
                alias_str
            );
        }
        Ok(Self {
            alias: alias_str,
            command: cmd,
            model: cfg.model.clone(),
            timeout_ms: cfg.timeout_ms,
            max_input_tokens: cfg.max_input_tokens,
        })
    }

    /// Sets the optional model name metadata.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the default execution timeout in milliseconds.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Sets the maximum input token budget.
    pub fn with_max_input_tokens(mut self, max_input_tokens: usize) -> Self {
        self.max_input_tokens = Some(max_input_tokens);
        self
    }

    /// Returns the configured command arguments.
    pub fn command(&self) -> &[String] {
        &self.command
    }
}

impl LlmProvider for CommandProvider {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse> {
        if self.command.is_empty() || self.command[0].trim().is_empty() {
            bail!("CommandProvider '{}' has an empty command", self.alias);
        }

        let program = &self.command[0];
        let raw_args = &self.command[1..];

        let has_system_placeholder = raw_args.iter().any(|a| a.contains("{system_prompt}"));
        let full_prompt = match (&req.system_prompt, has_system_placeholder) {
            (Some(sys), false) if !sys.trim().is_empty() => {
                format!("{}\n\n{}", sys.trim(), req.prompt)
            }
            _ => req.prompt.clone(),
        };

        let mut used_prompt_placeholder = false;
        let sys_str = req.system_prompt.as_deref().unwrap_or("");
        let model_str = self.model.as_deref().unwrap_or("");
        let mut resolved_args = Vec::with_capacity(raw_args.len());
        for arg in raw_args {
            let (rendered, used_prompt) =
                substitute_command_arg(arg, &full_prompt, sys_str, model_str);
            if used_prompt {
                used_prompt_placeholder = true;
            }
            resolved_args.push(rendered);
        }

        let start = Instant::now();
        let est_tokens_in = estimate_request_tokens(req);
        let timeout_ms = req.timeout_ms.or(self.timeout_ms);

        if timeout_ms == Some(0) {
            return Err(anyhow::Error::new(LlmExecutionError {
                message: format!(
                    "CommandProvider '{}' ('{}') timed out: timeout_ms budget is 0 ms",
                    self.alias, program
                ),
                retries: 0,
                schema_error: false,
                timed_out: true,
                budget_exceeded: false,
                tokens_in: est_tokens_in,
            }));
        }

        let mut cmd = Command::new(program);
        cmd.args(&resolved_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if used_prompt_placeholder {
            cmd.stdin(Stdio::null());
        } else {
            cmd.stdin(Stdio::piped());
        }

        let mut child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn CommandProvider '{}' executable '{}'",
                self.alias, program
            )
        })?;

        // Read stdout and stderr in background threads first so large outputs never deadlock pipe buffers
        let stdout_handle = child.stdout.take().map(|mut out| {
            thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = out.read_to_end(&mut buf);
                buf
            })
        });
        let stderr_handle = child.stderr.take().map(|mut err| {
            thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = err.read_to_end(&mut buf);
                buf
            })
        });

        // Write stdin in a background thread so large (>64 KB) prompts never block the timeout loop
        // if the child process hangs without reading from stdin.
        let stdin_handle = if !used_prompt_placeholder {
            child.stdin.take().map(|mut stdin_pipe| {
                let prompt_bytes = full_prompt.into_bytes();
                thread::spawn(move || {
                    let _ = stdin_pipe.write_all(&prompt_bytes);
                    let _ = stdin_pipe.flush();
                })
            })
        } else {
            None
        };

        let status = if let Some(budget_ms) = timeout_ms {
            let deadline = start + Duration::from_millis(budget_ms);
            loop {
                match child.try_wait().with_context(|| {
                    format!("Failed waiting on CommandProvider '{}'", self.alias)
                })? {
                    Some(exit_status) => break exit_status,
                    None => {
                        if Instant::now() >= deadline {
                            let _ = child.kill();
                            let _ = child.wait();
                            if let Some(h) = stdin_handle {
                                let _ = h.join();
                            }
                            if let Some(h) = stdout_handle {
                                let _ = h.join();
                            }
                            if let Some(h) = stderr_handle {
                                let _ = h.join();
                            }
                            return Err(anyhow::Error::new(LlmExecutionError {
                                message: format!(
                                    "CommandProvider '{}' ('{}') timed out after {} ms",
                                    self.alias, program, budget_ms
                                ),
                                retries: 0,
                                schema_error: false,
                                timed_out: true,
                                budget_exceeded: false,
                                tokens_in: est_tokens_in,
                            }));
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                }
            }
        } else {
            child
                .wait()
                .with_context(|| format!("Failed waiting on CommandProvider '{}'", self.alias))?
        };

        if let Some(h) = stdin_handle {
            let _ = h.join();
        }
        let stdout_bytes = stdout_handle
            .and_then(|h| h.join().ok())
            .unwrap_or_default();
        let stderr_bytes = stderr_handle
            .and_then(|h| h.join().ok())
            .unwrap_or_default();

        let latency_ms = start.elapsed().as_millis() as u64;
        let stdout_str = normalize_line_endings(&String::from_utf8_lossy(&stdout_bytes))
            .trim()
            .to_string();
        let stderr_str = normalize_line_endings(&String::from_utf8_lossy(&stderr_bytes))
            .trim()
            .to_string();

        if !status.success() {
            let code_desc = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            bail!(
                "CommandProvider '{}' ('{}') exited with non-zero status {}: {}",
                self.alias,
                program,
                code_desc,
                if stderr_str.is_empty() {
                    &stdout_str
                } else {
                    &stderr_str
                }
            );
        }

        let final_content = if let Some(ref schema) = req.json_schema {
            let validated_val = parse_and_validate_json(&stdout_str, Some(schema), est_tokens_in)?;
            serde_json::to_string(&validated_val)
                .unwrap_or_else(|_| extract_json_payload(&stdout_str).to_string())
        } else {
            stdout_str
        };

        let tokens_out = estimate_tokens(&final_content);

        Ok(LlmResponse {
            content: final_content,
            model: self.model.clone().unwrap_or_else(|| program.clone()),
            provider_kind: ModelKind::Command,
            alias: self.alias.clone(),
            tokens_in: est_tokens_in,
            tokens_out,
            latency_ms,
            retries: 0,
            fallback_used: false,
            work_package: None,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_json_schema: true,
            supports_streaming: false,
            is_agent_handoff: false,
            max_context_tokens: self.max_input_tokens,
        }
    }

    fn kind(&self) -> ModelKind {
        ModelKind::Command
    }

    fn alias(&self) -> &str {
        &self.alias
    }
}

fn substitute_command_arg(
    arg: &str,
    full_prompt: &str,
    system_prompt: &str,
    model: &str,
) -> (String, bool) {
    let mut out = String::with_capacity(arg.len());
    let mut used_prompt = false;
    let mut rest = arg;
    while let Some(open_idx) = rest.find('{') {
        out.push_str(&rest[..open_idx]);
        let after_open = &rest[open_idx + 1..];
        if let Some(close_idx) = after_open.find('}') {
            let key = &after_open[..close_idx];
            match key {
                "prompt" => {
                    out.push_str(full_prompt);
                    used_prompt = true;
                    rest = &after_open[close_idx + 1..];
                    continue;
                }
                "system_prompt" => {
                    out.push_str(system_prompt);
                    rest = &after_open[close_idx + 1..];
                    continue;
                }
                "model" => {
                    out.push_str(model);
                    rest = &after_open[close_idx + 1..];
                    continue;
                }
                _ => {}
            }
        }
        out.push('{');
        rest = after_open;
    }
    out.push_str(rest);
    (out, used_prompt)
}

/// Agent handoff provider (`kind = "agent"`, D14).
/// Builds an `AgentWorkPackage` (`package_id`, `role`, `facts`, `instruction`, `json_schema`)
/// to be returned over MCP and validated upon `context_submit` via `validate_agent_submission`.
#[derive(Debug, Clone)]
pub struct AgentProvider {
    alias: String,
    model: Option<String>,
    max_input_tokens: Option<usize>,
}

impl AgentProvider {
    /// Creates a new `AgentProvider` for `alias`.
    pub fn new(alias: impl Into<String>) -> Self {
        Self {
            alias: alias.into(),
            model: None,
            max_input_tokens: None,
        }
    }

    /// Creates an `AgentProvider` from a `ModelConfig`.
    pub fn from_config(alias: impl Into<String>, cfg: &ModelConfig) -> Self {
        Self {
            alias: alias.into(),
            model: cfg.model.clone(),
            max_input_tokens: cfg.max_input_tokens,
        }
    }

    /// Builds an `AgentWorkPackage` directly from an `LlmRequest`.
    pub fn build_work_package(&self, req: &LlmRequest) -> AgentWorkPackage {
        let seq = PACKAGE_SEQ.fetch_add(1, Ordering::Relaxed);
        let package_id = format!("pkg-{}-{}-{}", self.alias, req.role.as_str(), seq);

        let instruction = match &req.system_prompt {
            Some(sys) if !sys.trim().is_empty() => {
                format!("{}\n\n{}", sys.trim(), req.prompt.trim())
            }
            _ => req.prompt.clone(),
        };

        let facts = req.facts.clone().unwrap_or_else(|| json!({}));
        let json_schema = req
            .json_schema
            .clone()
            .unwrap_or_else(|| default_role_json_schema(req.role));

        AgentWorkPackage {
            package_id,
            role: req.role,
            facts,
            instruction,
            json_schema,
        }
    }
}

impl LlmProvider for AgentProvider {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse> {
        let start = Instant::now();
        let pkg = self.build_work_package(req);
        let content = serde_json::to_string_pretty(&pkg)
            .context("Failed to serialize AgentWorkPackage to JSON")?;
        let latency_ms = start.elapsed().as_millis() as u64;
        let tokens_in = estimate_request_tokens(req);
        let tokens_out = estimate_tokens(&content);

        Ok(LlmResponse {
            content,
            model: self
                .model
                .clone()
                .unwrap_or_else(|| "agent-handoff".to_string()),
            provider_kind: ModelKind::Agent,
            alias: self.alias.clone(),
            tokens_in,
            tokens_out,
            latency_ms,
            retries: 0,
            fallback_used: false,
            work_package: Some(pkg),
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_json_schema: true,
            supports_streaming: false,
            is_agent_handoff: true,
            max_context_tokens: self.max_input_tokens,
        }
    }

    fn kind(&self) -> ModelKind {
        ModelKind::Agent
    }

    fn alias(&self) -> &str {
        &self.alias
    }
}

/// Validates an agent submission (`context_submit`) against the `AgentWorkPackage` JSON Schema (D14).
///
/// Supports both:
/// 1. Direct payload matching `package.json_schema`.
/// 2. Envelope object `{"package_id": "...", "result": <payload>}` (or `"output"` / `"submission"`),
///    verifying that `package_id` matches `package.package_id`.
pub fn validate_agent_submission(
    package: &AgentWorkPackage,
    submission: &Value,
) -> Result<ValidatedAgentOutput> {
    if submission.is_null() {
        bail!(
            "Agent submission for package '{}' (role '{}') must not be null",
            package.package_id,
            package.role
        );
    }

    // If caller passed a JSON-encoded string while the schema does not expect a raw string, parse it first
    let parsed_from_str: Option<Value> = if let Some(raw_str) = submission.as_str() {
        let expects_string = package
            .json_schema
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == "string");
        if !expects_string {
            let cleaned = extract_json_payload(raw_str);
            Some(serde_json::from_str(cleaned).with_context(|| {
                format!(
                    "Agent submission for package '{}' is a string but not valid JSON",
                    package.package_id
                )
            })?)
        } else {
            None
        }
    } else {
        None
    };

    let root_val = parsed_from_str.as_ref().unwrap_or(submission);

    let schema_defines_package_id = package
        .json_schema
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|props| props.contains_key("package_id"))
        || package
            .json_schema
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|req| req.iter().any(|v| v.as_str() == Some("package_id")));

    let mut payload_cow = root_val.clone();

    if let Some(obj) = root_val.as_object() {
        if !schema_defines_package_id {
            if let Some(pkg_id_val) = obj.get("package_id") {
                let Some(sub_pkg_id) = pkg_id_val.as_str() else {
                    bail!(
                        "Agent submission 'package_id' must be a string, got {}",
                        pkg_id_val
                    );
                };
                if sub_pkg_id != package.package_id {
                    bail!(
                        "Agent submission package_id mismatch: expected '{}', got '{}'",
                        package.package_id,
                        sub_pkg_id
                    );
                }
            }
        }

        // Unwrap envelope if `result` / `output` / `submission` / `payload` field is present and outer object doesn't define that key in schema
        let mut unwrapped = false;
        for envelope_key in ["result", "output", "submission", "payload"] {
            if let Some(inner_val) = obj.get(envelope_key) {
                let schema_has_key = package
                    .json_schema
                    .get("properties")
                    .and_then(Value::as_object)
                    .is_some_and(|props| props.contains_key(envelope_key))
                    || package
                        .json_schema
                        .get("required")
                        .and_then(Value::as_array)
                        .is_some_and(|req| req.iter().any(|v| v.as_str() == Some(envelope_key)));
                if !schema_has_key {
                    payload_cow = inner_val.clone();
                    unwrapped = true;
                    break;
                }
            }
        }

        if !unwrapped && !schema_defines_package_id && obj.contains_key("package_id") {
            let mut stripped = obj.clone();
            stripped.remove("package_id");
            payload_cow = Value::Object(stripped);
        }
    }

    if let Some(inner_str) = payload_cow.as_str() {
        let expects_string = package
            .json_schema
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == "string");
        if !expects_string {
            let cleaned = extract_json_payload(inner_str);
            payload_cow = serde_json::from_str(cleaned).with_context(|| {
                format!(
                    "Agent submission envelope for package '{}' contains a string that is not valid JSON",
                    package.package_id
                )
            })?;
        }
    }

    validate_json_schema(&package.json_schema, &payload_cow).with_context(|| {
        format!(
            "Agent submission for package '{}' (role '{}') failed JSON Schema validation",
            package.package_id, package.role
        )
    })?;

    let serialized = serde_json::to_string(&payload_cow)
        .context("Failed to serialize validated agent submission")?;

    Ok(ValidatedAgentOutput {
        package_id: package.package_id.clone(),
        role: package.role,
        payload: payload_cow,
        serialized,
    })
}

/// Structured error returned when invoking an unimplemented v0.3 extension point provider (D15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionPointError {
    /// Configured model alias.
    pub alias: String,
    /// Extension point provider kind (`openai_compat`, `anthropic`, `gemini`, `sampling`).
    pub kind: ModelKind,
    /// Detail message describing the extension point.
    pub message: String,
}

impl fmt::Display for ExtensionPointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Provider '{}' (kind = '{}') is an unimplemented v0.3 extension point: {}",
            self.alias, self.kind, self.message
        )
    }
}

impl std::error::Error for ExtensionPointError {}

/// OpenAI-compatible HTTP endpoint extension point (`kind = "openai_compat"`, D15).
#[derive(Debug, Clone)]
pub struct OpenAiCompatProvider {
    alias: String,
    endpoint: Option<String>,
    model: Option<String>,
    max_input_tokens: Option<usize>,
}

impl OpenAiCompatProvider {
    /// Creates an `OpenAiCompatProvider` from a `ModelConfig`.
    pub fn from_config(alias: impl Into<String>, cfg: &ModelConfig) -> Self {
        Self {
            alias: alias.into(),
            endpoint: cfg.endpoint.clone(),
            model: cfg.model.clone(),
            max_input_tokens: cfg.max_input_tokens,
        }
    }

    /// Returns the configured endpoint URL, if any.
    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    /// Returns the configured model identifier, if any.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

impl LlmProvider for OpenAiCompatProvider {
    fn complete(&self, _req: &LlmRequest) -> Result<LlmResponse> {
        Err(anyhow::Error::new(ExtensionPointError {
            alias: self.alias.clone(),
            kind: ModelKind::OpenaiCompat,
            message: "v0.3 extension point for OpenAI-compatible HTTP endpoints (D15)".to_string(),
        }))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_json_schema: true,
            supports_streaming: true,
            is_agent_handoff: false,
            max_context_tokens: self.max_input_tokens,
        }
    }

    fn kind(&self) -> ModelKind {
        ModelKind::OpenaiCompat
    }

    fn alias(&self) -> &str {
        &self.alias
    }
}

/// Anthropic HTTP API extension point (`kind = "anthropic"`, D15).
#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    alias: String,
    endpoint: Option<String>,
    model: Option<String>,
    max_input_tokens: Option<usize>,
}

impl AnthropicProvider {
    /// Creates an `AnthropicProvider` from a `ModelConfig`.
    pub fn from_config(alias: impl Into<String>, cfg: &ModelConfig) -> Self {
        Self {
            alias: alias.into(),
            endpoint: cfg.endpoint.clone(),
            model: cfg.model.clone(),
            max_input_tokens: cfg.max_input_tokens,
        }
    }

    /// Returns the configured endpoint URL, if any.
    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    /// Returns the configured model identifier, if any.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

impl LlmProvider for AnthropicProvider {
    fn complete(&self, _req: &LlmRequest) -> Result<LlmResponse> {
        Err(anyhow::Error::new(ExtensionPointError {
            alias: self.alias.clone(),
            kind: ModelKind::Anthropic,
            message: "v0.3 extension point for Anthropic Messages API (D15)".to_string(),
        }))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_json_schema: true,
            supports_streaming: true,
            is_agent_handoff: false,
            max_context_tokens: self.max_input_tokens,
        }
    }

    fn kind(&self) -> ModelKind {
        ModelKind::Anthropic
    }

    fn alias(&self) -> &str {
        &self.alias
    }
}

/// Google Gemini HTTP API extension point (`kind = "gemini"`, D15).
#[derive(Debug, Clone)]
pub struct GeminiProvider {
    alias: String,
    endpoint: Option<String>,
    model: Option<String>,
    max_input_tokens: Option<usize>,
}

impl GeminiProvider {
    /// Creates a `GeminiProvider` from a `ModelConfig`.
    pub fn from_config(alias: impl Into<String>, cfg: &ModelConfig) -> Self {
        Self {
            alias: alias.into(),
            endpoint: cfg.endpoint.clone(),
            model: cfg.model.clone(),
            max_input_tokens: cfg.max_input_tokens,
        }
    }

    /// Returns the configured endpoint URL, if any.
    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    /// Returns the configured model identifier, if any.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

impl LlmProvider for GeminiProvider {
    fn complete(&self, _req: &LlmRequest) -> Result<LlmResponse> {
        Err(anyhow::Error::new(ExtensionPointError {
            alias: self.alias.clone(),
            kind: ModelKind::Gemini,
            message: "v0.3 extension point for Gemini generateContent API (D15)".to_string(),
        }))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_json_schema: true,
            supports_streaming: true,
            is_agent_handoff: false,
            max_context_tokens: self.max_input_tokens,
        }
    }

    fn kind(&self) -> ModelKind {
        ModelKind::Gemini
    }

    fn alias(&self) -> &str {
        &self.alias
    }
}

/// MCP `sampling/createMessage` extension point (`kind = "sampling"`, D15).
#[derive(Debug, Clone)]
pub struct SamplingProvider {
    alias: String,
    model: Option<String>,
    max_input_tokens: Option<usize>,
}

impl SamplingProvider {
    /// Creates a `SamplingProvider` from a `ModelConfig`.
    pub fn from_config(alias: impl Into<String>, cfg: &ModelConfig) -> Self {
        Self {
            alias: alias.into(),
            model: cfg.model.clone(),
            max_input_tokens: cfg.max_input_tokens,
        }
    }

    /// Returns the configured model hint, if any.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

impl LlmProvider for SamplingProvider {
    fn complete(&self, _req: &LlmRequest) -> Result<LlmResponse> {
        Err(anyhow::Error::new(ExtensionPointError {
            alias: self.alias.clone(),
            kind: ModelKind::Sampling,
            message: "v0.3 extension point for MCP sampling/createMessage (D15)".to_string(),
        }))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_json_schema: true,
            supports_streaming: false,
            is_agent_handoff: true,
            max_context_tokens: self.max_input_tokens,
        }
    }

    fn kind(&self) -> ModelKind {
        ModelKind::Sampling
    }

    fn alias(&self) -> &str {
        &self.alias
    }
}

/// Factory for instantiating `LlmProvider` instances from `ci_core::config::ModelConfig` (D13).
pub struct ProviderFactory;

impl ProviderFactory {
    /// Creates an undecorated `LlmProvider` instance for the given `alias` and `ModelConfig`.
    pub fn create_raw(alias: &str, cfg: &ModelConfig) -> Result<Arc<dyn LlmProvider>> {
        let provider: Arc<dyn LlmProvider> = match cfg.kind {
            ModelKind::Ollama => Arc::new(OllamaProvider::from_config(alias, cfg)),
            ModelKind::Command => Arc::new(CommandProvider::from_config(alias, cfg)?),
            ModelKind::Agent => Arc::new(AgentProvider::from_config(alias, cfg)),
            ModelKind::OpenaiCompat => Arc::new(OpenAiCompatProvider::from_config(alias, cfg)),
            ModelKind::Anthropic => Arc::new(AnthropicProvider::from_config(alias, cfg)),
            ModelKind::Gemini => Arc::new(GeminiProvider::from_config(alias, cfg)),
            ModelKind::Sampling => Arc::new(SamplingProvider::from_config(alias, cfg)),
        };
        Ok(provider)
    }

    /// Creates a provider wrapped with `TimeoutDecorator` and `TokenBudgetDecorator` from `ModelConfig`.
    pub fn create(alias: &str, cfg: &ModelConfig) -> Result<Arc<dyn LlmProvider>> {
        Self::create_with_decorators(alias, cfg, 0, None, None)
    }

    /// Creates a fully decorated provider chain (`TokenBudgetDecorator` -> `RetryDecorator` -> `TimeoutDecorator` -> `TelemetryDecorator`).
    pub fn create_with_decorators(
        alias: &str,
        cfg: &ModelConfig,
        max_retries: u32,
        telemetry: Option<Arc<dyn TelemetrySink>>,
        project_id: Option<&str>,
    ) -> Result<Arc<dyn LlmProvider>> {
        let mut provider = Self::create_raw(alias, cfg)?;

        provider = Arc::new(TimeoutDecorator::new(provider, cfg.timeout_ms));

        if max_retries > 0 {
            provider = Arc::new(RetryDecorator::new(provider, max_retries));
        }

        provider = Arc::new(TokenBudgetDecorator::new(provider, cfg.max_input_tokens));

        if let Some(sink) = telemetry {
            let mut tel_dec = TelemetryDecorator::new(provider, sink);
            if let Some(pid) = project_id {
                tel_dec = tel_dec.with_project_id(pid);
            }
            provider = Arc::new(tel_dec);
        }

        Ok(provider)
    }
}

/// Registry of named `LlmProvider` instances indexed by model alias (D13, D16).
#[derive(Default, Clone)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, Arc<dyn LlmProvider>>,
}

impl ProviderRegistry {
    /// Creates an empty `ProviderRegistry`.
    pub fn new() -> Self {
        Self {
            providers: BTreeMap::new(),
        }
    }

    /// Builds a `ProviderRegistry` from all `[models.<alias>]` entries in `Config`.
    pub fn from_config(config: &Config) -> Result<Self> {
        let mut registry = Self::new();
        for (alias, model_cfg) in &config.models {
            let provider = ProviderFactory::create(alias, model_cfg)?;
            registry.register_shared(alias.clone(), provider);
        }
        Ok(registry)
    }

    /// Registers or replaces a provider under `alias`.
    pub fn register<P: LlmProvider + 'static>(&mut self, alias: impl Into<String>, provider: P) {
        self.providers.insert(alias.into(), Arc::new(provider));
    }

    /// Registers an already-shared `Arc<dyn LlmProvider>` under `alias`.
    pub fn register_shared(&mut self, alias: impl Into<String>, provider: Arc<dyn LlmProvider>) {
        self.providers.insert(alias.into(), provider);
    }

    /// Looks up a provider by `alias`.
    pub fn get(&self, alias: &str) -> Option<Arc<dyn LlmProvider>> {
        self.providers.get(alias.trim()).cloned()
    }

    /// Returns `true` if `alias` is registered.
    pub fn contains(&self, alias: &str) -> bool {
        self.providers.contains_key(alias.trim())
    }

    /// Returns the number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Returns `true` if no providers are registered.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Returns an iterator over registered alias names.
    pub fn aliases(&self) -> impl Iterator<Item = &str> {
        self.providers.keys().map(String::as_str)
    }
}
