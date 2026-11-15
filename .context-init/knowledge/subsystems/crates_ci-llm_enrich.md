---
directory: crates/ci-llm/src/enrich.rs
source:
  kind: git
  blob_sha: bbb35e86894994a1d15f3db62f79ed3e7b5043d7
  commit_sha: a37e2f209483ac97bb4729dec70449be893293c4
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
