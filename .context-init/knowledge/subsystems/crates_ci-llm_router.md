---
directory: crates/ci-llm/src/router.rs
source:
  kind: git
  blob_sha: 819f1b1d77abedf8a88e2d1d2433f89f80284c95
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

# Подсистема: Ci Llm Router (L1)

Модуль-файл `crates/ci-llm/src/router.rs`

> **Идентификатор:** `crates_ci-llm_router`
> **Теги:** Rust (Cargo), crates_ci-llm_router

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/src/router.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub enum RoleExecutionOutcome
pub fn is_deterministic(&self) -> bool
pub fn is_completed(&self) -> bool
pub fn fallback_used(&self) -> bool
pub fn into_response(self) -> Option<LlmResponse>
pub fn as_response(&self) -> Option<&LlmResponse>
pub struct RoleRouter
pub fn new(config: Config, telemetry: Option<Arc<dyn TelemetrySink>>) -> Result<Self>
pub fn with_registry(config: Config, registry: ProviderRegistry, telemetry: Option<Arc<dyn TelemetrySink>>, ) -> Self
pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self
pub fn register_provider<P: LlmProvider + 'static>(&mut self, alias: impl Into<String>, provider: P, )
pub fn config(&self) -> &Config
pub fn registry(&self) -> &ProviderRegistry
pub fn resolve_chain(&self, role: RoleName, model_override: Option<&str>) -> Vec<String>
pub fn execute(&self, req: &LlmRequest, model_override: Option<&str>, ) -> Result<RoleExecutionOutcome>
pub fn complete_role(&self, req: &LlmRequest, model_override: Option<&str>, ) -> Result<Option<LlmResponse>>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
