# Подсистема: Ci Mcp Pagination (L1)

> **Идентификатор:** `crates_ci-mcp_pagination`
> **Теги:** Rust (Cargo), crates_ci-mcp_pagination

## 🎯 Назначение и роль
Модуль-файл `crates/ci-mcp/src/pagination.rs`

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/src/pagination.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub fn count_tokens(text: &str) -> usize
pub struct CursorData
pub fn base64_url_encode(data: &[u8]) -> String
pub fn base64_url_decode(input: &str) -> Option<Vec<u8>>
pub fn encode_cursor(cursor: &CursorData) -> String
pub fn decode_cursor(cursor_str: &str) -> Option<CursorData>
pub fn format_footer(truncated: bool, next_cursor: Option<&str>, stale: bool, tokens: usize, ) -> String
pub struct PaginatedOutput
pub fn paginate_content(full_text: &str, cursor: Option<&str>, max_tokens: usize, stale: bool, ) -> PaginatedOutput
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
