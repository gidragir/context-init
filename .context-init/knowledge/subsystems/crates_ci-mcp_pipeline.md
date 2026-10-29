---
directory: crates/ci-mcp/src/pipeline.rs
source:
  kind: git
  blob_sha: 830493844001bbb8dc674c0399ad8a321a7c1699
  commit_sha: 9048360bdc3d59b7d897b5cfe7eea7df1cd818a8
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

# Подсистема: Ci Mcp Pipeline (L1)

Модуль-файл `crates/ci-mcp/src/pipeline.rs`

> **Идентификатор:** `crates_ci-mcp_pipeline`
> **Теги:** Rust (Cargo), crates_ci-mcp_pipeline

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
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
