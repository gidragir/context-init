# Подсистема: Ci Llm Enrich (L1)

> **Идентификатор:** `crates_ci-llm_enrich`
> **Теги:** Rust (Cargo), crates_ci-llm_enrich

## 🎯 Назначение и роль
Модуль-файл `crates/ci-llm/src/enrich.rs`

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/src/enrich.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub enum EnrichOutcome
pub fn enrich_subsystems(router: &RoleRouter, prompts: &PromptTemplateRegistry, role: RoleName, analysis: &ProjectAnalysis, subsystems: &mut [SubsystemInfo], model_override: Option<&str>, ) -> EnrichOutcome
pub fn cli_outcome_message(outcome: &EnrichOutcome) -> Option<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
