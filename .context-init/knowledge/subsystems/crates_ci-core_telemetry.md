---
directory: crates/ci-core/src/telemetry.rs
source:
  kind: git
  blob_sha: 1a9e4f5e75742f14914fa4c7b79226338ee3c95e
  commit_sha: 4f153c6ee310b681808626eb7348ac3ca484e21d
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 1
  sampled_entries: 1
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Core Telemetry (L1)

Модуль-файл `crates/ci-core/src/telemetry.rs`

> **Идентификатор:** `crates_ci-core_telemetry`
> **Теги:** Rust (Cargo), crates_ci-core_telemetry

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/telemetry.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const TELEMETRY_SCHEMA_VERSION: u32 = 1
pub struct OperationalSignal
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
pub struct JsonlTelemetrySink
pub fn new(data_dir: impl Into<PathBuf>, enabled: bool) -> Self
pub fn with_exact_dir(telemetry_dir: impl Into<PathBuf>, enabled: bool) -> Self
pub fn telemetry_dir(&self) -> &Path
pub fn file_path_for_month(&self, year_month: &str) -> PathBuf
pub fn current_file_path(&self) -> PathBuf
pub fn read_all_events(&self) -> Result<Vec<TelemetryEvent>>
pub fn read_jsonl_file(path: &Path) -> Result<Vec<TelemetryEvent>>
pub fn render_telemetry_report(events: &[TelemetryEvent], include_notes: bool) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
