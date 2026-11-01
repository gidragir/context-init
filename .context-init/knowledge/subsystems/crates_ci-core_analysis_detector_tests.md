---
directory: crates/ci-core/src/analysis/detector/tests.rs
source:
  kind: git
  blob_sha: c9c08e468aff90ba1f1f37d836f21f0727379ff1
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

# Подсистема: Тесты детектора (L1)

Юнит-тесты детектора; символы Engine, Status, MyService — фикстурный Rust-код для проверки извлечения сигнатур, а не API проекта.

> **Идентификатор:** `crates_ci-core_analysis_detector_tests`
> **Теги:** fixtures, tests

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/detector/tests.rs` | Исходный код |

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
