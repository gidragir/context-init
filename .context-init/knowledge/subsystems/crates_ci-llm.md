---
directory: crates/ci-llm
source:
  kind: git
  blob_sha: 42d4313477923057a8b3614082c8fad2aacc4adb
  commit_sha: fff7dd106863757218c552cb8ef5b18e46c70899
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 3
  sampled_entries: 3
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: ci-llm (L1)

Слой LLM: трейт провайдера, RoleName, AgentWorkPackage и ValidatedAgentOutput для передачи работы агенту-харнессу через MCP.

> **Идентификатор:** `crates_ci-llm`
> **Теги:** agent-handoff, llm, provider

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/Cargo.toml` | ⭐ Точка входа |
| `crates/ci-llm/src/lib.rs` | ⭐ Точка входа |
| `crates/ci-llm/src/provider.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const VERSION: &str = env !("CARGO_PKG_VERSION")
pub fn crate_info() -> String
pub type RoleName = RoleKind
pub struct AgentWorkPackage
pub struct ValidatedAgentOutput
pub struct LlmRequest
pub fn new(role: RoleName, prompt: impl Into<String>) -> Self
pub fn with_system_prompt(mut self, system_prompt: impl Into<String>) -> Self
pub fn with_json_schema(mut self, json_schema: serde_json :: Value) -> Self
pub fn with_max_input_tokens(mut self, max_input_tokens: usize) -> Self
pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self
pub fn with_facts(mut self, facts: serde_json :: Value) -> Self
pub struct LlmResponse
pub struct ProviderCapabilities
pub trait LlmProvider
fn complete(&self, req: &LlmRequest) -> Result<LlmResponse>
fn capabilities(&self) -> ProviderCapabilities
fn kind(&self) -> ModelKind
fn alias(&self) -> &str
pub struct LlmExecutionError
pub fn inspect_execution_error(err: &anyhow :: Error) -> Option<&LlmExecutionError>
pub fn estimate_tokens(text: &str) -> usize
pub fn estimate_request_tokens(req: &LlmRequest) -> usize
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
