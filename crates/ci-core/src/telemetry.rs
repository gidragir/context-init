//! Opt-in privacy-preserving telemetry signals, JSONL sink, and Markdown digest generator (D28–D30, O6).

use crate::storage::hash_project_id;
use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

mod harness;
pub use harness::{harness_from_client_name, harness_from_env, HarnessId, ANTIGRAVITY_ENV_MARKERS};

/// Current telemetry JSONL schema version (D28). Version 1 events remain readable.
pub const TELEMETRY_SCHEMA_VERSION: u32 = 2;

/// Operational telemetry signal: tool invocation, channel, harness, latency, outcome, error class, version, OS (D29).
/// Contains no file paths, source code, or metadata object names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OperationalSignal {
    /// Invoked tool or CLI command identifier (e.g. `"context_pack"`, `"init"`).
    pub tool: String,
    /// Execution channel (`"mcp"` or `"cli"`).
    pub channel: String,
    /// Client harness identifier from MCP `clientInfo` (e.g. `"antigravity"`, `"claude-code"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    /// Execution latency in milliseconds.
    pub latency_ms: u64,
    /// Whether the invocation succeeded.
    pub success: bool,
    /// High-level error classification without sensitive paths or messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<String>,
    /// `context-init` version string.
    pub version: String,
    /// Host operating system identifier (e.g. `"linux"`, `"macos"`, `"windows"`).
    pub os: String,
    /// Whether the call was made by a nested invocation (e.g. a CLI command run inside an MCP call). Absent in v1 events.
    #[serde(default)]
    pub nested: bool,
}

/// Execution channel of a recorded call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Mcp,
    Cli,
}

impl Channel {
    fn as_str(&self) -> &'static str {
        match self {
            Channel::Mcp => "mcp",
            Channel::Cli => "cli",
        }
    }
}

/// Per-call context attached to operational telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallContext {
    pub harness: HarnessId,
    pub channel: Channel,
    pub nested: bool,
}

/// High-level error class; carries no message or path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    NotFound,
    InvalidParams,
    Io,
    Drift,
    Internal,
}

impl ErrorClass {
    /// Stable string form, identical to the serde representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorClass::NotFound => "not_found",
            ErrorClass::InvalidParams => "invalid_params",
            ErrorClass::Io => "io",
            ErrorClass::Drift => "drift",
            ErrorClass::Internal => "internal",
        }
    }
}

/// Result of a recorded call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallOutcome {
    Ok,
    Err(ErrorClass),
}

/// Classifies an error into an [`ErrorClass`] without retaining its message.
pub fn classify_error(err: &anyhow::Error) -> ErrorClass {
    if err
        .chain()
        .any(|c| c.downcast_ref::<std::io::Error>().is_some())
    {
        return ErrorClass::Io;
    }
    let msg = format!("{err:#}").to_lowercase();
    if msg.contains("not found") || msg.contains("unknown subsystem") {
        ErrorClass::NotFound
    } else if msg.contains("invalid") || msg.contains("must") {
        ErrorClass::InvalidParams
    } else if msg.contains("drift") {
        ErrorClass::Drift
    } else {
        ErrorClass::Internal
    }
}

/// Fire-and-forget telemetry recorder: sink errors are swallowed so telemetry never breaks a call.
#[derive(Clone)]
pub struct Recorder {
    sink: Option<Arc<dyn TelemetrySink>>,
    project_id: Option<String>,
}

impl Recorder {
    /// Creates a recorder over `sink`; without a sink or project id nothing is written.
    pub fn new(sink: Option<Arc<dyn TelemetrySink>>, project_id: Option<String>) -> Self {
        Self { sink, project_id }
    }

    /// Recorder that never writes.
    pub fn disabled() -> Self {
        Self {
            sink: None,
            project_id: None,
        }
    }

    /// Records an operational signal for a finished call.
    pub fn record_call(
        &self,
        ctx: &CallContext,
        tool: &str,
        started: Instant,
        outcome: CallOutcome,
    ) {
        let (success, error_class) = match outcome {
            CallOutcome::Ok => (true, None),
            CallOutcome::Err(c) => (false, Some(c.as_str().to_string())),
        };
        self.record(TelemetrySignal::Operational(OperationalSignal {
            tool: tool.to_string(),
            channel: ctx.channel.as_str().to_string(),
            harness: Some(ctx.harness.as_str().to_string()),
            latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            success,
            error_class,
            version: crate::VERSION.to_string(),
            os: std::env::consts::OS.to_string(),
            nested: ctx.nested,
        }));
    }

    /// Whether a record would actually be written (sink present, enabled, project known).
    pub fn is_active(&self) -> bool {
        self.project_id.is_some() && self.sink.as_ref().is_some_and(|s| s.is_enabled())
    }

