---
directory: crates/ci-core/src/analysis/pack.rs
source:
  kind: git
  blob_sha: 12d94be2d111034f0181d038423aa03a280be0b5
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

# Подсистема: Упаковщик контекста (L1)

pack_subsystem / pack_files: сборка файлов подсистемы в XML или Markdown с AST-сжатием (тела функций вырезаются) для передачи в LLM.

> **Идентификатор:** `crates_ci-core_analysis_pack`
> **Теги:** compression, context, pack

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/pack.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub enum PackStyle
pub fn parse(style: &str) -> Self
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
