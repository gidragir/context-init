# Подсистема: Ci Core Telemetry (L1)

> **Идентификатор:** `crates_ci-core_telemetry`
> **Теги:** Rust (Cargo), crates_ci-core_telemetry

## 🎯 Назначение и роль
Модуль-файл `crates/ci-core/src/telemetry.rs`

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
