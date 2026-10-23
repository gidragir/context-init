# Инструкции для ИИ-агентов в проекте context-init

Данный проект настроен для автономной работы ассистентов.

## 🗺 Архитектурная карта и навигация (OpenViking L0-L2)

Проект `context-init` описан трехуровневой базой знаний context-init (L0-L2), которая хранится вне репозитория. Для навигации используйте её вместо чтения исходников целиком.

### ⚡ MCP-First Protocol (context-init)

1. `context_get_subsystems` — карта подсистем проекта (L0).
2. `context_get_subsystem(subsystem_id)` — компактная L1-карточка подсистемы.
3. `context_pack(subsystem_id, compress=true)` — AST-сжатый код подсистемы.
4. `context_check` — проверка архитектурного дрейфа перед сдачей задачи.

Без доступа к MCP (субагенты, скрипты): `context-init pack <subsystem_id> --compress` и `context-init check --compact`.

### 📦 Доступные подсистемы:
- `crates_ci-core` (Ci Core) — Подсистема `crates/ci-core` с ключевыми точками входа: crates/ci-core/src/lib.rs, crates/ci-core/src/analysis/mod.rs, crates/ci-core/Cargo.toml
- `crates_ci-llm` (Ci Llm) — Подсистема `crates/ci-llm` с ключевыми точками входа: crates/ci-llm/src/lib.rs, crates/ci-llm/Cargo.toml
- `crates_ci-mcp` (Ci Mcp) — Подсистема `crates/ci-mcp` с ключевыми точками входа: crates/ci-mcp/src/lib.rs, crates/ci-mcp/Cargo.toml
- `references` (References) — Модуль и компоненты директории `references` (1 файлов)
- `reports` (Reports) — Модуль и компоненты директории `reports` (11 файлов)
- `src_cli` (Cli) — Подсистема `src/cli` с ключевыми точками входа: src/cli/mod.rs, src/cli/setup/mod.rs
- `tests` (Tests) — Модуль и компоненты директории `tests` (2 файлов)
