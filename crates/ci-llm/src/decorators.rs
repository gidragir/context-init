//! Provider decorators: `RetryDecorator`, `TimeoutDecorator`, `TokenBudgetDecorator`, and `TelemetryDecorator` (D13, D29).

use crate::provider::{
    estimate_request_tokens, estimate_tokens, inspect_execution_error, LlmExecutionError,
    LlmProvider, LlmRequest, LlmResponse, ProviderCapabilities, RoleName,
};
use anyhow::Result;
use ci_core::config::ModelKind;
use ci_core::telemetry::{LlmSignal, TelemetryEvent, TelemetrySignal, TelemetrySink};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Decorator that retries failed provider invocations and tracks total retry count (D13, D29).
pub struct RetryDecorator {
    inner: Arc<dyn LlmProvider>,
    max_retries: u32,
    backoff_ms: u64,
}

impl RetryDecorator {
    /// Creates a `RetryDecorator` that retries up to `max_retries` times on failure (total attempts = `1 + max_retries`).
    pub fn new<P: LlmProvider + 'static>(inner: P, max_retries: u32) -> Self {
        Self {
            inner: Arc::new(inner),
            max_retries,
            backoff_ms: 0,
        }
    }

    /// Sets an optional delay in milliseconds between retry attempts.
    pub fn with_backoff_ms(mut self, backoff_ms: u64) -> Self {
        self.backoff_ms = backoff_ms;
        self
    }

    /// Returns the configured maximum number of retries.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

impl LlmProvider for RetryDecorator {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse> {
        let mut attempt = 0u32;
        loop {
            match self.inner.complete(req) {
                Ok(mut resp) => {
                    resp.retries = resp.retries.saturating_add(attempt);
                    return Ok(resp);
                }
                Err(err) => {
                    // Do not retry deterministic pre-flight token budget errors or unimplemented extension points
                    let is_non_retryable = inspect_execution_error(&err)
                        .is_some_and(|m| m.budget_exceeded)
                        || err
                            .chain()
                            .any(|c| c.is::<crate::providers::ExtensionPointError>());
                    if is_non_retryable {
                        return Err(err);
                    }

                    if attempt >= self.max_retries {
                        let (prev_retries, schema_error, timed_out, budget_exceeded, tokens_in) =
                            if let Some(meta) = inspect_execution_error(&err) {
                                (
                                    meta.retries,
                                    meta.schema_error,
                                    meta.timed_out,
                                    meta.budget_exceeded,
                                    meta.tokens_in,
                                )
                            } else {
                                let msg_lower = err.to_string().to_ascii_lowercase();
                                let is_schema = msg_lower.contains("schema")
                                    || msg_lower.contains("invalid json");
                                (0, is_schema, false, false, estimate_request_tokens(req))
                            };

                        let total_retries = prev_retries.saturating_add(attempt);
                        return Err(anyhow::Error::new(LlmExecutionError {
                            message: format!(
                                "Provider '{}' failed after {} retries: {}",
                                self.inner.alias(),
                                total_retries,
                                err
                            ),
                            retries: total_retries,
                            schema_error,
                            timed_out,
                            budget_exceeded,
                            tokens_in,
                        }));
                    }

                    attempt += 1;
                    if self.backoff_ms > 0 {
                        thread::sleep(Duration::from_millis(self.backoff_ms));
                    }
                }
            }
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn kind(&self) -> ModelKind {
        self.inner.kind()
    }

    fn alias(&self) -> &str {
        self.inner.alias()
    }
}

/// Decorator that enforces an execution time budget (`timeout_ms`) on provider calls (D13).
pub struct TimeoutDecorator {
    inner: Arc<dyn LlmProvider>,
    default_timeout_ms: Option<u64>,
}

impl TimeoutDecorator {
    /// Creates a `TimeoutDecorator` with an optional default timeout in milliseconds.
    pub fn new<P: LlmProvider + 'static>(inner: P, default_timeout_ms: Option<u64>) -> Self {
        Self {
            inner: Arc::new(inner),
            default_timeout_ms,
        }
    }

