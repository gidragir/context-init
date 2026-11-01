---
directory: crates/ci-llm/src/decorators.rs
source:
  kind: git
  blob_sha: a12861766d4f9eeb4625627ce0b8ff62f7baa303
  commit_sha: 2ea8f3affd3c7e26633575afd792de4875d13585
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 1
  sampled_entries: 1
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Декораторы провайдеров (L1)

RetryDecorator с backoff и TimeoutDecorator — обёртки над LlmProvider для повторов и таймаутов.

> **Идентификатор:** `crates_ci-llm_decorators`
> **Теги:** llm, retry, timeout

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
