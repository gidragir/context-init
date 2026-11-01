---
directory: crates/ci-llm/src/providers.rs
source:
  kind: git
  blob_sha: 30b43bb6b2a11e2d8883675d048666e87a570413
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

# Подсистема: LLM-провайдеры (L1)

Провайдеры ollama, command и agent (handoff через context_submit); заглушки openai_compat/anthropic/gemini; парсинг SubsystemAiSummary, healthcheck Ollama, таймаут 60 с по умолчанию.

> **Идентификатор:** `crates_ci-llm_providers`
> **Теги:** agent, command, llm, ollama

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
