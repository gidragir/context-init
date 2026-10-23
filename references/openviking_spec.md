# Спецификация OpenViking (L0–L2 Reference)

> Источник: volcengine/OpenViking (`docs/en/concepts/03-context-layers.md`, `14-ovpack.md`).
> Назначение: Архитектурный эталон иерархического контекста для `context-init`.

---

## 1. Концепция трехуровневого контекста (L0–L2)

| Слой | Имя | Назначение | Бюджет символов | Исходный формат в OpenViking |
| :--- | :--- | :--- | :--- | :--- |
| **L0** | Abstract | Экспресс-фильтрация, векторный индекс, глобальный обзор | ~256 символов (~50 токенов) | `.abstract.md` |
| **L1** | Overview | Навигация, скоуп подсистемы, реранкинг, принятие решения об L2 | ~4000 символов (~800 токенов) | `.overview.md` |
| **L2** | Detail | Полный контекст кода/структуры, методы, сигнатуры, ханки | По требованию (On-Demand) | Исходные файлы / AST-пакеты |

---

## 2. Формат карточек (OKF Sidecar: Frontmatter + Markdown Body)

Каждая карточка L0/L1 состоит из строгого YAML frontmatter и markdown-тела:

```markdown
---
directory: <нормализованный URI или относительный путь>
source:
  kind: git
  blob_sha: <git_blob_sha>
  commit_sha: <commit_generation_sha>
generated_by:
  component: context-init
  role: <init|update|task>
  model: <alias>
freshness:
  total_entries: 12
  sampled_entries: 12
  unsampled_entries: 0
  pending_child_changes: 0
---

# Заголовок подсистемы / объекта

Краткое описание сущности (L0-абстракт берется из первого параграфа после H1).

## Quick Navigation / Экспортные интерфейсы
- `item_1`: описание интерфейса или метода
- `item_2`: ключевая зона ответственности
```

---

## 3. Принципы агрегации и дрейфа (Freshness & Bubbling)

1. **Bottom-Up генерация:**
   `Файлы/Символы` $\rightarrow$ `L1 подсистемы` $\rightarrow$ `L0 индекс подсистемы` $\rightarrow$ `L0 корень проекта (workspace)`.
2. **Агрегация родителя:**
   L1 подсистемы агрегирует только L0-абстракты своих прямых дочерних модулей/файлов.
3. **Ленивая валидация (Stale):**
   - Карточка содержит `blob_sha` исходников на момент генерации.
   - При чтении выполняется быстрый `git diff --name-only <commit_sha>..HEAD`.
   - Если хэши не совпадают, выставляется статус `stale = true` и возвращается сжатый `diff --stat`.
4. **Бюджет ответа MCP:**
   - Ни один ответ инструмента (`context_get_subsystem`, `context_pack`, `context_search`) не должен превышать жесткий лимит (~4000 токенов).
   - Превышение разбивается курсорной пагинацией (`next_cursor`).
