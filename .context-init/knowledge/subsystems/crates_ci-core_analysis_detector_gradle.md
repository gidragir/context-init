---
directory: crates/ci-core/src/analysis/detector/gradle.rs
source:
  kind: git
  blob_sha: 5ddb15648602923f5551e42993158819f0c8e062
  commit_sha: 05b17436dd0087469fb8bef6c61f16e554831faa
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

# Подсистема: Ci Core Analysis Detector Gradle (L1)

Модуль-файл `crates/ci-core/src/analysis/detector/gradle.rs`

> **Идентификатор:** `crates_ci-core_analysis_detector_gradle`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_detector_gradle

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/detector/gradle.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub fn parse_gradle_workspace(root: &Path) -> (Option<String>, Vec<String>)
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
