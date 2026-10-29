---
directory: crates/ci-core/src/analysis/generator.rs
source:
  kind: git
  blob_sha: 7236433c889b4fb3efe000c6714c4aa2eb4abdf2
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
pub fn update(analysis: &ProjectAnalysis, new_subsystems: &[SubsystemInfo], knowledge_dir: &Path, agents_md: Option<StorageMode>, ) -> Result<(usize, usize)>
pub fn sanitize_id(raw: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
