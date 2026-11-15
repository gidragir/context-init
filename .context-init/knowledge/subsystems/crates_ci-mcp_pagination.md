---
directory: crates/ci-mcp/src/pagination.rs
source:
  kind: git
  blob_sha: 910bfdb701c2f488146c0de6bb0dd61aff0a88d8
  commit_sha: 8edfd693b819f9b4b54f664fed4f61310c0300c2
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
pub struct PaginationMeta<'a>
pub fn compute_query_hash(parts: &[&str], full_text: &str) -> u64
pub fn extract_section_names(text: &str) -> Vec<String>
pub fn base64_url_encode(data: &[u8]) -> String
pub fn base64_url_decode(input: &str) -> Option<Vec<u8>>
pub fn encode_cursor(cursor: &CursorData) -> String
pub fn decode_cursor(cursor_str: &str) -> Option<CursorData>
pub fn format_footer(truncated: bool, next_cursor: Option<&str>, stale: bool, tokens: usize, ) -> String
pub struct FooterExtras<'a>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
