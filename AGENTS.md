# Инструкции для ИИ-агентов в проекте context-init

Данный проект настроен для автономной работы ассистентов.

## 🗺 Архитектурная карта и навигация (OpenViking L0-L2)

Проект `context-init` использует трехуровневую базу знаний в `.agents/knowledge/`:

- **L0 Индекс (Mental Map):** [`.agents/knowledge/L0_index.md`](.agents/knowledge/L0_index.md) | [JSON](.agents/knowledge/L0_index.json)
- **Справочник команд (L2):** [`.agents/knowledge/cli/commands.md`](.agents/knowledge/cli/commands.md)
- **Архитектурные решения (ADR):** [`.agents/knowledge/adr/0001_initial_architecture.md`](.agents/knowledge/adr/0001_initial_architecture.md)

### Подсистемы (L1):
- [Main](.agents/knowledge/subsystems/src_core.md) — Подсистема `src/core` с ключевыми точками входа: src/main.rs
- [Detector](.agents/knowledge/subsystems/src_detector.md) — Модуль и компоненты директории `src/detector` (1 файлов)
- [Generator](.agents/knowledge/subsystems/src_generator.md) — Модуль и компоненты директории `src/generator` (1 файлов)
- [Ollama](.agents/knowledge/subsystems/src_ollama.md) — Модуль и компоненты директории `src/ollama` (1 файлов)
- [Types](.agents/knowledge/subsystems/src_types.md) — Модуль и компоненты директории `src/types` (1 файлов)
- [Mcp](.agents/knowledge/subsystems/src_mcp.md) — Модуль и компоненты директории `src/mcp` (1 файлов)
- [Service](.agents/knowledge/subsystems/src_service.md) — Модуль и компоненты директории `src/service` (1 файлов)
- [Reports](.agents/knowledge/subsystems/reports.md) — Модуль и компоненты директории `reports` (2 файлов)
- [Tests](.agents/knowledge/subsystems/tests.md) — Модуль и компоненты директории `tests` (1 файлов)
