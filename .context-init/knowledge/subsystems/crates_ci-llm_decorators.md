---
directory: crates/ci-llm/src/decorators.rs
source:
  kind: git
  blob_sha: a12861766d4f9eeb4625627ce0b8ff62f7baa303
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

# Подсистема: Ci Llm Decorators (L1)

Модуль-файл `crates/ci-llm/src/decorators.rs`

> **Идентификатор:** `crates_ci-llm_decorators`
> **Теги:** Rust (Cargo), crates_ci-llm_decorators

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
