---
directory: crates/ci-core/src/analysis/pack.rs
source:
  kind: git
  blob_sha: 887615585a7d09a2cbc087cab2df8e9938a4e779
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
pub struct PackedSubsystems
pub fn pack_subsystem(root: &Path, knowledge_dir: &Path, index: &L0Index, subsystem_id: &str, compress: bool, style: PackStyle, ) -> Result<PackedSubsystems>
pub fn pack_files(root: &Path, subs: &[&SubsystemInfo], files: &[String], compress: bool, style: PackStyle, ) -> String
pub fn compress_source(content: &str, ext: &str) -> String
pub fn compress_rust_ast(content: &str) -> Option<String>
pub fn format_rust_items(items: &[syn :: Item]) -> String
pub fn normalize_rust_tokens(s: &str) -> String
pub fn compress_code(content: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
