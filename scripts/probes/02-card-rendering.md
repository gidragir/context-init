# Probe 02: Card Rendering

## Question
Как устроен процесс генерации карточек подсистем L1: в каком формате они формируются, какие метаданные включаются во frontmatter, какие основные секции генерируются в теле документа и как защищаются пользовательские правки при повторном обновлении?

## Kind
overview

## Expected
- `crates/ci-core/src/analysis/generator.rs:98`: Функция `render_subsystem_card_okf` оборачивает сгенерированный markdown карточки в OpenViking OKF (sidecar) с YAML frontmatter (`directory`, `source`, `generated_by`, `freshness`), описанным типами в `crates/ci-core/src/openviking.rs:70`.
- `crates/ci-core/src/analysis/generator.rs:849`: Функция `render_subsystem_card` формирует разделы карточки L1: заголовок `# Подсистема: {name} (L1)`, теги и id, секцию `## 📁 Точки входа и структура файлов` со статусами файлов («⭐ Точка входа», «Исходный код»), `## 🔑 Ключевой контракт и экспорты`, и `## 🧪 Запуск и тестирование`.
- `crates/ci-core/src/analysis/generator.rs:150`: Функция `card_content` сохраняет существующую карточку на диске без перезаписи, если изменения касаются только метаданных provenance (`same_card(&disk, &fresh)`) и git подтверждает актуальность (`!is_git_stale`).