    /// Records a context-efficiency signal.
    pub fn record_efficiency(&self, sig: EfficiencySignal) {
        self.record(TelemetrySignal::Efficiency(sig));
    }

    /// Records any signal; no-op when disabled, and sink errors are ignored.
    pub fn record(&self, signal: TelemetrySignal) {
        let (Some(sink), Some(project_id)) = (&self.sink, &self.project_id) else {
            return;
        };
        if !sink.is_enabled() {
            return;
        }
        let _ = sink.record(&TelemetryEvent::new(project_id, signal));
    }
}

/// Recorder bundled with the per-call context; handlers record efficiency through it.
#[derive(Clone)]
pub struct TelemetryScope {
    pub recorder: Recorder,
    pub ctx: CallContext,
}

impl TelemetryScope {
    /// Scope that never writes (CLI paths without telemetry wiring).
    pub fn disabled() -> Self {
        Self {
            recorder: Recorder::disabled(),
            ctx: CallContext {
                harness: HarnessId::Other,
                channel: Channel::Cli,
                nested: false,
            },
        }
    }

    /// Whether recording would write anything; lets callers skip computing source tokens.
    pub fn is_active(&self) -> bool {
        self.recorder.is_active()
    }

    /// Records one efficiency signal; `response_tokens` is counted from `response`.
    #[allow(clippy::too_many_arguments)]
    pub fn efficiency(
        &self,
        tool: &str,
        source_tokens: usize,
        response: &str,
        truncated: bool,
        cursor_hops: usize,
        stale: bool,
        empty_search: bool,
    ) {
        self.recorder.record_efficiency(EfficiencySignal {
            harness: Some(self.ctx.harness.as_str().to_string()),
            tool: tool.to_string(),
            response_tokens: crate::tokens::count_tokens(response),
            source_tokens,
            truncated,
            cursor_hops,
            stale,
            empty_search,
        });
    }
}

/// Context efficiency telemetry signal: response vs source tokens, truncations, cursor hops, staleness, empty search (D29).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EfficiencySignal {
    /// Harness identifier (e.g. `"claude-code"`, `"antigravity"`), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    /// Tool identifier.
    pub tool: String,
    /// Number of tokens returned in the response.
    pub response_tokens: usize,
    /// Number of raw source tokens before compression/budgeting.
    pub source_tokens: usize,
    /// Whether the response was truncated at `max_response_tokens`.
    pub truncated: bool,
    /// Pagination cursor hop index (`0` for initial request, `>0` for subsequent pages).
    pub cursor_hops: usize,
    /// Whether the accessed card was marked `stale`.
    pub stale: bool,
    /// Whether a search query returned zero matches.
    pub empty_search: bool,
}

/// LLM execution telemetry signal: alias, provider kind, role, tokens in/out, fallback, schema errors, retries (D29).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LlmSignal {
    /// Configured model alias (e.g. `"local"`).
    pub alias: String,
    /// Provider kind (`"ollama"`, `"command"`, `"agent"`, etc.).
    pub kind: String,
    /// Execution role (`"init"`, `"update"`, `"task"`, `"search"`).
    pub role: String,
    /// Prompt/input token count.
    pub input_tokens: usize,
    /// Completion/output token count.
    pub output_tokens: usize,
    /// Whether a fallback alias in the role chain was used.
    pub fallback_used: bool,
    /// Count of JSON schema validation errors encountered.
    pub schema_errors: usize,
    /// Number of retry attempts performed.
    pub retries: usize,
}

/// Session call-sequence pattern signal for detecting agent navigation loops / wandering (D29).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionPatternSignal {
    /// Sequence of tool names invoked in the session window (no arguments or paths).
    pub call_sequence: Vec<String>,
    /// Whether repetitive/wandering tool usage was detected.
    pub wandering_detected: bool,
}

/// Explicit agent/user feedback signal from `context_feedback` or CLI `feedback` (D29, O6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeedbackSignal {
    /// Target tool name being rated.
    pub tool: String,
    /// Verdict label (e.g. `"helpful"`, `"unhelpful"`, `"accurate"`, `"stale"`).
    pub verdict: String,
    /// Optional free-text note. Hidden by default in `render_telemetry_report` until manual review (O6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Tagged union of all supported telemetry signal payloads (D29).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "signal_type", rename_all = "snake_case")]
pub enum TelemetrySignal {
    Operational(OperationalSignal),
    Efficiency(EfficiencySignal),
    Llm(LlmSignal),
    SessionPattern(SessionPatternSignal),
    Feedback(FeedbackSignal),
}

fn default_telemetry_schema_version() -> u32 {
    TELEMETRY_SCHEMA_VERSION
}

