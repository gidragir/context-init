---
directory: crates/ci-llm/src/router.rs
source:
  kind: git
  blob_sha: 819f1b1d77abedf8a88e2d1d2433f89f80284c95
  commit_sha: 2ea8f3affd3c7e26633575afd792de4875d13585
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

# Подсистема: Маршрутизатор ролей (L1)

RoleRouter исполняет цепочку моделей для роли с фолбэком; RoleExecutionOutcome различает детерминированный, завершённый и фолбэк-исход.

> **Идентификатор:** `crates_ci-llm_router`
> **Теги:** fallback, llm, router

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
