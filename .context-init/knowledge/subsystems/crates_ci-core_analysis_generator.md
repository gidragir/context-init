---
directory: crates/ci-core/src/analysis/generator.rs
source:
  kind: git
  blob_sha: b487681c23f6474ba120fb81ed30f8b030b05aeb
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

# Подсистема: Генератор базы знаний (L1)

Рендер L0-индекса и OKF-карточек L1 (render_subsystem_card_okf), generate для записи базы, audit для поиска дрейфа, restamp_card_source для обновления commit/blob SHA во frontmatter.

> **Идентификатор:** `crates_ci-core_analysis_generator`
> **Теги:** drift, generator, l0, l1, okf

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
pub fn update(analysis: &ProjectAnalysis, new_subsystems: &[SubsystemInfo], knowledge_dir: &Path, agents_md: Option<StorageMode>, ) -> Result<(usize, usize)>
pub struct SummaryUpdate
pub fn apply_summaries(analysis: &ProjectAnalysis, knowledge_dir: &Path, updates: &[SummaryUpdate], source: SummarySource, model: Option<&str>, agents_md: Option<StorageMode>, ) -> Result<usize>
pub fn sanitize_id(raw: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
