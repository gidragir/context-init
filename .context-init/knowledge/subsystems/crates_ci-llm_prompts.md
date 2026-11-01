---
directory: crates/ci-llm/src/prompts.rs
source:
  kind: git
  blob_sha: fe0aaca9b58acc61f15014dde5b419340de950f7
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
pub struct RenderedPrompt
pub struct PromptTemplateRegistry
pub fn language_instruction(language: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