    /// Creates a `TimeoutDecorator` with an explicit timeout in milliseconds.
    pub fn with_timeout_ms<P: LlmProvider + 'static>(inner: P, timeout_ms: u64) -> Self {
        Self::new(inner, Some(timeout_ms))
    }

    /// Returns the configured default timeout in milliseconds.
    pub fn default_timeout_ms(&self) -> Option<u64> {
        self.default_timeout_ms
    }
}

impl LlmProvider for TimeoutDecorator {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse> {
        let effective_timeout = req.timeout_ms.or(self.default_timeout_ms);
        let start = Instant::now();

        let Some(budget_ms) = effective_timeout else {
            let mut resp = self.inner.complete(req)?;
            let elapsed_ms = start.elapsed().as_millis() as u64;
            if resp.latency_ms == 0 {
                resp.latency_ms = elapsed_ms;
            }
            return Ok(resp);
        };

        if budget_ms == 0 {
            return Err(anyhow::Error::new(LlmExecutionError {
                message: format!(
                    "Provider '{}' request timed out: timeout_ms budget is 0 ms",
                    self.inner.alias()
                ),
                retries: 0,
                schema_error: false,
                timed_out: true,
                budget_exceeded: false,
                tokens_in: estimate_request_tokens(req),
            }));
        }

        let mut forwarded_req = req.clone();
        forwarded_req.timeout_ms = Some(budget_ms);

        let (tx, rx) = mpsc::channel();
        let inner = Arc::clone(&self.inner);
        thread::spawn(move || {
            let res = inner.complete(&forwarded_req);
            let _ = tx.send(res);
        });

        match rx.recv_timeout(Duration::from_millis(budget_ms)) {
            Ok(Ok(mut resp)) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                let measured_ms = resp.latency_ms.max(elapsed_ms);
                if measured_ms > budget_ms {
                    return Err(anyhow::Error::new(LlmExecutionError {
                        message: format!(
                            "Provider '{}' timed out: execution took {} ms (budget: {} ms)",
                            self.inner.alias(),
                            measured_ms,
                            budget_ms
                        ),
                        retries: resp.retries,
                        schema_error: false,
                        timed_out: true,
                        budget_exceeded: false,
                        tokens_in: if resp.tokens_in > 0 {
                            resp.tokens_in
                        } else {
                            estimate_request_tokens(req)
                        },
                    }));
                }
                if resp.latency_ms == 0 {
                    resp.latency_ms = elapsed_ms;
                }
                Ok(resp)
            }
            Ok(Err(err)) => {
                if inspect_execution_error(&err).is_none()
                    && start.elapsed().as_millis() as u64 >= budget_ms
                {
                    return Err(anyhow::Error::new(LlmExecutionError {
                        message: format!(
                            "Provider '{}' timed out after {} ms: {}",
                            self.inner.alias(),
                            budget_ms,
                            err
                        ),
                        retries: 0,
                        schema_error: false,
                        timed_out: true,
                        budget_exceeded: false,
                        tokens_in: estimate_request_tokens(req),
                    }));
                }
                Err(err)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Err(anyhow::Error::new(LlmExecutionError {
                message: format!(
                    "Provider '{}' timed out after {} ms",
                    self.inner.alias(),
                    budget_ms
                ),
                retries: 0,
                schema_error: false,
                timed_out: true,
                budget_exceeded: false,
                tokens_in: estimate_request_tokens(req),
            })),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let timed_out = start.elapsed().as_millis() as u64 >= budget_ms;
                Err(anyhow::Error::new(LlmExecutionError {
                    message: format!(
                        "Provider '{}' worker thread disconnected (elapsed: {} ms, budget: {} ms)",
                        self.inner.alias(),
                        start.elapsed().as_millis(),
                        budget_ms
                    ),
                    retries: 0,
                    schema_error: false,
                    timed_out,
                    budget_exceeded: false,
                    tokens_in: estimate_request_tokens(req),
                }))
            }
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn kind(&self) -> ModelKind {
        self.inner.kind()
    }

