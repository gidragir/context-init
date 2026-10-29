---
directory: crates/ci-core/src/analysis/detector/tests.rs
source:
  kind: git
  blob_sha: 0a0cf5184e174338b64fc9082880c9c3cea39a9a
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

# Подсистема: Ci Core Analysis Detector Tests (L1)

Модуль-файл `crates/ci-core/src/analysis/detector/tests.rs`

> **Идентификатор:** `crates_ci-core_analysis_detector_tests`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_detector_tests

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-core/src/analysis/detector/tests.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct Engine<T: Clone>
pub enum Status
pub async fn run_task<T>(id: &str, payload: T) -> Result<Status, Error>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-core
```
