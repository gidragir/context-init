---
directory: crates/ci-core/src/analysis/pack.rs
source:
  kind: git
  blob_sha: 8fa22435a78a8b4d5a0ec09abe2d108afbcb172a
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