/// Single JSONL telemetry event record with `schema_version` and SHA-256 hashed `project-id` (D28).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TelemetryEvent {
    /// Telemetry schema version (`1`).
    #[serde(default = "default_telemetry_schema_version")]
    pub schema_version: u32,
    /// UTC timestamp in `YYYY-MM-DD` or ISO-8601 format (`YYYY-MM-DDTHH:MM:SSZ`).
    pub timestamp: String,
    /// SHA-256 anonymized `project-id` hash (never a raw path or unhashed identifier).
    pub project_id_hash: String,
    /// Signal payload.
    pub signal: TelemetrySignal,
}

impl TelemetryEvent {
    /// Creates a new `TelemetryEvent` for `raw_or_hashed_project_id`, automatically hashing the ID
    /// with SHA-256 if it is not already a 64-character lowercase hex digest (D28).
    pub fn new(raw_or_hashed_project_id: &str, signal: TelemetrySignal) -> Self {
        Self {
            schema_version: TELEMETRY_SCHEMA_VERSION,
            timestamp: current_utc_date(),
            project_id_hash: ensure_hashed_project_id(raw_or_hashed_project_id),
            signal,
        }
    }

    /// Creates a `TelemetryEvent` with an explicit timestamp string.
    pub fn with_timestamp(
        raw_or_hashed_project_id: &str,
        timestamp: impl Into<String>,
        signal: TelemetrySignal,
    ) -> Self {
        Self {
            schema_version: TELEMETRY_SCHEMA_VERSION,
            timestamp: timestamp.into(),
            project_id_hash: ensure_hashed_project_id(raw_or_hashed_project_id),
            signal,
        }
    }

    /// Extracts the `YYYY-MM` month bucket from `self.timestamp`, falling back to the current UTC month.
    pub fn year_month(&self) -> String {
        extract_year_month(&self.timestamp).unwrap_or_else(current_utc_year_month)
    }
}

/// Extension point trait for recording telemetry events (D30).
pub trait TelemetrySink: Send + Sync {
    /// Returns `true` if telemetry collection is enabled.
    fn is_enabled(&self) -> bool;

    /// Records a single telemetry event if enabled; no-op when disabled.
    fn record(&self, event: &TelemetryEvent) -> Result<()>;
}

/// Records an explicit feedback signal (CLI `feedback`, MCP `context_feedback`, D29).
/// Returns `Ok(false)` without writing when telemetry is disabled (opt-in, D28).
pub fn record_feedback(
    cfg: &crate::config::Config,
    storage: &crate::config::StoragePaths,
    project_id: &str,
    signal: FeedbackSignal,
) -> Result<bool> {
    if !cfg.telemetry.enabled {
        return Ok(false);
    }
    let sink = JsonlTelemetrySink::with_exact_dir(&storage.telemetry_dir, true);
    sink.record(&TelemetryEvent::new(
        project_id,
        TelemetrySignal::Feedback(signal),
    ))?;
    Ok(true)
}

/// JSONL file telemetry sink writing to `<data_dir>/context-init/telemetry/YYYY-MM.jsonl`
/// only when `enabled == true` (D28, D30).
#[derive(Debug, Clone)]
pub struct JsonlTelemetrySink {
    telemetry_dir: PathBuf,
    enabled: bool,
}

impl JsonlTelemetrySink {
    /// Creates a `JsonlTelemetrySink` from a base data directory (or telemetry directory) and `enabled` flag.
    ///
    /// Path normalization rules:
    /// - If `data_dir` already ends with `context-init/telemetry` or `telemetry`, uses `data_dir` directly.
    /// - If `data_dir` ends with `context-init` (such as `StoragePaths::data_dir`), uses `<data_dir>/telemetry`.
    /// - Otherwise (e.g. `$XDG_DATA_HOME`), uses `<data_dir>/context-init/telemetry`.
    pub fn new(data_dir: impl Into<PathBuf>, enabled: bool) -> Self {
        let base = data_dir.into();
        let telemetry_dir = resolve_telemetry_dir(&base);
        Self {
            telemetry_dir,
            enabled,
        }
    }

    /// Creates a `JsonlTelemetrySink` using the exact directory provided without appending `context-init/telemetry`.
    pub fn with_exact_dir(telemetry_dir: impl Into<PathBuf>, enabled: bool) -> Self {
        Self {
            telemetry_dir: telemetry_dir.into(),
            enabled,
        }
    }

    /// Returns the resolved directory where `YYYY-MM.jsonl` files are stored.
    pub fn telemetry_dir(&self) -> &Path {
        &self.telemetry_dir
    }

    /// Returns the JSONL file path for a specific `YYYY-MM` bucket.
    pub fn file_path_for_month(&self, year_month: &str) -> PathBuf {
        self.telemetry_dir.join(format!("{year_month}.jsonl"))
    }

    /// Returns the JSONL file path for the current UTC month.
    pub fn current_file_path(&self) -> PathBuf {
        self.file_path_for_month(&current_utc_year_month())
    }

