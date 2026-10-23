# 🚀 context-init

CLI-утилита на Rust для автоматической инициализации и поддержания трехуровневой базы знаний (**OpenViking L0-L2**) для автономных ИИ-ассистентов (Antigravity, Hermes, Claude Code, Cursor, Aider).

---

## 🧭 Зачем это нужно ИИ-ассистентам

Большинство ИИ тратят тысячи токенов и десятки запросов на слепое рекурсивное сканирование кодовой базы (`find`, `grep`, `ls`). `context-init` решает эту проблему:
1. **L0 (Mental Map):** Компактный индекс (`L0_index.md` и `L0_index.json`) со списком подсистем, корневыми точками входа и ключевыми командами сборки/тестов.
2. **L1 (Subsystems):** Изолированные карточки подсистем (`subsystems/*.md`) с фактическими таблицами файлов, точек входа, сигнатурами экспортов (`pub fn`, `export class`, etc.) и таргетированными командами тестов.
3. **L2 (Operational & Architecture):** Справочник реальных команд (`cli/commands.md`) и история архитектурных решений (`adr/`).
4. **Интеграция с агентами:** Автоматически создает/дополняет корневой `AGENTS.md` ссылками на базу знаний.

---

## 🛠 Сборка и установка

```bash
# Быстрая сборка в release режиме:
cargo build --release

# Установка в систему (~/.local/bin):
cp target/release/context-init ~/.local/bin/

# Либо через cargo:
cargo install --path . --root ~/.local
```

---

## ⚡️ Использование

### 1. Первичная инициализация (`init`)

```bash
# Инициализация текущего проекта (база знаний пишется в ~/.local/share/context-init/projects/<id>/):
context-init init .

# С глубоким извлечением сигнатур публичных функций, типов и структур:
context-init init . --deep

# Режим предварительного просмотра без записи на диск:
context-init init . --dry-run

# С описанием подсистем через любую LLM из конфига (алиас из [models]):
context-init init . --model local

# Отдельная модель для конкретной роли (повторяемый флаг):
context-init init . --role init=local,none
```

Модели задаются в `~/.config/context-init/config.toml` (или `<repo>/.context-init.toml`):

```toml
[models.local]
kind = "ollama"
model = "qwen2.5:14b-ctx64k"
endpoint = "http://127.0.0.1:11434"

[roles]
init = ["local", "none"]   # "none" = детерминированный режим без LLM (по умолчанию)
```

#### Миграция с v0.2

| Было (v0.2) | Стало (v0.3) |
| :--- | :--- |
| `context-init .` / `context-init --deep .` | `context-init init .` / `context-init init . --deep` |
| `--ai -m qwen2.5:14b-ctx64k` | `[models.local] kind = "ollama"` + `--model local` |
| `--ollama-url <URL>` | `endpoint = "<URL>"` в `[models.<alias>]` |
| `--with-agents-md` (всегда включено) | по умолчанию включено, `--no-agents-md` отключает |
| `.agents/knowledge/` в репозитории | центральное хранилище; перенос — `context-init migrate` |

### 2. Проверка дрейфа архитектуры (`check`)

Выявляет расхождения между кодовой базой на диске и базой знаний (удаленные папки, незадокументированные новые модули):

```bash
context-init check .

# Однострочный машиночитаемый вывод:
context-init check --compact .

# Для CI/CD пайплайнов и git pre-commit хуков (код возврата 1 при дрейфе):
context-init check --exit-code --compact .
```

### 3. Инкрементальное обновление (`update`)

Добавляет новые подсистемы и обновляет команды без затирания ручных правок в существующих карточках L1:

```bash
context-init update .

# С глубоким парсингом сигнатур и BPE токенометрией:
context-init update --deep .
```

### 4. Упаковка контекста подсистемы (`pack`)

Быстрая выгрузка файлов подсистемы в компактный AI-контекст. По умолчанию используется встроенный Rust AST-пакер (в 5–6 раз быстрее и в 3–5 раз компактнее за счет стриппинга тел функций и нормализации токенов):

```bash
# Упаковка конкретной подсистемы в XML (вывод в stdout, сжатие включено по умолчанию):
context-init pack detector

# Упаковка без сжатия (сохраняет полный исходный код файлов):
context-init pack detector --no-compress

# Упаковка в файл с форматированием Markdown:
context-init pack generator --style markdown -o generator_pack.md

# Использование внешней утилиты Repomix (при наличии в $PATH):
context-init pack detector --repomix

# Упаковка всех подсистем проекта:
context-init pack all -o full_context.xml
```

### 5. Интеграция с RTK (Rust Token Killer)

Все обнаруженные команды сборки, тестирования и линтинга автоматически оборачиваются префиксом `rtk` при его наличии в `$PATH` или передаче флага `--with-rtk`:

```bash
context-init init --with-rtk .
```

---

## 📚 Структура артефактов

```text
.agents/
├── AGENTS.md                  # Корневой навигатор и правила для ассистента
└── knowledge/
    ├── L0_index.md            # Компактная ментальная карта (оптимизирована под LLM)
    ├── L0_index.json          # Машиночитаемый JSON-индекс со схемой
    ├── subsystems/            # L1-карточки подсистем
    │   ├── core.md
    │   └── detector.md
    ├── cli/
    │   └── commands.md        # L2-справочник фактических команд
    └── adr/
        └── 0001_initial_architecture.md
```
