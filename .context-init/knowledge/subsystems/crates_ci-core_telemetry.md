---
directory: crates/ci-core/src
source:
  kind: git
  blob_sha: aa624a03f8a512b11f43586d5ac53dbf79029e42
  commit_sha: b040800534c30a22a394125140a07068cb5d9d25
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