    /// Reads and parses all `TelemetryEvent` records across all `.jsonl` files in `self.telemetry_dir`.
    pub fn read_all_events(&self) -> Result<Vec<TelemetryEvent>> {
        if !self.telemetry_dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut files: Vec<PathBuf> = fs::read_dir(&self.telemetry_dir)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
            .collect();
        files.sort();

        let mut events = Vec::new();
        for file in files {
            events.extend(read_jsonl_file(&file)?);
        }
        Ok(events)
    }
}

impl TelemetrySink for JsonlTelemetrySink {
    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn record(&self, event: &TelemetryEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        fs::create_dir_all(&self.telemetry_dir).with_context(|| {
            format!(
                "Failed to create telemetry directory {:?}",
                self.telemetry_dir
            )
        })?;

        // Guarantee schema_version and hashed project_id on serialization
        let sanitized_event = TelemetryEvent {
            schema_version: if event.schema_version == 0 {
                TELEMETRY_SCHEMA_VERSION
            } else {
                event.schema_version
            },
            timestamp: if event.timestamp.trim().is_empty() {
                current_utc_date()
            } else {
                event.timestamp.clone()
            },
            project_id_hash: ensure_hashed_project_id(&event.project_id_hash),
            signal: event.signal.clone(),
        };

        let file_path = self.file_path_for_month(&sanitized_event.year_month());
        let mut line = serde_json::to_string(&sanitized_event)
            .context("Failed to serialize TelemetryEvent to JSON")?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .with_context(|| format!("Failed to open telemetry JSONL file {:?}", file_path))?;

        file.write_all(line.as_bytes())
            .with_context(|| format!("Failed to write telemetry event to {:?}", file_path))?;

        Ok(())
    }
}

/// Reads `TelemetryEvent` items from a single `.jsonl` file.
pub fn read_jsonl_file(path: &Path) -> Result<Vec<TelemetryEvent>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read telemetry file {:?}", path))?;
    let mut events = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let ev: TelemetryEvent = serde_json::from_str(trimmed).with_context(|| {
            format!(
                "Failed to parse telemetry JSONL line {} in {:?}",
                idx + 1,
                path
            )
        })?;
        events.push(ev);
    }
    Ok(events)
}

