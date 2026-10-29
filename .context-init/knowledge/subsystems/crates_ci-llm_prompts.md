---
directory: crates/ci-llm/src/prompts.rs
source:
  kind: git
  blob_sha: 43d585731dfd249c16abfa2e141226a5d8940e8c
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

# Подсистема: Ci Llm Prompts (L1)

Модуль-файл `crates/ci-llm/src/prompts.rs`

> **Идентификатор:** `crates_ci-llm_prompts`
> **Теги:** Rust (Cargo), crates_ci-llm_prompts

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
