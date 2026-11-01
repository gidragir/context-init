---
directory: crates/ci-core/src/analysis/detector/tests.rs
source:
  kind: git
  blob_sha: a03cdecd74751f9ffdba0cea6a841c6233674613
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
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
