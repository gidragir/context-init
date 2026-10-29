---
directory: crates/ci-mcp/src/server.rs
source:
  kind: git
  blob_sha: 6bf6ccd7e20a1a9b83b76c2d581bf6ff1976e7d6
  commit_sha: 9048360bdc3d59b7d897b5cfe7eea7df1cd818a8
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
