---
directory: crates/ci-llm/src/router.rs
source:
  kind: git
  blob_sha: 819f1b1d77abedf8a88e2d1d2433f89f80284c95
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

# Подсистема: Ci Llm Router (L1)

Модуль-файл `crates/ci-llm/src/router.rs`

> **Идентификатор:** `crates_ci-llm_router`
> **Теги:** Rust (Cargo), crates_ci-llm_router

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-llm/src/router.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub enum RoleExecutionOutcome
pub struct RoleRouter
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-llm
```
