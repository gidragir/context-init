---
directory: crates/ci-mcp/src/server.rs
source:
  kind: git
  blob_sha: 07d07557d8dc894b07d7901014d0ee771541d5ee
  commit_sha: 9660354a570d6203128acd3df79cd62f23660f76
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

# Подсистема: MCP-сервер (L1)

ContextInitServer на stdio: регистрация инструментов, MCP roots (roots/list) для выбора проекта; TelemetrySink подключается через with_telemetry_sink (в продакшен-запуске run_mcp_server не подключён).

> **Идентификатор:** `crates_ci-mcp_server`
> **Теги:** mcp, roots, server, telemetry

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
