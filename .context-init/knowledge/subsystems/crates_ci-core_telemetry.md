---
directory: crates/ci-core/src
source:
  kind: git
  blob_sha: c8a2a7135af911e689e7325b7797a27e16dd693c
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
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
