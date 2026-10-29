---
directory: crates/ci-llm/src/providers.rs
source:
  kind: git
  blob_sha: c6b88401c4a5d569f7eed2a59344270d1ac76f7f
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

# Подсистема: Ci Llm Providers (L1)

Модуль-файл `crates/ci-llm/src/providers.rs`

> **Идентификатор:** `crates_ci-llm_providers`
> **Теги:** Rust (Cargo), crates_ci-llm_providers

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/src/providers.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const DEFAULT_PROVIDER_TIMEOUT_MS: u64 = 60_000
pub const DEFAULT_HEALTHCHECK_TIMEOUT_MS: u64 = 1_500
pub struct SubsystemAiSummary
pub fn parse_subsystem_summaries(raw: &str) -> Result<Vec<SubsystemAiSummary>>
pub fn is_ollama_online(endpoint: &str, timeout_ms: Option<u64>) -> bool
pub struct OllamaProvider
pub struct CommandProvider
pub struct AgentProvider
pub fn validate_agent_submission(package: &AgentWorkPackage, submission: &Value, ) -> Result<ValidatedAgentOutput>
pub struct ExtensionPointError
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
