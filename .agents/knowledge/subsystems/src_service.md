# Подсистема: Service (L1)

> **Идентификатор:** `src_service`
> **Теги:** src_service, Rust (Cargo) | **Объем:** ~5500 токенов BPE

## 🎯 Назначение и роль
Модуль и компоненты директории `src/service` (1 файлов)

## 📁 Точки входа и структура файлов

| Файл | Строк | Статус |
| :--- | :--- | :--- |
| `src/service.rs` | 694 | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct ServiceInitOptions<'a>
pub struct ServiceUpdateOptions<'a>
pub struct ContextService
pub fn build_subsystems(analysis: &ProjectAnalysis, ai: bool, ollama_url: &str, model: &str, _compact: bool, ) -> Result<Vec<SubsystemInfo>>
pub fn pack_builtin_to_string(root: &Path, subs: &[&SubsystemInfo], files: &[String], compress: bool, style: &str, ) -> String
pub fn compress_file_content(content: &str, ext: &str) -> String
pub fn compress_rust_ast(content: &str) -> Option<String>
pub fn normalize_rust_tokens(s: &str) -> String
pub fn compress_code(content: &str) -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test
```
