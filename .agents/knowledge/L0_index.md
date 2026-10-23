# 🗺 Карта архитектуры: context-init (L0)

> **Стек:** Rust (Cargo) | **Обновлено:** 2026-10-10
> **Описание:** CLI-утилита на Rust для автоматической инициализации и поддержания трехуровневой базы знаний (**OpenViking L0-L2**) для автономных ИИ-ассистентов (Antigravity, Hermes, Claude Code, Cursor, Aider).

## 🚪 Точки входа (Root Entrypoints)
- `src/main.rs`
- `src/lib.rs`
- `Cargo.toml`

## ⚡️ Быстрые команды
```bash
# Сборка
rtk cargo build

# Тесты
rtk cargo test

# Линтер
rtk cargo clippy

# Запуск
rtk cargo run

```

## 🧩 Каталог подсистем (L1)

| Подсистема | Роль / Назначение | Точки входа | Токены (BPE) | L1 Карточка |
| :--- | :--- | :--- | :--- | :--- |
| **Main** | Подсистема `src/core` с ключевыми точками входа: src/main.rs | `src/lib.rs` | ~27 | [subsystems/src_core.md](subsystems/src_core.md) |
| **Detector** | Модуль и компоненты директории `src/detector` (1 файлов) | `-` | ~14445 | [subsystems/src_detector.md](subsystems/src_detector.md) |
| **Generator** | Модуль и компоненты директории `src/generator` (1 файлов) | `-` | ~6959 | [subsystems/src_generator.md](subsystems/src_generator.md) |
| **Ollama** | Модуль и компоненты директории `src/ollama` (1 файлов) | `-` | ~1183 | [subsystems/src_ollama.md](subsystems/src_ollama.md) |
| **Types** | Модуль и компоненты директории `src/types` (1 файлов) | `-` | ~2080 | [subsystems/src_types.md](subsystems/src_types.md) |
| **Mcp** | Модуль и компоненты директории `src/mcp` (1 файлов) | `-` | ~3761 | [subsystems/src_mcp.md](subsystems/src_mcp.md) |
| **Service** | Модуль и компоненты директории `src/service` (1 файлов) | `-` | ~6833 | [subsystems/src_service.md](subsystems/src_service.md) |
| **Reports** | Модуль и компоненты директории `reports` (2 файлов) | `-` | ~1265 | [subsystems/reports.md](subsystems/reports.md) |
| **Tests** | Модуль и компоненты директории `tests` (1 файлов) | `-` | ~4845 | [subsystems/tests.md](subsystems/tests.md) |

## 📋 Инварианты и правила проекта
- Все внешние крейты и зависимости строго объявляются в Cargo.toml.
- Код должен проходить `rtk cargo clippy --all-targets` и `rtk cargo fmt --check` без ошибок.
- Использование `unsafe` запрещено без исчерпывающего комментария безопасности `// SAFETY:`.
- Публичные структуры и функции должны сопровождаться docstring (`///`).
