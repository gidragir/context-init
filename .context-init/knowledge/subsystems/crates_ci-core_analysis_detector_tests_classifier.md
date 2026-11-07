---
directory: crates/ci-core/src/analysis/detector/tests_classifier.rs
source:
  kind: git
  blob_sha: 47884625561e5b3d757166677683662725dfeea9
  commit_sha: d8acb6e026f3c5cf497ef12860f10754102db968
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

# Подсистема: Ci Core Analysis Detector Tests Classifier (L1)

Модуль-файл `crates/ci-core/src/analysis/detector/tests_classifier.rs`

> **Идентификатор:** `crates_ci-core_analysis_detector_tests_classifier`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_detector_tests_classifier

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/detector/tests_classifier.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub fn is_test_path(rel: &str) -> bool
pub fn test_target(rel: &str) -> Option<&'static str>
pub fn test_group_stem(rel: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
