---
directory: docs/superpowers
source:
  kind: git
  blob_sha: 1b4e073a2e58603263154c873a1af87482eec0a6
  commit_sha: dc60f5d01420811e5956062f2bb1460d782688ad
generated_by:
  component: context-init
  role: update
  model: none
freshness:
  total_entries: 9
  sampled_entries: 9
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Docs (L1)

Модуль и компоненты директории `docs` (9 файлов)

> **Идентификатор:** `docs`
> **Теги:** Rust (Cargo), docs

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `docs/superpowers/plans/2026-10-23-v0.3-step5-cli.md` | Исходный код |
| `docs/superpowers/plans/2026-10-24-repo-storage.md` | Исходный код |
| `docs/superpowers/plans/2026-10-28-complete-design-v0.3-v0.6.md` | Исходный код |
| `docs/superpowers/plans/2026-10-29-context-init-adoption-and-integrity.md` | Исходный код |
| `docs/superpowers/plans/2026-11-01-data-projects-production.md` | Исходный код |
| `docs/superpowers/specs/2026-10-23-v0.3-step5-cli-design.md` | Исходный код |
| `docs/superpowers/specs/2026-10-24-repo-storage-design.md` | Исходный код |
| `docs/superpowers/specs/2026-10-29-context-init-adoption-and-integrity-design.md` | Исходный код |
| `docs/superpowers/specs/2026-11-01-data-projects-production-design.md` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub enum EnrichOutcome
pub fn enrich_subsystems(...) -> EnrichOutcome ``` It never returns `Err`, because enrichment must not break the pipeline. The facts are built from `analysis` the same way `src/ollama.rs:36-75` does. LLM content parsed by `parse_subsystem_summaries` is merged only into ids tha...
pub struct FullDriftReport
pub has_drift: bool, pub missing_in_fs: Vec<String>, pub untracked_in_knowledge: Vec<String>, pub up_to_date_count: usize, pub git_stale: bool, pub stale_subsystems: Vec<String>, } pub fn run_check(project: &ResolvedProject) -> Result<FullDriftReport>
pub fn validate_model_override(config: &ci_core::config::Config, model_override: Option<&str>) -> Result<()>
pub fn atomic_write_str(path: &Path, content: &str) -> Result<()>
pub fn create_secure_temp_markdown(content: &str) -> Result<std::path::PathBuf>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test
```
