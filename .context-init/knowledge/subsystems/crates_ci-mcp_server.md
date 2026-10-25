# Подсистема: Ci Mcp Server (L1)

> **Идентификатор:** `crates_ci-mcp_server`
> **Теги:** Rust (Cargo), crates_ci-mcp_server

## 🎯 Назначение и роль
Модуль-файл `crates/ci-mcp/src/server.rs`

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
