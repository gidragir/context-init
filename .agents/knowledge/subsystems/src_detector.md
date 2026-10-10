# Подсистема: Detector (L1)

> **Идентификатор:** `src_detector`
> **Теги:** src_detector, Rust (Cargo) | **Объем:** ~13075 токенов BPE

## 🎯 Назначение и роль
Модуль и компоненты директории `src/detector` (1 файлов)

## 📁 Точки входа и структура файлов

| Файл | Строк | Статус |
| :--- | :--- | :--- |
| `src/detector.rs` | 1702 | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub fn analyze_project(root: &Path, deep: bool, with_rtk: bool) -> Result<ProjectAnalysis>
pub fn is_rtk_available() -> bool
pub fn prefix_with_rtk(cmd: &str) -> String
pub fn count_tokens(text: &str) -> usize
pub fn extract_rust_symbols(content: &str, deep: bool) -> Vec<String>
pub fn extract_multiline_symbols(content: &str, ext: &str, deep: bool) -> Vec<String>
pub fn clean_symbol_line(line: &str) -> String
pub fn sanitize_symbol(raw: &str) -> Option<String>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test
```
