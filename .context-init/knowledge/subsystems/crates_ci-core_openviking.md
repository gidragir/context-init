---
directory: crates/ci-core/src/openviking.rs
source:
  kind: git
  blob_sha: 0cc4de7ba10b4779cc82728037a00fae9d7e1577
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

# Подсистема: Ci Core Openviking (L1)

Модуль-файл `crates/ci-core/src/openviking.rs`

> **Идентификатор:** `crates_ci-core_openviking`
> **Теги:** Rust (Cargo), crates_ci-core_openviking

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/openviking.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub const L0_ABSTRACT_MAX_CHARS: usize = 256
pub struct CardSource
pub struct CardGeneratedBy
pub struct CardFreshness
pub struct CardFrontmatter
pub struct OkfCard
pub fn extract_l0_abstract(markdown: &str) -> String
pub fn extract_l0_abstract_with_limit(markdown: &str, max_chars: usize) -> String
pub fn truncate_to_boundary(text: &str, max_chars: usize) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
