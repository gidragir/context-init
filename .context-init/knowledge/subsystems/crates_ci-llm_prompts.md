# Подсистема: Ci Llm Prompts (L1)

> **Идентификатор:** `crates_ci-llm_prompts`
> **Теги:** Rust (Cargo), crates_ci-llm_prompts

## 🎯 Назначение и роль
Модуль-файл `crates/ci-llm/src/prompts.rs`

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