/// Renders an aggregated AI-ready telemetry digest in Markdown with YAML frontmatter (D30, O6).
///
/// By default (`include_notes == false`), free-text `note` fields from `FeedbackSignal` are
/// omitted/redacted from the output to prevent leaking sensitive identifiers prior to manual review (O6).
pub fn render_telemetry_report(events: &[TelemetryEvent], include_notes: bool) -> String {
    let mut unique_projects = BTreeSet::new();

    let mut op_count = 0usize;
    let mut op_success = 0usize;
    let mut op_total_latency_ms = 0u64;
    let mut op_max_latency_ms = 0u64;
    let mut op_by_tool: BTreeMap<String, usize> = BTreeMap::new();
    let mut op_errors: BTreeMap<String, usize> = BTreeMap::new();

    let mut op_groups: OpGroups = BTreeMap::new();
    let mut op_nested = 0usize;
    // Efficiency savings grouped by harness ("unknown" if absent).
    let mut savings_by_harness: BTreeMap<String, usize> = BTreeMap::new();
    let mut search_total = 0usize;
    let mut search_empty = 0usize;

    let mut eff_count = 0usize;
    let mut eff_response_tokens = 0usize;
    let mut eff_source_tokens = 0usize;
    let mut eff_truncated = 0usize;
    let mut eff_cursor_hops = 0usize;
    let mut eff_stale = 0usize;
    let mut eff_empty_search = 0usize;

    let mut llm_count = 0usize;
    let mut llm_input_tokens = 0usize;
    let mut llm_output_tokens = 0usize;
    let mut llm_fallbacks = 0usize;
    let mut llm_schema_errors = 0usize;
    let mut llm_retries = 0usize;
    let mut llm_by_role: BTreeMap<String, usize> = BTreeMap::new();

    let mut session_count = 0usize;
    let mut session_wandering = 0usize;

    let mut feedback_count = 0usize;
    let mut feedback_by_verdict: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut feedback_notes: Vec<(String, String, String)> = Vec::new();

    for ev in events {
        if !ev.project_id_hash.is_empty() {
            unique_projects.insert(ev.project_id_hash.clone());
        }
        match &ev.signal {
            TelemetrySignal::Operational(op) => {
                op_count += 1;
                if op.success {
                    op_success += 1;
                }
                op_total_latency_ms = op_total_latency_ms.saturating_add(op.latency_ms);
                if op.latency_ms > op_max_latency_ms {
                    op_max_latency_ms = op.latency_ms;
                }
                let group = op_groups
                    .entry((
                        op.harness.clone().unwrap_or_else(|| "unknown".to_string()),
                        op.channel.clone(),
                        op.tool.clone(),
                    ))
                    .or_default();
                group.0 += 1;
                if !op.success {
                    group.1 += 1;
                }
                group.2.push(op.latency_ms);
                if op.nested {
                    op_nested += 1;
                }
                *op_by_tool.entry(op.tool.clone()).or_insert(0) += 1;
                if let Some(ref err_cls) = op.error_class {
                    *op_errors.entry(err_cls.clone()).or_insert(0) += 1;
                }
            }
            TelemetrySignal::Efficiency(eff) => {
                eff_count += 1;
                let saved = eff.source_tokens.saturating_sub(eff.response_tokens);
                let harness = eff.harness.as_deref().unwrap_or("unknown");
                let entry = savings_by_harness.entry(harness.to_string()).or_insert(0);
                *entry = entry.saturating_add(saved);
                if matches!(eff.tool.as_str(), "context_search" | "search") {
                    search_total += 1;
                    if eff.empty_search {
                        search_empty += 1;
                    }
                }
                eff_response_tokens = eff_response_tokens.saturating_add(eff.response_tokens);
                eff_source_tokens = eff_source_tokens.saturating_add(eff.source_tokens);
                if eff.truncated {
                    eff_truncated += 1;
                }
                eff_cursor_hops = eff_cursor_hops.saturating_add(eff.cursor_hops);
                if eff.stale {
                    eff_stale += 1;
                }
                if eff.empty_search {
                    eff_empty_search += 1;
                }
            }
            TelemetrySignal::Llm(llm) => {
                llm_count += 1;
                llm_input_tokens = llm_input_tokens.saturating_add(llm.input_tokens);
                llm_output_tokens = llm_output_tokens.saturating_add(llm.output_tokens);
                if llm.fallback_used {
                    llm_fallbacks += 1;
                }
                llm_schema_errors = llm_schema_errors.saturating_add(llm.schema_errors);
                llm_retries = llm_retries.saturating_add(llm.retries);
                *llm_by_role.entry(llm.role.clone()).or_insert(0) += 1;
            }
            TelemetrySignal::SessionPattern(sp) => {
                session_count += 1;
                if sp.wandering_detected {
                    session_wandering += 1;
                }
            }
            TelemetrySignal::Feedback(fb) => {
                feedback_count += 1;
                *feedback_by_verdict
                    .entry((fb.tool.clone(), fb.verdict.clone()))
                    .or_insert(0) += 1;
                if let Some(ref note) = fb.note {
                    let trimmed = note.trim();
                    if !trimmed.is_empty() {
                        feedback_notes.push((
                            fb.tool.clone(),
                            fb.verdict.clone(),
                            trimmed.to_string(),
                        ));
                    }
                }
            }
        }
    }

    let avg_latency_ms = if op_count > 0 {
        op_total_latency_ms / (op_count as u64)
    } else {
        0
    };

    let notes_count = feedback_notes.len();
    let redacted_notes_count = if include_notes { 0 } else { notes_count };

    let mut md = String::new();
    md.push_str("---\n");
    md.push_str(&format!("schema_version: {}\n", TELEMETRY_SCHEMA_VERSION));
    md.push_str(&format!("total_events: {}\n", events.len()));
    md.push_str(&format!("unique_projects: {}\n", unique_projects.len()));
    md.push_str(&format!("operational_events: {}\n", op_count));
    md.push_str(&format!("efficiency_events: {}\n", eff_count));
    md.push_str(&format!("llm_events: {}\n", llm_count));
    md.push_str(&format!("session_pattern_events: {}\n", session_count));
    md.push_str(&format!("feedback_events: {}\n", feedback_count));
    md.push_str(&format!("feedback_notes_total: {}\n", notes_count));
    md.push_str(&format!(
        "feedback_notes_redacted: {}\n",
        redacted_notes_count
    ));
    md.push_str(&format!("include_notes: {}\n", include_notes));
    md.push_str("---\n\n");

    md.push_str("# Context-Init Telemetry Digest\n\n");

    md.push_str("## 1. Operational Signals\n");
    md.push_str(&format!(
        "- Total calls: {}\n- Successful calls: {}\n- Avg latency: {} ms (max: {} ms)\n",
        op_count, op_success, avg_latency_ms, op_max_latency_ms
    ));
    if !op_by_tool.is_empty() {
        md.push_str("- Calls by tool:\n");
        for (tool, cnt) in &op_by_tool {
            md.push_str(&format!("  - `{}`: {}\n", tool, cnt));
        }
    }
    if !op_errors.is_empty() {
        md.push_str("- Error classes:\n");
        for (err_cls, cnt) in &op_errors {
            md.push_str(&format!("  - `{}`: {}\n", err_cls, cnt));
        }
    }
    md.push('\n');

    md.push_str("## 2. Context Efficiency\n");
    md.push_str(&format!(
        "- Samples: {}\n- Response tokens: {}\n- Source tokens: {}\n- Truncated responses: {}\n- Cursor hops: {}\n- Stale accesses: {}\n- Empty searches: {}\n\n",
        eff_count,
        eff_response_tokens,
        eff_source_tokens,
        eff_truncated,
        eff_cursor_hops,
        eff_stale,
        eff_empty_search
    ));

    md.push_str("## 3. LLM Routing & Execution\n");
    md.push_str(&format!(
        "- Invocations: {}\n- Input tokens: {}\n- Output tokens: {}\n- Fallbacks triggered: {}\n- Schema errors: {}\n- Retries: {}\n",
        llm_count,
        llm_input_tokens,
        llm_output_tokens,
        llm_fallbacks,
        llm_schema_errors,
        llm_retries
    ));
    if !llm_by_role.is_empty() {
        md.push_str("- Calls by role:\n");
        for (role, cnt) in &llm_by_role {
            md.push_str(&format!("  - `{}`: {}\n", role, cnt));
        }
    }
    md.push('\n');

    md.push_str("## 4. Session Patterns\n");
    md.push_str(&format!(
        "- Recorded sequences: {}\n- Wandering detected: {}\n\n",
        session_count, session_wandering
    ));

    md.push_str("## 5. Feedback Summary\n");
    md.push_str(&format!("- Total feedback entries: {}\n", feedback_count));
    for ((tool, verdict), cnt) in &feedback_by_verdict {
        md.push_str(&format!("  - `{}` [{}]: {}\n", tool, verdict, cnt));
    }
    if include_notes {
        if !feedback_notes.is_empty() {
            md.push_str("- Notes (manual review enabled):\n");
            for (tool, verdict, note) in &feedback_notes {
                md.push_str(&format!("  - `{}` ({}): {}\n", tool, verdict, note));
            }
        }
    } else if notes_count > 0 {
        md.push_str(&format!(
            "- Notes redacted: {} (pass include_notes=true after manual review to inspect free-text notes)\n",
            notes_count
        ));
    }

    md.push_str("\n## По харнессам и каналам\n\n");
    md.push_str("| харнесс | канал | инструмент | вызовы | ошибки, % | p50, мс |\n");
    md.push_str("|---|---|---|---|---|---|\n");
    for ((harness, channel, tool), (calls, errors, latencies)) in &mut op_groups {
        latencies.sort_unstable();
        // Lower median: for an even count take the lower of the two middle values.
        let p50 = latencies[(latencies.len() - 1) / 2];
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            harness,
            channel,
            tool,
            calls,
            percent(*errors, *calls),
            p50
        ));
    }

    md.push_str("\n## Вложенные вызовы\n\n");
    md.push_str(&format!(
        "- Вложенных: {} из {} ({}%)\n",
        op_nested,
        op_count,
        percent(op_nested, op_count)
    ));

    md.push_str("\n## Экономия токенов\n\n");
    let total_saved: usize = savings_by_harness.values().sum();
    md.push_str(&format!("- Сэкономлено токенов: {}\n", total_saved));
    for (harness, saved) in &savings_by_harness {
        md.push_str(&format!("  - `{}`: {}\n", harness, saved));
    }

    md.push_str("\n## Пустой поиск\n\n");
    md.push_str(&format!(
        "- Пустых: {} из {} ({}%)\n",
        search_empty,
        search_total,
        percent(search_empty, search_total)
    ));

    md
}

