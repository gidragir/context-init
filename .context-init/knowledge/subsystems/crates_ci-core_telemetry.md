---
directory: crates/ci-core/src
source:
  kind: git
  blob_sha: b0e5ea4b585339d2d0c93a70d03287a6573a0f99
  commit_sha: cd776cdaf43d357c59b46fea99176db18588f7b5
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 2
  sampled_entries: 2
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Телеметрия (L1)

Opt-in JSONL-телеметрия без путей и кода: операционные (tool, channel mcp/cli, harness), эффективность (токены ответа vs исходника, усечения, stale), LLM- и сессионные сигналы; рендер дайджеста.

> **Идентификатор:** `crates_ci-core_telemetry`
> **Теги:** jsonl, privacy, telemetry

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/telemetry.rs` | Исходный код |
| `crates/ci-core/src/telemetry/harness.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const TELEMETRY_SCHEMA_VERSION: u32 = 2
pub struct OperationalSignal
pub enum Channel
pub struct CallContext
pub enum ErrorClass
pub fn as_str(&self) -> &'static str
pub enum CallOutcome
pub fn classify_error(err: &anyhow :: Error) -> ErrorClass
pub struct Recorder
pub fn new(sink: Option<Arc<dyn TelemetrySink>>, project_id: Option<String>) -> Self
pub fn disabled() -> Self
pub fn record_call(&self, ctx: &CallContext, tool: &str, started: Instant, outcome: CallOutcome, )
pub fn is_active(&self) -> bool
pub fn record_efficiency(&self, sig: EfficiencySignal)
pub fn record(&self, signal: TelemetrySignal)
pub struct TelemetryScope
pub fn efficiency(&self, tool: &str, source_tokens: usize, response: &str, truncated: bool, cursor_hops: usize, stale: bool, empty_search: bool, )
pub struct EfficiencySignal
pub struct LlmSignal
pub struct SessionPatternSignal
pub struct FeedbackSignal
pub enum TelemetrySignal
pub struct TelemetryEvent
pub fn new(raw_or_hashed_project_id: &str, signal: TelemetrySignal) -> Self
pub fn with_timestamp(raw_or_hashed_project_id: &str, timestamp: impl Into<String>, signal: TelemetrySignal, ) -> Self
pub fn year_month(&self) -> String
pub trait TelemetrySink
fn is_enabled(&self) -> bool
fn record(&self, event: &TelemetryEvent) -> Result<()>
pub fn record_feedback(cfg: &crate :: config :: Config, storage: &crate :: config :: StoragePaths, project_id: &str, signal: FeedbackSignal, ) -> Result<bool>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
