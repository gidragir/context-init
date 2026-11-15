---
directory: crates/ci-core/src/analysis/pack.rs
source:
  kind: git
  blob_sha: f809f245c56ada00bf27c0ea4a1f827e78972427
  commit_sha: 8edfd693b819f9b4b54f664fed4f61310c0300c2
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

# Подсистема: Ci Core Analysis Pack (L1)

Модуль-файл `crates/ci-core/src/analysis/pack.rs`

> **Идентификатор:** `crates_ci-core_analysis_pack`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_pack

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/pack.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub enum PackStyle
pub enum PackMode
pub struct CompressOptions
pub struct PackRequest<'a>
pub fn compress_source_with(content: &str, ext: &str, opts: &CompressOptions) -> String
pub struct PackedSubsystems
pub fn pack_subsystem(root: &Path, knowledge_dir: &Path, index: &L0Index, subsystem_id: &str, compress: bool, style: PackStyle, ) -> Result<PackedSubsystems>
pub fn validate_file_patterns(patterns: &[String]) -> Result<GlobSet>
pub fn pack_subsystem_with(root: &Path, knowledge_dir: &Path, index: &L0Index, req: &PackRequest<'_>, ) -> Result<PackedSubsystems>
pub fn pack_files(root: &Path, subs: &[&SubsystemInfo], files: &[String], compress: bool, style: PackStyle, ) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
