---
directory: crates/ci-mcp/src/pagination.rs
source:
  kind: git
  blob_sha: 042310663a7bbfd2195ac951707ba3381ad6aa79
  commit_sha: e9daa96fc49222863d0ca3be1c21d8edee9a512b
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

# Подсистема: Ci Mcp Pagination (L1)

Модуль-файл `crates/ci-mcp/src/pagination.rs`

> **Идентификатор:** `crates_ci-mcp_pagination`
> **Теги:** Rust (Cargo), crates_ci-mcp_pagination

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/src/pagination.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
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