    fn alias(&self) -> &str {
        self.inner.alias()
    }
}

/// Decorator that estimates input/output tokens and enforces `max_input_tokens` before invoking the provider (D13).
pub struct TokenBudgetDecorator {
    inner: Arc<dyn LlmProvider>,
    max_input_tokens: Option<usize>,
}

impl TokenBudgetDecorator {
    /// Creates a `TokenBudgetDecorator` with an optional default `max_input_tokens` limit.
    pub fn new<P: LlmProvider + 'static>(inner: P, max_input_tokens: Option<usize>) -> Self {
        Self {
            inner: Arc::new(inner),
            max_input_tokens,
        }
    }

    /// Creates a `TokenBudgetDecorator` with an explicit token limit.
    pub fn with_limit<P: LlmProvider + 'static>(inner: P, max_input_tokens: usize) -> Self {
        Self::new(inner, Some(max_input_tokens))
    }

    /// Returns the configured default `max_input_tokens` limit.
    pub fn max_input_tokens(&self) -> Option<usize> {
        self.max_input_tokens
    }
}

impl LlmProvider for TokenBudgetDecorator {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse> {
        let tokens_in = estimate_request_tokens(req);
        let effective_limit = match (req.max_input_tokens, self.max_input_tokens) {
            (Some(req_lim), Some(cfg_lim)) => Some(req_lim.min(cfg_lim)),
            (Some(req_lim), None) => Some(req_lim),
            (None, Some(cfg_lim)) => Some(cfg_lim),
            (None, None) => self.inner.capabilities().max_context_tokens,
        };

        if let Some(limit) = effective_limit {
            if tokens_in > limit {
                return Err(anyhow::Error::new(LlmExecutionError {
                    message: format!(
                        "Input token budget exceeded for provider '{}': estimated {} input tokens > max_input_tokens limit {}",
                        self.inner.alias(),
                        tokens_in,
                        limit
                    ),
                    retries: 0,
                    schema_error: false,
                    timed_out: false,
                    budget_exceeded: true,
                    tokens_in,
                }));
            }
        }

        let mut resp = self.inner.complete(req)?;
        if resp.tokens_in == 0 {
            resp.tokens_in = tokens_in;
        }
        if resp.tokens_out == 0 {
            resp.tokens_out = estimate_tokens(&resp.content);
        }
        Ok(resp)
    }

    fn capabilities(&self) -> ProviderCapabilities {
        let mut caps = self.inner.capabilities();
        if caps.max_context_tokens.is_none() {
            caps.max_context_tokens = self.max_input_tokens;
        }
        caps
    }

    fn kind(&self) -> ModelKind {
        self.inner.kind()
    }

    fn alias(&self) -> &str {
        self.inner.alias()
    }
}

/// Detailed telemetry record captured by `TelemetryDecorator` (R2, D29).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LlmTelemetryRecord {
    /// Configured model alias.
    pub model_alias: String,
    /// Provider kind (`ollama`, `command`, `agent`, etc.).
    pub provider_kind: ModelKind,
    /// Execution role (`init`, `update`, `task`, `search`).
    pub role: RoleName,
    /// Prompt/input token count.
    pub tokens_in: usize,
    /// Completion/output token count.
    pub tokens_out: usize,
    /// Execution latency in milliseconds.
    pub latency_ms: u64,
    /// Whether a fallback alias in the role chain was used.
    pub fallback_used: bool,
    /// Whether a JSON / JSON Schema validation error occurred.
    pub schema_error: bool,
    /// Number of retry attempts performed.
    pub retries: u32,
    /// Whether the provider call succeeded.
    pub success: bool,
}

/// Decorator that records `LlmSignal` events into `Arc<dyn TelemetrySink>` from `ci-core` (D13, D29).
pub struct TelemetryDecorator {
    inner: Arc<dyn LlmProvider>,
    sink: Arc<dyn TelemetrySink>,
    project_id: String,
    fallback_used: bool,
    records: Mutex<Vec<LlmTelemetryRecord>>,
}

