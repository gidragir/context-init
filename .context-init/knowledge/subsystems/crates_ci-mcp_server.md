---
directory: crates/ci-mcp/src/server.rs
source:
  kind: git
  blob_sha: 03e89a3a0c5922563c6e8324a9d15f8e5a2109fe
  commit_sha: 14efb361ef43dd6f73ad66906402e36beb29aa4a
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

# Подсистема: Ci Mcp Server (L1)

Модуль-файл `crates/ci-mcp/src/server.rs`

> **Идентификатор:** `crates_ci-mcp_server`
> **Теги:** Rust (Cargo), crates_ci-mcp_server

## 📁 Точки входа и структура файлов

| Файл | Статус |
| :--- | :--- |
| `crates/ci-mcp/src/server.rs` | Исходный код |

## 🔑 Ключевой контракт и экспорты
```text
pub struct ContextInitServer
pub fn find_project_root_upward(start: &Path) -> Option<PathBuf>
pub fn roots_to_paths(roots: &[Root]) -> Vec<PathBuf>
pub async fn run_mcp_server(default_root: Option<&Path>) -> Result<()>
```

## 🧪 Запуск и тестирование
```bash
rtk cargo test -p ci-mcp
```
