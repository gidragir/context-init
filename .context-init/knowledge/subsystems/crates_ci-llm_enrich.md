---
directory: crates/ci-llm/src/enrich.rs
source:
  kind: git
  blob_sha: 66d17dbd2699be57fd4f5e19ba7574a557465c0a
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

# Подсистема: Обогащение подсистем (L1)

enrich_subsystems: прогон роли через маршрутизатор и слияние сводок/тегов по id; EnrichOutcome (Skipped, Enriched, PendingAgent, Failed) никогда не ломает конвейер.

> **Идентификатор:** `crates_ci-llm_enrich`
> **Теги:** enrich, llm, pipeline

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
