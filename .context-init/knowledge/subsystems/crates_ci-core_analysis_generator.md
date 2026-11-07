---
directory: crates/ci-core/src/analysis/generator.rs
source:
  kind: git
  blob_sha: b231c057aa6f9daa0356a6e39aed4d7ce72b21d8
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

# Подсистема: Ci Core Analysis Generator (L1)

Модуль-файл `crates/ci-core/src/analysis/generator.rs`

> **Идентификатор:** `crates_ci-core_analysis_generator`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_generator

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/generator.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct GenerateOptions
pub fn render_subsystem_card_okf(sub: &SubsystemInfo, analysis: &ProjectAnalysis, source_shas: Option<(String, String)>, role: &str, model: &str, ) -> String
pub fn restamp_card_source(card_path: &Path, root: &Path, directory: Option<&str>) -> Result<bool>
pub fn generate(analysis: &ProjectAnalysis, subsystems: &[SubsystemInfo], knowledge_dir: &Path, opts: &GenerateOptions, ) -> Result<()>
pub fn audit(analysis: &ProjectAnalysis, knowledge_dir: &Path) -> Result<DriftReport>
pub struct UpdateStats
pub fn update(analysis: &ProjectAnalysis, new_subsystems: &[SubsystemInfo], knowledge_dir: &Path, agents_md: Option<StorageMode>, ) -> Result<(usize, usize)>
pub fn update_with_options(analysis: &ProjectAnalysis, new_subsystems: &[SubsystemInfo], knowledge_dir: &Path, agents_md: Option<StorageMode>, prune: bool, ) -> Result<UpdateStats>
pub struct SummaryUpdate
pub fn apply_summaries(analysis: &ProjectAnalysis, knowledge_dir: &Path, updates: &[SummaryUpdate], source: SummarySource, model: Option<&str>, agents_md: Option<StorageMode>, ) -> Result<usize>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
