---
directory: crates/ci-core/src/openviking.rs
source:
  kind: git
  blob_sha: d0748321868e0379c157555376f25c17ac24b0f7
  commit_sha: 14efb361ef43dd6f73ad66906402e36beb29aa4a
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
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
