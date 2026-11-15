# Промпт исполнителя: улучшения упаковки `context-init` и ускорение синхронизации

## Целевые скиллы
- `superpowers:subagent-driven-development` (или `superpowers:executing-plans` при последовательном выполнении в одной сессии)
- `superpowers:test-driven-development` (строго Red → Green → Refactor по шагам плана)
- `superpowers:verification-before-completion`
- `context-init`

## Артефакты задачи
- **Спецификация:** `docs/superpowers/specs/2026-11-15-context-init-pack-and-sync-improvements-design.md`
- **План реализации:** `docs/superpowers/plans/2026-11-15-context-init-pack-and-sync-improvements.md`

## Обязательные директивы навигации и среды
1. Для исследования архитектуры и API используй `rtk context-init pack <subsystem_id> --compress` (или MCP `context_pack` с явным `"path": "/home/gidragir/projects/context-init"` и `"style": "markdown"`). Не вычитывай файлы директории целиком.
2. Запрещено читать `.context-init/knowledge/**` напрямую и использовать GNU `find` / `grep` (только `rtk fd` и `rtk rg` без `-E`). Все shell-команды запускай с префиксом `rtk`.
3. Соблюдай все инварианты из секции `Global Constraints` плана:
   - Не ломать `l0_omits_volatile_fields` (`L0_index.json` v4 без `tokens`/`lines`/`root_path`).
   - Кэш метрик писать строго в `StoragePaths::cache_dir` (вне Git-дерева), используя `sha2::Sha256`.
   - Сохранять legacy `compress_source(content, ext)` для совместимости unit-тестов.
   - Удерживать `rule_template.md <= 25` строк, `AGENTS.md <= 22 000` байт, завершающий `\n` на всех JSON/MD.

## Границы модулей по волнам
- **Волна 1 (Tasks 1–3) — Контракт упаковки, фильтрация и пагинация:**
  - `crates_ci-core_analysis_pack` (`crates/ci-core/src/analysis/pack.rs`)
  - `crates_ci-mcp_tools`, `crates_ci-mcp_handlers`, `crates_ci-mcp_pagination`, `crates_ci-mcp_server`
  - `src_cli` (`src/cli/pack.rs`, `src/cli/mod.rs`), `tests/cli.rs`, `crates/ci-mcp/src/lib.rs`
- **Волна 2 (Tasks 4–5) — Адаптеры языков, L1.5 TOC, бюджет CLI и инструкции:**
  - `crates_ci-core_analysis_lang` (`crates/ci-core/src/analysis/lang/{mod,rust,kotlin,brace,python,bsl}.rs`)
  - `src_cli` (`src/cli/pack.rs`), `src_cli_setup` (`src/cli/setup/{rule_template.md,rules.rs}`)
  - `crates_ci-core_analysis_generator` (`render_root_agents_md`), `.agents/skills/context-init/SKILL.md`
- **Волна 3 (Tasks 6–8) — Скорость синхронизации:**
  - `crates_ci-core_git` (`crates/ci-core/src/git.rs`)
  - `crates_ci-core_analysis_generator`, `crates_ci-core_analysis_scan`, `crates_ci-core_analysis` (`scope.rs`, `detector.rs`, `subsystems.rs`, новый `cache.rs`)
  - `crates_ci-mcp_pipeline` (`crates/ci-mcp/src/pipeline.rs`), `crates_ci-mcp_handlers`

## Команды верификации
По завершении каждой задачи и всей реализации выполнить в точном порядке:
```bash
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace
rtk cargo run -- update . --prune --compact
rtk cargo run -- check . --compact --exit-code
```
Коммиты выполнять после зелёных проверок через `rtk oco -y`.
