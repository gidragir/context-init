# Подсистема: Ci Llm Decorators (L1)

> **Идентификатор:** `crates_ci-llm_decorators`
> **Теги:** Rust (Cargo), crates_ci-llm_decorators

## 🎯 Назначение и роль
Модуль-файл `crates/ci-llm/src/decorators.rs`

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/src/decorators.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct RetryDecorator
pub struct TimeoutDecorator
pub struct TokenBudgetDecorator
pub struct LlmTelemetryRecord
pub struct TelemetryDecorator
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
