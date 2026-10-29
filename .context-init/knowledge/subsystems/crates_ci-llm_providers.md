---
directory: crates/ci-llm/src/providers.rs
source:
  kind: git
  blob_sha: c6b88401c4a5d569f7eed2a59344270d1ac76f7f
  commit_sha: 4f153c6ee310b681808626eb7348ac3ca484e21d
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
pub fn new(alias: impl Into<String>, endpoint: impl Into<String>, model: impl Into<String>, ) -> Self
pub fn from_config(alias: impl Into<String>, cfg: &ModelConfig) -> Self
pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self
pub fn with_max_input_tokens(mut self, max_input_tokens: usize) -> Self
pub fn endpoint(&self) -> Option<&str>
pub fn model(&self) -> Option<&str>
pub fn is_online(&self) -> bool
pub fn fetch_tags(&self) -> Result<Vec<String>>
pub struct CommandProvider
pub fn new(alias: impl Into<String>, command: Vec<String>) -> Result<Self>
pub fn from_config(alias: impl Into<String>, cfg: &ModelConfig) -> Result<Self>
pub fn with_model(mut self, model: impl Into<String>) -> Self
pub fn command(&self) -> &[String]
pub struct AgentProvider
pub fn new(alias: impl Into<String>) -> Self
pub fn build_work_package(&self, req: &LlmRequest) -> AgentWorkPackage
pub fn validate_agent_submission(package: &AgentWorkPackage, submission: &Value, ) -> Result<ValidatedAgentOutput>
pub struct ExtensionPointError
pub struct OpenAiCompatProvider
pub struct AnthropicProvider
pub struct GeminiProvider
pub struct SamplingProvider
pub struct ProviderFactory
pub fn create_raw(alias: &str, cfg: &ModelConfig) -> Result<Arc<dyn LlmProvider>>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
