---
directory: crates/ci-core/src/telemetry.rs
source:
  kind: git
  blob_sha: 1a9e4f5e75742f14914fa4c7b79226338ee3c95e
  commit_sha: 36198169b6c3e1dac832cf4204e7b96c6805df94
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
pub trait TelemetrySink
pub fn record_feedback(cfg: &crate :: config :: Config, storage: &crate :: config :: StoragePaths, project_id: &str, signal: FeedbackSignal, ) -> Result<bool>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