/// `(harness, channel, tool)` -> `(calls, errors, latencies)`.
type OpGroups = BTreeMap<(String, String, String), (usize, usize, Vec<u64>)>;

/// Integer percentage `part / total`, rounded to nearest; `0` when `total == 0`.
fn percent(part: usize, total: usize) -> usize {
    (part * 100 + total / 2).checked_div(total).unwrap_or(0)
}

fn resolve_telemetry_dir(base: &Path) -> PathBuf {
    let file_name = base
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if file_name == "telemetry" {
        base.to_path_buf()
    } else if file_name == "context-init" {
        base.join("telemetry")
    } else {
        base.join("context-init").join("telemetry")
    }
}

fn ensure_hashed_project_id(id: &str) -> String {
    let trimmed = id.trim();
    if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        trimmed.to_ascii_lowercase()
    } else {
        hash_project_id(trimmed)
    }
}

fn extract_year_month(timestamp: &str) -> Option<String> {
    let t = timestamp.trim();
    if t.len() >= 7 {
        let bytes = t.as_bytes();
        let valid_year = bytes[0..4].iter().all(u8::is_ascii_digit);
        let valid_sep = bytes[4] == b'-';
        let valid_month = bytes[5..7].iter().all(u8::is_ascii_digit);
        if valid_year && valid_sep && valid_month {
            return Some(t[..7].to_string());
        }
    }
    None
}

fn current_utc_year_month() -> String {
    let date = current_utc_date();
    date[..7].to_string()
}

