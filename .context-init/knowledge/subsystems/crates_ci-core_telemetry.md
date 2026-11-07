---
directory: crates/ci-core/src
source:
  kind: git
  blob_sha: f3f97a5d63562ee2e55d5c8a7d389b7043d1e089
  commit_sha: 05b17436dd0087469fb8bef6c61f16e554831faa
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 2
  sampled_entries: 2
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Core Telemetry (L1)

Модуль и компоненты директории `crates/ci-core/src/telemetry` (2 файлов)

> **Идентификатор:** `crates_ci-core_telemetry`
> **Теги:** Rust (Cargo), crates_ci-core_telemetry

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
pub enum CallOutcome
pub fn classify_error(err: &anyhow :: Error) -> ErrorClass
pub struct Recorder
pub struct TelemetryScope
pub struct EfficiencySignal
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
