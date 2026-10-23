//! Opt-in privacy-preserving telemetry signals, JSONL sink, and Markdown digest generator (D28–D30, O6).

use crate::storage::hash_project_id;
use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Current telemetry JSONL schema version (D28).
pub const TELEMETRY_SCHEMA_VERSION: u32 = 1;

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
}

/// Context efficiency telemetry signal: response vs source tokens, truncations, cursor hops, staleness, empty search (D29).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EfficiencySignal {
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
                *op_by_tool.entry(op.tool.clone()).or_insert(0) += 1;
                if let Some(ref err_cls) = op.error_class {
                    *op_errors.entry(err_cls.clone()).or_insert(0) += 1;
                }
            }
            TelemetrySignal::Efficiency(eff) => {
                eff_count += 1;
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

    md
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
