# Подсистема: Ci Core Analysis Detector Tests (L1)

> **Идентификатор:** `crates_ci-core_analysis_detector_tests`
> **Теги:** Rust (Cargo), crates_ci-core_analysis_detector_tests

## 🎯 Назначение и роль
Модуль-файл `crates/ci-core/src/analysis/detector/tests.rs`

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
