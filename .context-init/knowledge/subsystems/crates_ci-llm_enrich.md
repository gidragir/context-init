---
directory: crates/ci-llm/src/enrich.rs
source:
  kind: git
  blob_sha: 729e060503a71b3f84b3d928ca358d5ff3784c29
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
pub fn enrich_subsystems(router: &RoleRouter, prompts: &PromptTemplateRegistry, role: RoleName, analysis: &ProjectAnalysis, subsystems: &mut [SubsystemInfo], model_override: Option<&str>, ) -> EnrichOutcome
pub fn cli_outcome_message(outcome: &EnrichOutcome) -> Option<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
