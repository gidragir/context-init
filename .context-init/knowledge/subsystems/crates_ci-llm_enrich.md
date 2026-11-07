---
directory: crates/ci-llm/src/enrich.rs
source:
  kind: git
  blob_sha: 66d17dbd2699be57fd4f5e19ba7574a557465c0a
  commit_sha: e9daa96fc49222863d0ca3be1c21d8edee9a512b
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

# Подсистема: Ci Llm Enrich (L1)

Модуль-файл `crates/ci-llm/src/enrich.rs`

> **Идентификатор:** `crates_ci-llm_enrich`
> **Теги:** Rust (Cargo), crates_ci-llm_enrich

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/src/enrich.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub enum EnrichOutcome
pub fn enrich_subsystems(router: &RoleRouter, prompts: &PromptTemplateRegistry, role: RoleName, analysis: &ProjectAnalysis, subsystems: &[SubsystemInfo], model_override: Option<&str>, ) -> EnrichOutcome
pub fn cli_outcome_message(outcome: &EnrichOutcome) -> Option<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