fn current_utc_date() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let mut year = 1970u64;
    let mut remaining_days = days;

    loop {
        let leap =
            (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
        let days_in_year = if leap { 366 } else { 365 };
        if remaining_days >= days_in_year {
            remaining_days -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }

    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    let days_in_months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &d in &days_in_months {
        if remaining_days >= d {
            remaining_days -= d;
            month += 1;
        } else {
            break;
        }
    }
    let day = remaining_days + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}

#[cfg(test)]
mod record_feedback_tests {
    use super::*;
    use crate::config::{Config, StoragePaths};

    #[test]
    fn record_feedback_respects_enabled_flag() {
        let data = std::env::temp_dir().join(format!(
            "ci_feedback_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let storage = StoragePaths::resolve_with_env(None, Some(&data), |k: &str| {
            (k == "XDG_DATA_HOME").then(|| data.join("xdg").to_string_lossy().into_owned())
        });
        let signal = || FeedbackSignal {
            tool: "context_pack".to_string(),
            verdict: "helpful".to_string(),
            note: None,
        };

        let mut cfg = Config::default();
        assert!(!record_feedback(&cfg, &storage, "raw-project", signal()).unwrap());
        assert!(!storage.telemetry_dir.exists());

        cfg.telemetry.enabled = true;
        assert!(record_feedback(&cfg, &storage, "raw-project", signal()).unwrap());
        let events = JsonlTelemetrySink::with_exact_dir(&storage.telemetry_dir, true)
            .read_all_events()
            .unwrap();
        assert_eq!(events.len(), 1);
        let line = serde_json::to_string(&events[0]).unwrap();
        assert!(
            !line.contains("raw-project"),
            "project id must be hashed: {line}"
        );
        let _ = fs::remove_dir_all(&data);
    }
}

#[cfg(test)]
mod recorder_tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    fn unique_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ci_{tag}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    struct FailingSink;
    impl TelemetrySink for FailingSink {
        fn is_enabled(&self) -> bool {
            true
        }
        fn record(&self, _event: &TelemetryEvent) -> Result<()> {
            anyhow::bail!("disk full")
        }
    }

    #[test]
    fn harness_from_client_name_normalizes() {
        assert_eq!(
            harness_from_client_name(Some("claude-code")),
            HarnessId::ClaudeCode
        );
        assert_eq!(
            harness_from_client_name(Some("Antigravity")),
            HarnessId::Antigravity
        );
        assert_eq!(harness_from_client_name(Some("weird")), HarnessId::Other);
        assert_eq!(harness_from_client_name(None), HarnessId::Other);
    }

    #[test]
    fn harness_from_env_detects_claude() {
        assert_eq!(
            harness_from_env(|k| k == "CLAUDECODE"),
            HarnessId::ClaudeCode
        );
        assert_eq!(harness_from_env(|_| false), HarnessId::Terminal);
    }

    #[test]
    fn recorder_writes_operational_v2() {
        let dir = unique_dir("rec_v2");
        let sink = Arc::new(JsonlTelemetrySink::with_exact_dir(&dir, true));
        let rec = Recorder::new(Some(sink.clone()), Some("raw-project".to_string()));
        let ctx = CallContext {
            harness: HarnessId::ClaudeCode,
            channel: Channel::Mcp,
            nested: true,
        };
        rec.record_call(
            &ctx,
            "context_pack",
            Instant::now(),
            CallOutcome::Err(ErrorClass::Io),
        );
        let events = sink.read_all_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].schema_version, 2);
        let TelemetrySignal::Operational(op) = &events[0].signal else {
            panic!("expected operational signal");
        };
        assert_eq!(op.harness.as_deref(), Some("claude-code"));
        assert_eq!(op.channel, "mcp");
        assert!(op.nested);
        assert!(!op.success);
        assert_eq!(op.error_class.as_deref(), Some("io"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recorder_swallows_sink_errors() {
        let rec = Recorder::new(Some(Arc::new(FailingSink)), Some("p".to_string()));
        let ctx = CallContext {
            harness: HarnessId::Other,
            channel: Channel::Cli,
            nested: false,
        };
        rec.record_call(&ctx, "init", Instant::now(), CallOutcome::Ok);
        rec.record_efficiency(EfficiencySignal::default());
        Recorder::disabled().record_call(&ctx, "init", Instant::now(), CallOutcome::Ok);
    }

    #[test]
    fn telemetry_scope_efficiency_populates_harness() {
        let dir = unique_dir("scope_eff");
        let sink = Arc::new(JsonlTelemetrySink::with_exact_dir(&dir, true));
        let rec = Recorder::new(Some(sink.clone()), Some("raw-project".to_string()));
        let scope = TelemetryScope {
            recorder: rec,
            ctx: CallContext {
                harness: HarnessId::ClaudeCode,
                channel: Channel::Mcp,
                nested: false,
            },
        };
        scope.efficiency("context_pack", 1000, "resp", false, 0, false, false);
        let events = sink.read_all_events().unwrap();
        assert_eq!(events.len(), 1);
        let TelemetrySignal::Efficiency(eff) = &events[0].signal else {
            panic!("expected efficiency signal");
        };
        assert_eq!(eff.harness.as_deref(), Some("claude-code"));
        assert_eq!(eff.tool, "context_pack");
        assert_eq!(eff.source_tokens, 1000);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_v1_events_without_nested() {
        let dir = unique_dir("rec_v1");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-01.jsonl");
        let hash = "a".repeat(64);
        let line = format!(
            r#"{{"schema_version":1,"timestamp":"2026-01-02","project_id_hash":"{hash}","signal":{{"signal_type":"operational","tool":"init","channel":"cli","latency_ms":5,"success":true,"version":"0.1.0","os":"linux"}}}}"#
        );
        fs::write(&path, format!("{line}\n")).unwrap();
        let events = read_jsonl_file(&path).unwrap();
        assert_eq!(events[0].schema_version, 1);
        let TelemetrySignal::Operational(op) = &events[0].signal else {
            panic!("expected operational signal");
        };
        assert!(!op.nested);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn classify_error_maps_io_and_not_found() {
        let io = anyhow::Error::new(std::io::Error::other("boom")).context("reading");
        assert_eq!(classify_error(&io), ErrorClass::Io);
        assert_eq!(
            classify_error(&anyhow::anyhow!("Unknown subsystem: x")),
            ErrorClass::NotFound
        );
        assert_eq!(
            classify_error(&anyhow::anyhow!("card not found")),
            ErrorClass::NotFound
        );
        assert_eq!(
            classify_error(&anyhow::anyhow!("name must not be empty")),
            ErrorClass::InvalidParams
        );
        assert_eq!(
            classify_error(&anyhow::anyhow!("architectural drift")),
            ErrorClass::Drift
        );
        assert_eq!(
            classify_error(&anyhow::anyhow!("weird")),
            ErrorClass::Internal
        );
    }
}

#[cfg(test)]
mod report_usage_tests {
    use super::*;

    fn op(harness: &str, channel: &str, tool: &str, ok: bool, nested: bool) -> TelemetryEvent {
        TelemetryEvent::with_timestamp(
            "p",
            "2026-01-02",
            TelemetrySignal::Operational(OperationalSignal {
                tool: tool.to_string(),
                channel: channel.to_string(),
                harness: Some(harness.to_string()),
                latency_ms: 10,
                success: ok,
                error_class: (!ok).then(|| "io".to_string()),
                version: "0.1.0".to_string(),
                os: "linux".to_string(),
                nested,
            }),
        )
    }

    fn eff(
        harness: Option<&str>,
        tool: &str,
        source: usize,
        response: usize,
        empty: bool,
    ) -> TelemetryEvent {
        TelemetryEvent::with_timestamp(
            "p",
            "2026-01-02",
            TelemetrySignal::Efficiency(EfficiencySignal {
                harness: harness.map(str::to_string),
                tool: tool.to_string(),
                response_tokens: response,
                source_tokens: source,
                empty_search: empty,
                ..Default::default()
            }),
        )
    }

    #[test]
    fn report_groups_by_harness_and_channel() {
        let events = vec![
            op("claude-code", "mcp", "context_pack", true, false),
            op("claude-code", "mcp", "context_pack", true, true),
            op("antigravity", "cli", "pack", false, false),
            eff(Some("claude-code"), "context_pack", 5000, 1000, false),
            eff(Some("claude-code"), "context_pack", 2000, 3000, false),
            eff(Some("antigravity"), "context_pack", 1500, 500, false),
            eff(None, "context_pack", 1000, 500, false),
            eff(Some("claude-code"), "context_search", 0, 0, true),
            eff(None, "context_search", 5, 5, false),
        ];
        let report = render_telemetry_report(&events, false);
        assert!(report.contains("## По харнессам и каналам"), "{report}");
        assert!(
            report.contains("claude-code | mcp | context_pack | 2 | 0"),
            "{report}"
        );
        assert!(
            report.contains("antigravity | cli | pack | 1 | 100"),
            "{report}"
        );
        assert!(report.contains("## Вложенные вызовы"), "{report}");
        assert!(report.contains("Вложенных: 1 из 3 (33%)"), "{report}");
        assert!(report.contains("## Экономия токенов"), "{report}");
        assert!(report.contains("Сэкономлено токенов: 5500"), "{report}");
        assert!(report.contains("- `antigravity`: 1000"), "{report}");
        assert!(report.contains("- `claude-code`: 4000"), "{report}");
        assert!(report.contains("- `unknown`: 500"), "{report}");
        assert!(report.contains("## Пустой поиск"), "{report}");
        assert!(report.contains("Пустых: 1 из 2 (50%)"), "{report}");
    }
}
