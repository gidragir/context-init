---
directory: crates/ci-core/src/analysis/generator.rs
source:
  kind: git
  blob_sha: 92faf3de0002206019eb7df8afb8cbefe1d5d2ab
  commit_sha: b040800534c30a22a394125140a07068cb5d9d25
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
pub type CardCache = BTreeMap<String, String>
pub fn render_subsystem_card_okf(sub: &SubsystemInfo, analysis: &ProjectAnalysis, source_shas: Option<(String, String)>, role: &str, model: &str, ) -> String
pub fn restamp_card_source(card_path: &Path, root: &Path, directory: Option<&str>) -> Result<bool>
pub fn generate(analysis: &ProjectAnalysis, subsystems: &[SubsystemInfo], knowledge_dir: &Path, opts: &GenerateOptions, ) -> Result<()>
pub fn audit(analysis: &ProjectAnalysis, knowledge_dir: &Path) -> Result<DriftReport>
pub fn audit_with_cache(analysis: &ProjectAnalysis, knowledge_dir: &Path, cards: &mut CardCache, ) -> Result<DriftReport>
pub struct UpdateStats
pub fn update(analysis: &ProjectAnalysis, new_subsystems: &[SubsystemInfo], knowledge_dir: &Path, agents_md: Option<StorageMode>, ) -> Result<(usize, usize)>
pub fn update_with_options(analysis: &ProjectAnalysis, new_subsystems: &[SubsystemInfo], knowledge_dir: &Path, agents_md: Option<StorageMode>, prune: bool, ) -> Result<UpdateStats>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
