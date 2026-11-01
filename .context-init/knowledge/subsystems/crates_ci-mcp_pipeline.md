---
directory: crates/ci-mcp/src/pipeline.rs
source:
  kind: git
  blob_sha: 9d5f2db3ba5c2e627655eab5ea7b0e9fd5ed7ce0
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
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

# Подсистема: Конвейер init/update (L1)

run_init / run_update: анализ, обогащение ролями, генерация базы и AGENTS.md; FullDriftReport объединяет файловый и git-дрейф.

> **Идентификатор:** `crates_ci-mcp_pipeline`
> **Теги:** drift, init, pipeline, update

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/src/pipeline.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct PipelineOptions<'a>
pub struct PipelineOutcome
pub fn run_init(project: &ResolvedProject, router: &RoleRouter, opts: &PipelineOptions, ) -> Result<PipelineOutcome>
pub fn run_update(project: &ResolvedProject, router: &RoleRouter, opts: &PipelineOptions, ) -> Result<PipelineOutcome>
pub struct FullDriftReport
pub fn run_check(project: &ResolvedProject) -> Result<FullDriftReport>
pub fn validate_model_override(config: &Config, model_override: Option<&str>) -> Result<()>
pub fn refresh_stale_card(project: &ResolvedProject, sub: &SubsystemInfo) -> Result<()>
pub fn storage_mismatch_hint(project: &ResolvedProject) -> Option<String>
pub fn subsystems_needing_enrichment(existing: &L0Index, fresh: &[SubsystemInfo]) -> Vec<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
