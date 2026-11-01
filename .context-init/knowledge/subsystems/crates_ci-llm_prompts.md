---
directory: crates/ci-llm/src/prompts.rs
source:
  kind: git
  blob_sha: fe0aaca9b58acc61f15014dde5b419340de950f7
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

# Подсистема: Шаблоны промптов (L1)

Реестр PromptTemplate с источниками (встроенные/пользовательские) и PromptVars (факты, проект, язык) для ролей init/update/task/search.

> **Идентификатор:** `crates_ci-llm_prompts`
> **Теги:** prompts, templates

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/src/prompts.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub enum PromptTemplateSource
pub struct PromptTemplate
pub struct PromptVars
pub fn with_facts(facts: impl Into<String>) -> Self
pub fn project(mut self, project: impl Into<String>) -> Self
pub fn query(mut self, query: impl Into<String>) -> Self
pub fn json_schema(mut self, json_schema: impl Into<String>) -> Self
pub fn structured_facts(mut self, facts: Value) -> Self
pub struct RenderedPrompt
pub struct PromptTemplateRegistry
pub fn new(language: impl Into<String>) -> Self
pub fn from_config(config: &Config, config_dir: Option<&Path>, repo_root: Option<&Path>, ) -> Result<Self>
pub fn load(language: impl Into<String>, config_dir: Option<&Path>, repo_root: Option<&Path>, ) -> Result<Self>
pub fn language(&self) -> &str
pub fn set_language(&mut self, language: impl Into<String>)
pub fn get(&self, role: RoleName) -> &PromptTemplate
pub fn set(&mut self, role: RoleName, template: PromptTemplate)
pub fn load_global_overrides(&mut self, config_dir: &Path) -> Result<()>
pub fn load_repo_overrides(&mut self, repo_root: &Path) -> Result<()>
pub fn render(&self, role: RoleName, vars: &PromptVars) -> RenderedPrompt
pub fn render_facts(&self, role: RoleName, facts: &str) -> RenderedPrompt
pub fn to_llm_request(&self, role: RoleName, vars: &PromptVars) -> LlmRequest
pub fn language_instruction(language: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
