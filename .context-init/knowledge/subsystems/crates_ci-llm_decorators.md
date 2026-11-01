---
directory: crates/ci-llm/src/decorators.rs
source:
  kind: git
  blob_sha: a12861766d4f9eeb4625627ce0b8ff62f7baa303
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
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
pub fn new<P: LlmProvider + 'static>(inner: P, max_retries: u32) -> Self
pub fn with_backoff_ms(mut self, backoff_ms: u64) -> Self
pub fn max_retries(&self) -> u32
pub struct TimeoutDecorator
pub fn new<P: LlmProvider + 'static>(inner: P, default_timeout_ms: Option<u64>) -> Self
pub fn with_timeout_ms<P: LlmProvider + 'static>(inner: P, timeout_ms: u64) -> Self
pub fn default_timeout_ms(&self) -> Option<u64>
pub struct TokenBudgetDecorator
pub fn new<P: LlmProvider + 'static>(inner: P, max_input_tokens: Option<usize>) -> Self
pub fn with_limit<P: LlmProvider + 'static>(inner: P, max_input_tokens: usize) -> Self
pub fn max_input_tokens(&self) -> Option<usize>
pub struct LlmTelemetryRecord
pub struct TelemetryDecorator
pub fn new<P: LlmProvider + 'static>(inner: P, sink: Arc<dyn TelemetrySink>) -> Self
pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self
pub fn with_fallback_used(mut self, fallback_used: bool) -> Self
pub fn records(&self) -> Vec<LlmTelemetryRecord>
pub fn last_record(&self) -> Option<LlmTelemetryRecord>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
