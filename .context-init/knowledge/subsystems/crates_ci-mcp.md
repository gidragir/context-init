---
directory: crates/ci-mcp
source:
  kind: git
  blob_sha: d60742eed738b9b35796027aa828fec30ac6391b
  commit_sha: ebecc37e77cdf411f5310fea0fda7166b9e27a50
generated_by:
  component: context-init
  role: init
  model: none
freshness:
  total_entries: 2
  sampled_entries: 2
  unsampled_entries: 0
  pending_child_changes: 0
---

# Подсистема: Ci Mcp (L1)

Подсистема `crates/ci-mcp` с ключевыми точками входа: crates/ci-mcp/Cargo.toml, crates/ci-mcp/src/lib.rs

> **Идентификатор:** `crates_ci-mcp`
> **Теги:** Rust (Cargo), crates_ci-mcp

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/Cargo.toml` | ⭐ Точка входа |
| `crates/ci-mcp/src/lib.rs` | ⭐ Точка входа |

## 🔑 Ключевой контракт и экспорты
```text
pub const VERSION: &str = env !("CARGO_PKG_VERSION")
pub fn crate_info() -> String
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
