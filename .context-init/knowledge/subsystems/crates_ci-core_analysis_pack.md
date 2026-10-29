---
directory: crates/ci-core/src/analysis/pack.rs
source:
  kind: git
  blob_sha: 58581e25cc2dfe9a8577924ae5899af9b34914fd
  commit_sha: 36198169b6c3e1dac832cf4204e7b96c6805df94
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