impl TelemetryDecorator {
    /// Creates a `TelemetryDecorator` wrapping `inner` and recording to `sink`.
    pub fn new<P: LlmProvider + 'static>(inner: P, sink: Arc<dyn TelemetrySink>) -> Self {
        Self {
            inner: Arc::new(inner),
            sink,
            project_id: "default-project".to_string(),
            fallback_used: false,
            records: Mutex::new(Vec::new()),
        }
    }

    /// Sets the `project_id` (or hash) attached to emitted `TelemetryEvent`s.
    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = project_id.into();
        self
    }

    /// Sets whether this provider invocation is executing as a fallback in a role chain.
    pub fn with_fallback_used(mut self, fallback_used: bool) -> Self {
        self.fallback_used = fallback_used;
        self
    }

    /// Returns all `LlmTelemetryRecord` items captured by this decorator instance.
    pub fn records(&self) -> Vec<LlmTelemetryRecord> {
        self.records
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Returns the most recent `LlmTelemetryRecord` captured by this decorator instance.
    pub fn last_record(&self) -> Option<LlmTelemetryRecord> {
        self.records
            .lock()
            .ok()
            .and_then(|guard| guard.last().cloned())
    }

    fn emit_telemetry(&self, record: LlmTelemetryRecord) {
        let signal = LlmSignal {
            alias: record.model_alias.clone(),
            kind: record.provider_kind.as_str().to_string(),
            role: record.role.as_str().to_string(),
            input_tokens: record.tokens_in,
            output_tokens: record.tokens_out,
            fallback_used: record.fallback_used,
            schema_errors: usize::from(record.schema_error),
            retries: record.retries as usize,
        };
        let event = TelemetryEvent::new(&self.project_id, TelemetrySignal::Llm(signal));
        let _ = self.sink.record(&event);
        if let Ok(mut guard) = self.records.lock() {
            guard.push(record);
        }
    }
}

impl LlmProvider for TelemetryDecorator {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse> {
        let start = Instant::now();
        match self.inner.complete(req) {
            Ok(mut resp) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                if resp.latency_ms == 0 {
                    resp.latency_ms = elapsed_ms;
                }
                if self.fallback_used {
                    resp.fallback_used = true;
                }
                if resp.tokens_in == 0 {
                    resp.tokens_in = estimate_request_tokens(req);
                }
                if resp.tokens_out == 0 {
                    resp.tokens_out = estimate_tokens(&resp.content);
                }

                let record = LlmTelemetryRecord {
                    model_alias: resp.alias.clone(),
                    provider_kind: resp.provider_kind,
                    role: req.role,
                    tokens_in: resp.tokens_in,
                    tokens_out: resp.tokens_out,
                    latency_ms: resp.latency_ms,
                    fallback_used: resp.fallback_used,
                    schema_error: false,
                    retries: resp.retries,
                    success: true,
                };
                self.emit_telemetry(record);
                Ok(resp)
            }
            Err(err) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                let (retries, schema_error, tokens_in) =
                    if let Some(meta) = inspect_execution_error(&err) {
                        (meta.retries, meta.schema_error, meta.tokens_in)
                    } else {
                        let msg = err.to_string().to_ascii_lowercase();
                        let is_schema = msg.contains("schema") || msg.contains("invalid json");
                        (0, is_schema, estimate_request_tokens(req))
                    };

                let record = LlmTelemetryRecord {
                    model_alias: self.inner.alias().to_string(),
                    provider_kind: self.inner.kind(),
                    role: req.role,
                    tokens_in,
                    tokens_out: 0,
                    latency_ms: elapsed_ms,
                    fallback_used: self.fallback_used,
                    schema_error,
                    retries,
                    success: false,
                };
                self.emit_telemetry(record);
                Err(err)
            }
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn kind(&self) -> ModelKind {
        self.inner.kind()
    }

    fn alias(&self) -> &str {
        self.inner.alias()
    }
}
