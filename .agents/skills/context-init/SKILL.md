---
name: context-init
description: >-
  Руководство по использованию context-init: нативный MCP-сервер и CLI-утилита
  для генерации, аудита архитектурного дрейфа (check), инкрементального обновления (update)
  и автономной AST-упаковки (pack) трехуровневой базы знаний OpenViking (L0-L2).
---

# context-init: Управление контекстом OpenViking L0-L2

Утилита и MCP-сервер `context-init` создают и поддерживают компактную ментальную карту проекта для ИИ-ассистентов, устраняя необходимость рекурсивного сканирования репозитория и экономя до 95% токенов.

---

## ⚡️ Приоритет взаимодействия (MCP-First Protocol)

> **ZERO-VIOLATION INVARIANT (MCP-FIRST PROTOCOL):** Если в среде зарегистрирован MCP-сервер `context-init`, агент **ОБЯЗАН** использовать native MCP-инструменты вместо вызова команд через bash-терминал.
> Терминальные команды `context-init` используются только как fallback при отсутствии прямого MCP-подключения и строго с префиксом `rtk`.
>
> **NEGATIVE CONSTRAINT (ZERO DIRECT FILE ACCESS):** Категорически **ЗАПРЕЩЕНО** читать файлы `.agents/knowledge/**` напрямую через инструменты файлового доступа (`view_file`, `cat`, `headroom_compress` и др.). Вся навигация, чтение ментальной карты и карточек подсистем выполняются **ИСКЛЮЧИТЕЛЬНО** через вызовы MCP-инструментов `context_get_subsystems` и `context_get_subsystem`.

---

## 🛡 Архитектурные инварианты и гигиена (Invariants)

1. **Ignore Engine (Сегментная валидация):**
   - Каталоги экосистем и сборщиков (`.vault`, `.turbo`, `out`, `.svelte-kit`, `.astro`, `.parcel-cache`) обязаны игнорироваться на уровне всех сегментов пути (`is_ignored_path`), предотвращая попадание служебных файлов монорепозиториев в базу знаний.
2. **Linter & Formatter Hygiene (POSIX Newline):**
   - Все файлы JSON/MD (`L0_index.json`, `L0_index.md`, отчеты) сериализуются с гарантированным завершающим переводом строки `\n` (`0x0A`), обеспечивая совместимость со строгими проверками `biome check` и pre-commit линтерами.

---

## 🔌 Набор MCP-инструментов (Tools)

Сервер запускается командой `context-init mcp [path]` (транспорт `stdio`).

| MCP Tool | Назначение | Параметры | Результат |
| :--- | :--- | :--- | :--- |
| `context_check` | Быстрая проверка дрейфа базы знаний относительно диска | `path` (опционально) | Structured JSON: `{ has_drift: bool, missing_in_fs: [...], untracked_in_knowledge: [...], up_to_date_count: usize, message: str }` |
| `context_pack` | Получение AST-сжатого контекста подсистемы (вырезание тел функций до `⋮----`) | `subsystem_id`, `path`, `compress` (true), `style` ("xml" \| "markdown") | Текст со сжатым исходным кодом интерфейсов и заголовком со статистикой токенов |
| `context_get_subsystems` | Список всех подсистем проекта и их метрик | `path` (опционально) | Массив объектов `SubsystemInfo` из `L0_index.json` (точки входа, ключевые файлы, BPE токены) |
| `context_get_subsystem` | Получение компактной L1-карточки подсистемы (описание, точки входа, ключевые файлы) | `subsystem_id`, `path` (опционально) | Текст L1-карточки в формате Markdown без раздувания контекста |
| `context_update` | Инкрементальное обновление базы знаний при обнаружении новых файлов/модулей | `path`, `deep` (false), `model` (алиас из `[models]`), `prune` (false) | Текстовый статус синхронизации добавленных/обновленных/удалённых модулей |
| `context_init` | Первичная генерация L0-L2 базы знаний | `path`, `deep`, `model`, `force`, `with_agents_md` | Текстовый статус инициализации |

---

## 🧠 Трехэтапный когнитивный цикл агента

1. **Фаза навигации (L0/L1):**
   - **Шаг 1 (L0):** Вызов `context_get_subsystems` для получения структурированного списка всех модулей, их границ и назначения.
   - **Шаг 2 (L1):** Вызов `context_get_subsystem(subsystem_id="...")` для получения точечной L1-карточки подсистемы (точки входа, ключевые файлы, тестовые команды). Прямое чтение файлов через `view_file` категорически запрещено.
2. **Фаза загрузки кода (Pack):**
   - Вызов `context_pack(subsystem_id="...")` для получения AST-сжатых интерфейсов модуля (вырезание тел функций до `⋮----`) вместо ручного вычитывания сырых файлов с диска.
3. **Фаза исполнения и верификации (Check/L2):**
   - Перед рефакторингом, фиксацией изменений или коммитом: обязательный вызов `context_check`. При наличии `has_drift: true` вызов `context_update`.
   - Запуск команд сборки и тестирования проекта строго по справочнику `.agents/knowledge/cli/commands.md` с префиксом `rtk`.

---

## 🛠 Fallback: Справочник CLI-команд (при отсутствии MCP)

При отсутствии MCP-соединения команды запускаются в терминале строго через `rtk`:

### 1. Запуск MCP-сервера вручную
```bash
rtk context-init mcp .
```

### 2. Аудит архитектурного дрейфа (`check`)
```bash
# Машиночитаемая однострочная проверка (<10 мс)
rtk context-init check --compact .

# Для git хуков и CI (код возврата 1 при дрейфе)
rtk context-init check --compact --exit-code .
```

### 3. Инкрементальное обновление (`update`)
```bash
# Обычное обновление
rtk context-init update . --compact

# Очистка удалённых модулей (missing_in_fs) и удаление устаревших L1-карточек
rtk context-init update . --prune --compact
```

### 4. Упаковка контекста подсистемы (`pack`)
```bash
# Встроенный автономный AST-компрессор в stdout
rtk context-init pack <subsystem_id> -o -

# Запись в файл
rtk context-init pack <subsystem_id> -o /tmp/context.xml
```

### 5. Первичная инициализация (`init`)
```bash
rtk context-init init . --deep --with-rtk --compact
```

### 6. Регламент выпуска релиза

1. **Бамп версии в `Cargo.toml`:**
   - Обновить `[workspace.package].version` и версии внутренних зависимостей `ci-core`, `ci-llm`, `ci-mcp`.
   - Обновить тесты `smoke_version_*` в `crates/ci-*/src/lib.rs`.
   - Если менялись или объединялись подсистемы, актуализировать `SEARCH_GOLD` в `tests/regression_benchmark.rs`.
2. **Форматирование, линтинг и тесты:**
   ```bash
   rtk cargo fmt --all -- --check
   rtk cargo clippy --workspace --all-targets -- -D warnings
   rtk cargo test --workspace
   ```
3. **Синхронизация базы знаний (устранение `git_stale`):**
   ```bash
   rtk cargo run -- update . --prune --compact
   rtk git add -A && rtk git commit -m "chore(release): vX.Y.Z"
   rtk cargo run -- init . --force --compact
   rtk git add -A && rtk git commit -m "chore(kb): sync knowledge base with release vX.Y.Z"
   rtk cargo run -- check . --compact --exit-code
   ```
4. **Установка и тегирование:**
   ```bash
   mise run install
   rtk git tag -a vX.Y.Z -m "Release vX.Y.Z"
   rtk git push origin main --tags
   ```
