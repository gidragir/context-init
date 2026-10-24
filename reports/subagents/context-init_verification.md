# Отчет о независимой верификации Шага 5 вехи v0.3 (bin `context-init` поверх `ci-*`)

**Дата аудита:** 2026-10-23
**Аудитор:** независимый верификатор. Работал только на чтение исходников. Бинарь запускался только с поддельными `HOME`/`XDG_*` в scratchpad. Единственная запись — этот отчет.
**Диапазон:** `019e04b..7f2c761` (17 коммитов, ветка `feat/v0.3-step5`)
**Итоговый статус:** **PASS WITH NOTES.** Гейт §9 пройден, кроме п. 3 («≥ подсистем»): там осознанное отклонение, см. M12. Тесты и clippy зеленые. Конвейер init/update действительно общий для CLI и MCP. Ни одного Critical. Есть 3 замечания Important:
- I1: чтение произвольных файлов через пути в L0-индексе (`pack`, `context_pack`, `context_get_subsystem`);
- I2: бэкапы `setup` доступны на чтение всем, даже если оригинал был `0600`;
- I3: после `init` на legacy-проекте пользователю советуют удалить in-repo базу, хотя ее ручные файлы в центральное хранилище не перенесены.

I1 и I2 рекомендуется исправить до слияния. **Ready to merge: With fixes.**

---

## 1. Гейт приемки (spec §9)

| # | Пункт | Статус | Доказательство |
| :--- | :--- | :--- | :--- |
| 9.1 | `cargo check` / `clippy -D warnings` / `nextest --workspace` | **PASS** | §3: check OK; clippy «No issues found», exit 0; nextest «147 passed (6 binaries)», exit 0 |
| 9.2 | `rg -i '1[cс]'` по коду — 0 | **PASS** | `rtk proxy rg -i -c '1[cс]' crates src tests` → exit 1, совпадений нет |
| 9.3a | `check` на репозитории без дрейфа | **PASS** | Снимок HEAD (`git archive`), поддельный HOME: после `init` → `check . --exit-code --compact` выдает `OK: 7 subsystems up-to-date`, exit 0 |
| 9.3b | `init` пишет в центральное хранилище | **PASS** | `OK: initialized 7 subsystems in <XDG_DATA_HOME>/context-init/projects/p3-…`. `git status` чистый, кроме `AGENTS.md` (ожидаемо, `with_agents_md`) |
| 9.3c | `init` дает ≥ подсистем, чем текущий `L0_index.json` | **FAIL (принятое отклонение)** | 7 против 9 в закоммиченном `.agents/knowledge/L0_index.json`. Ruling Task 16: дерево изменилось. См. M12 |
| 9.4 | Smoke: `setup claude-code` (dry-run diff), `config validate`, `migrate --dry-run`, `telemetry report` | **PASS** | Все четыре выполнены в поддельном HOME (§3.3). Dry-run ничего не пишет |
| 9.5 | Независимый отчет | **PASS** | этот файл |

## 2. Чек-лист по фазам

| Фаза / задача | Статус | Доказательство / замечание |
| :--- | :--- | :--- |
| 0 / F1: `context_submit`, обход пути | **PASS** | `storage.rs:275-301` (`validate_subsystem_id` + `subsystem_card_path`). `handlers.rs:383-460`: все id проверяются до любой записи, `files`/`l1_path` из submission не принимаются (`l1_path` формируется из проверенного id, `:424`). Тесты `lib.rs:620,661`. Остаточный риск (симлинк `subsystems/` в legacy-базе) — см. M13 |
| 0 / F2: неизвестный `package_id` | **PASS** | `handlers.rs:475-492` |
| 0 / F3: `roots/list` | **PASS (проверено вживую)** | `server.rs:429-453`. Живой JSON-RPC зонд (python, stdio, `cwd=/`): после `notifications/initialized` сервер шлет `roots/list`. Ответ `file://…/p4` делает `context_get_subsystems` без аргументов рабочим для этого корня. Дедлока нет |
| 0 / F4: лимит `pack all`, переполнение курсора | **PASS** | `handlers.rs` (`PACK_ALL_MAX_PAGES.saturating_mul(budget)`); `pagination.rs` (`saturating_add`) |
| 1 / Task 5–6: `ci_core::analysis` | **PASS** | detector/generator/pack/subsystems перенесены, `compress_rust_syntax` удален, дубли типов сведены в `ci_core::storage` |
| 1 / Task 7: `ci_llm::enrich` | **PASS** | `enrich.rs:30-80`: ошибка провайдера → `Failed{warning}`, `none` → `Skipped`, `agent` → `PendingAgent`. Обогащение меняет только name/summary/tags существующих id |
| 1 / Task 8: MCP-обработчики на общем конвейере | **PASS** | `handlers.rs:650-694` → `pipeline::run_init/run_update` |
| 1 / Task 9: тонкий bin, глобальные флаги | **PASS** | `src/main.rs` + `src/cli/*`. `--model`/`--role`/`--config`/`--compact` превращаются в `CliOverrides`. Опечатка в `--model` не останавливает работу — см. M7 |
| 2 / Task 10: `config` | **PASS** | init (повтор без `--force` → exit 1), show `--json`, path, validate. Зонд: 3 ошибки сразу, exit 1. Предупреждение `<64` при загрузке, а не ошибка |
| 2 / Task 11: `migrate` | **PASS WITH NOTES** | copy / dry-run / remove-source / «уже мигрировано без `--force`» → ошибка. Вложенные базы: мигрирует только первая (M2) |
| 3 / Task 12–13: `setup` | **PASS WITH NOTES** | Комментарии TOML сохраняются, невалидный JSON → ошибка, файл не тронут (`cmp` подтвердил). Повторный `--write` дает `unchanged` без бэкапа. Пути с пробелами, `ü` и `"` проходят туда и обратно. Замечания: I2, M3–M6, M8 |
| 4 / Task 14: `telemetry` | **PASS** | Без tty и без `--yes` → exit 1, ничего не отправлено. С tty сначала печатается дайджест, затем вопрос `[y/N]`. Заметки скрыты по умолчанию. Мелочь — M9 |
| 4 / Task 15: `feedback` | **PASS** | При выключенной телеметрии: «Feedback not recorded…». При включенной: «Feedback recorded» и запись в дайджесте |
| Единый конвейер CLI ↔ MCP | **PASS для init/update/pack. `check` расходится (M1)** | `pipeline.rs` общий. `pack_subsystem` общий. `check` собирается в двух местах с разной логикой |

## 3. Вывод команд (фактический)

### 3.1 Сборка и тесты
```
$ timeout 300 rtk cargo nextest run --workspace
cargo nextest: 147 passed (6 binaries, 0.489s)          exit=0
$ timeout 300 rtk cargo clippy --workspace --all-targets -- -D warnings
cargo clippy: No issues found                           exit=0
$ timeout 300 rtk cargo check --workspace
Finished `dev` profile [unoptimized + debuginfo] target(s)
$ rtk proxy rg -i -c '1[cс]' crates src tests           exit=1 (0 совпадений)
```

### 3.2 Классы входов из Review Focus

1. **Необычные пользовательские конфиги (`setup`).**
   - `~/.codex/config.toml` с комментариями (`# top comment`, `# inline`, `# keep`) и `env = { KEY = "secret" }`: после `--write` комментарии и чужие серверы на месте. Второй прогон → `unchanged`, бэкап не создается.
   - `~/.cursor/mcp.json` с висячей запятой → `error: … existing config is not valid JSON: trailing comma at line 3 column 1`, exit 1, `cmp` с оригиналом совпадает.
   - Минифицированный `~/.claude.json` на 20.9 МБ: dry-run занял 0.09 с, но **вывел 43.2 МБ diff** (весь файл как `-`, затем как `+`), см. M5. `--write` выполнился за 0.06 с. Целое `12345678901234567890123` стало `1.2345678901234568e+22`, см. M5.
   - Бэкап файла с правами `0600` создан с правами `-rw-r--r--`, см. I2.
2. **Legacy-база в репозитории** (снимок `019e04b` с настоящей `.agents/knowledge`).
   - `check` читает legacy и выводит `note: Reading the legacy…`.
   - `pack src_mcp` работает. `migrate --dry-run` показывает 14 файлов.
   - `init` пишет центральную базу (13 подсистем) и советует удалить in-repo копию.
   - `adr/0002_ollama_json_wrapper.md` в центральную базу не попал: `init` сгенерировал только шаблонный `0001` с новой датой. См. I3.
   - После этого `migrate` → `error: central knowledge base already exists…; pass --force`.
   - `migrate --force` затирает свежий индекс legacy-индексом на 9 подсистем, `check` → `DRIFT … untracked=[crates_ci-core, crates_ci-llm, crates_ci-mcp, references]`.
   - Две вложенные базы (`alpha`, `beta`) + `--remove-source`: мигрирует и удаляется только `alpha`. `beta` остается и больше никогда не читается, предупреждения нет. См. M2.
3. **Модель `kind = "agent"` в CLI** (`[roles] init = ["ag"]`).
   - `init` и `update` завершаются с exit 0 за доли секунды и пишут `note: Model kind "agent" is answered by an AI harness over MCP…`. Детерминированные summary сохранены, зависаний нет.
   - Попутно: `--model nosuch init` → exit 0 с note «LLM enrichment failed…», см. M7.
4. **Пробелы и необычные символы в путях.** `current_exe()` = `…/bin dir/context init`, HOME = `…/home with space/ü "q"`. В TOML записано `command = "…/bin dir/context init"`, в JSON — корректная строка, `json.load` читает. Unit-тест `setup_exe_path_with_spaces_roundtrips` покрывает `"`.
5. **Повторные запуски.**
   - `setup --write` ×2 → `unchanged`.
   - `init` ×2 → второй с exit 1: «уже существует… --force или update».
   - `init --force` ×2 и `update`: блок навигации в `AGENTS.md` не дублируется.
   - `migrate` после миграции без `--force` → ошибка.
   - `config init` ×2 → ошибка.

### 3.3 Smoke гейта 9.4 (поддельный HOME)
```
setup claude-code         -> "--- …/.claude.json / +++ … / @@ line N @@" + SKILL.md /dev/null diff; "Dry run: nothing was written."
config validate           -> "OK: configuration is valid (0 source file(s))"
migrate --dry-run .       -> "OK: would migrate 14 files (9 subsystems) …/.agents/knowledge -> …/projects/p3-371094aa"
telemetry report          -> "Telemetry is off (opt-in). To record … [telemetry] enabled = true"
telemetry report --submit </dev/null -> "error: refusing to submit without confirmation on a non-interactive stdin; pass --yes" (exit 1)
```

## 4. Замечания

### Critical
Нет.

### Important

**I1. Пути в L0-индексе позволяют читать произвольные файлы вне проекта и базы знаний.**
`crates/ci-core/src/analysis/pack.rs:79` (`knowledge_dir.join(&sub.l1_path)`), `:99` (`root.join(path)` для `files[].path`/`key_files`), `:137`, `:163` (`root.join(file_rel)` при чтении), `crates/ci-mcp/src/handlers.rs:273` (`context_get_subsystem`), `handlers.rs:112` (staleness).
- `Path::join` с абсолютным путем заменяет базу, а `..` не фильтруется.
- Legacy in-repo индекс — это контент репозитория, а для чужого склонированного репо он недоверенный. Он читается напрямую через D21 lazy fallback, а `migrate` копирует его в центральное хранилище без санитизации (`upgrade_to_current` только нормализует слеши).
- **Сценарий (воспроизведен):** репозиторий содержит `.agents/knowledge/L0_index.json` с `{"id":"evil","l1_path":"/abs/secret.txt"}` и `{"id":"evil2","key_files":["../secret.txt"]}`. `context-init pack evil .` и `pack evil2 .` выводят `SECRET-OUTSIDE` в XML-пак. Через MCP `context_pack` / `context_get_subsystem` те же байты попадают в контекст агента (например `~/.ssh/id_*`, `~/.aws/credentials`).
- Чтение через `files`/`key_files` было и до Шага 5. Fallback на L1-карточку с абсолютным путем добавлен в этом диапазоне (Ruling Task 9: «cost if wrong: none»).
- **Исправление:** в `pack_subsystem` и при чтении карточек отвергать абсолютные пути и компоненты `ParentDir`. Пути из `files`/`key_files` канонизировать и требовать `starts_with(root)`, `l1_path` — `starts_with(knowledge_dir)`. Санитизировать при `migrate`/`load_l0_index`.

**I2. Бэкапы `setup` создаются с правами по умолчанию (0644) и раскрывают секреты из файлов с правами 0600.**
`src/cli/setup/merge.rs:137-138` (`fs::write(&b, old)`).
- **Сценарий (воспроизведен):** у `~/.codex/config.toml` права `0600` и в нем `env = { KEY = "secret" }` (то же относится к `~/.claude.json`, где хранятся данные аккаунта/ключи). После `setup codex --write` появляется `config.toml.bak.20261023-173312` с правами `-rw-r--r--`. На системах с домашним каталогом 0755 этот файл может прочитать любой локальный пользователь.
- Бэкапы копятся при каждом изменении и не удаляются.
- **Исправление:** копировать права оригинала (`fs::copy` переносит mode) или создавать бэкап с `0600`. Писать через временный файл + `rename` (см. M8).

**I3. `init` на непромигрированном проекте советует удалить in-repo базу, хотя ручные файлы из нее не перенесены, а `migrate` после этого либо блокирован, либо портит индекс.**
`crates/ci-mcp/src/pipeline.rs:102` (текст «…is no longer read…; you can delete it.»), вызовы в `src/cli/init_update.rs:102` и `crates/ci-mcp/src/handlers.rs:668`. Подсказка выводится и при `init --dry-run`, когда ничего не записано.
- **Сценарий (воспроизведен на снимке этого же репозитория):** пользователь запускает `context-init init` вместо `migrate`. Центральная база сгенерирована заново. Ручной `adr/0002_ollama_json_wrapper.md` (и любые ручные правки карточек) остается только в репозитории, а CLI и MCP рекомендуют его удалить.
- Попытка восстановления: `migrate` → ошибка без `--force`; `migrate --force` заменяет свежий индекс на 13 подсистем устаревшим legacy-индексом на 9, и `check` показывает дрейф.
- Review Focus #2 («init doesn't silently diverge») выполнен лишь частично: расхождение сопровождается советом, который ведет к потере данных.
- **Исправление:** если legacy существует, `init` либо отказывает с подсказкой `migrate` (без `--force`), либо копирует несгенерированные файлы (`adr/`, `cli/`, ручные карточки) в центральную базу. Подсказку заменить на «run `context-init migrate --force` to carry over hand-written files» или убрать «you can delete it».

### Minor

- **M1.** `src/cli/check.rs:29-30` против `crates/ci-mcp/src/handlers.rs:162-186`: CLI `check` не учитывает git-устаревание карточек, MCP `context_check` учитывает (`git_stale`). После коммита в подсистему MCP сообщает `DRIFT`, а `context-init check --compact` / `--exit-code` — `OK`. Правило `setup` при этом объявляет CLI-команду fallback-эквивалентом `context_check`. Кроме того, MCP при ошибке загрузки индекса или детектора отвечает `DRIFT` с пустыми списками и скрывает причину. Логику `check` стоит вынести в общую функцию.
- **M2.** `crates/ci-core/src/storage.rs:531-547`: при нескольких вложенных `.agents/knowledge/<name>/` мигрирует только первая по алфавиту. После `--remove-source` остальные лежат в репозитории, больше никогда не читаются (central активен), предупреждения нет. Стоит предупреждать или отказывать при >1 базе.
- **M3.** `src/cli/setup/mod.rs:108`: `setup all` выбирает харнессы по существованию родительского каталога MCP-файла. Для `claude-code` это `$HOME`, который есть всегда. Проверено на пустом HOME: в `all` попадает `claude-code`, и `--write` создаст `~/.claude.json` и `~/.claude/skills/…` у пользователя без Claude Code.
- **M4.** `src/cli/setup/mod.rs:118`: в `setup all --write` планирование идет внутри цикла записи. Если конфиг одного из следующих харнессов невалиден, команда завершается ошибкой после того, как предыдущие харнессы уже записаны, а остальные не обработаны. Лучше сначала спланировать все, потом писать.
- **M5.** `src/cli/setup/merge.rs:39`: файл целиком перезаписывается через `to_string_pretty`.
  - Минифицированный `~/.claude.json` на 20.9 МБ дал 43.2 МБ dry-run diff, хотя спека обещает «unified diff предлагаемых изменений».
  - Без `arbitrary_precision` большие целые теряют точность (`12345678901234567890123` → `1.2345678901234568e+22`), а `1e3` превращается в `1000.0`. Это меняет данные за пределами ключа `context-init`.
- **M6.** `merge.rs:44-63`: валидный TOML с inline-таблицами (`mcp_servers = { … }` или `[mcp_servers]` + `context-init = { command = … }`) отклоняется ошибкой «is not a table». Файл не трогается, но настройка невозможна.
- **M7.** `crates/ci-llm/src/router.rs:216`: `--model nosuch` (и `--role init=nosuch`) дает только `note` и exit 0. Пользователь явно попросил модель, а получил детерминированный результат «успешно». `config validate` при этом опечатку ловит. Неизвестный алиас из CLI-флага стоит считать ошибкой.
- **M8.** `merge.rs:143`: запись через `fs::write` неатомарна (truncate + write). `~/.claude.json` параллельно переписывается работающим Claude Code. Сбой или гонка повредит файл, бэкап при этом есть. Лучше писать во временный файл и делать `rename`.
- **M9.** `src/cli/telemetry.rs:136`: тело issue пишется в общий `temp_dir()` под предсказуемым именем `context-init-telemetry-<pid>.md`, а `fs::write` следует по симлинкам. Лучше `tempfile`/`create_new`.
- **M10.** `crates/ci-core/src/analysis/mod.rs:44-46` против `storage.rs:531`: корневой `L0_index.json` считается legacy-базой при чтении, но `migrate` его не находит («In-repo L0_index.json not found»). `update` и `context_submit` пишут `subsystems/*.md` прямо в корень репозитория. `legacy_hint` для этого случая молчит.
- **M11.** Закоммиченная `.agents/knowledge/` этого репозитория устарела: 9 подсистем, в том числе удаленные `src_detector`, `src_mcp` и т. д. Любой свежий клон без центральной копии (CI, другой разработчик) читает ее и видит дрейф. Ее стоит удалить или мигрировать в рамках ветки.
- **M12.** Гейт §9.3c не выполнен буквально (7 < 9). Обоснование ruling'а принимаю (7 однофайловых модулей `src/*.rs` превратились в 3 крейта), но это отклонение от обязательного гейта, и подтвердить его должен владелец.
- **M13.** `storage.rs:296-298`: проверка `starts_with` лексическая. Если в legacy-базе из репозитория `subsystems` — симлинк, `context_submit`/`update` (активный каталог = legacy) запишут `<id>.md` по ссылке за пределами базы. Нужна атакующая структура репозитория. Исправление: `symlink_metadata` или канонизация родителя.

## 5. Оценка ruling'ов исполнителя

Согласен со всеми, кроме перечисленных ниже.

- **Task 9 (L1-card fallback, «cost if wrong: none») — не согласен.** Fallback принимает `l1_path` из индекса без ограничений и возвращает абсолютный путь. Это новый вектор I1.
- **Task 8 (`context_init` всегда пишет central + подсказка) — частично не согласен.** Писать в central правильно (D1/D23). Но текст «you can delete it» и отсутствие переноса ручных файлов — это I3. Ruling оценивает цену ошибки как «two KBs», хотя реальная цена — потеря ADR при выполнении совета.
- **Task 11 (вложенные базы, «migrate only the first») — частично не согласен.** Поведение допустимо, но молчаливое. Нужно предупреждение (M2).
- **Task 16 (гейт 7 < 9) — согласен по существу**, но вынести на подтверждение владельцу (M12).
- **Task 12 (merge обновляет ключи существующей записи) — согласен.** Риск «stale custom keys survive» мал, `env` сохранить важнее.
- **Task 0 (`cp --remove-destination`) — согласен** для Linux-хоста. На macOS хук сломается.

## 6. Сильные стороны

- F1 закрыт грамотно: валидация всех id до первой записи, `files`/`l1_path` из submission игнорируются, ошибки I/O возвращаются агенту (`handlers.rs:383-460`).
- Конвейер init/update один для CLI и MCP (`ci_mcp::pipeline`). `pack_subsystem` тоже общий. В MCP заглушки init/update заменены реальным детектором.
- `setup`: TOML через `toml_edit` с сохранением комментариев, JSON с `preserve_order`, без изменений — без записи, невалидный файл — без записи, маркерные блоки идемпотентны.
- Отправка телеметрии никогда не идет без подтверждения: без tty требуется `--yes`.
- `migrate --remove-source` удаляет источник только после проверки наличия каждого файла в целевом каталоге. `remove_dir_all` не следует по симлинкам. Пустые `.agents` удаляются только если пусты.
- Живой обмен `roots/list` работает (проверено зондом, вне тестов исполнителя).

## 7. Declined to judge

- **Производительность и гранулярность детектора** (`crates_ci-core` ≈ 73k токенов в одной подсистеме): исполнитель указал это как follow-up; это вне скоупа Шага 5.
- **Полнота сигналов D29** (операционные события в MCP-обработчиках): спека §2a явно выносит это за скоуп.
- **Поведение на macOS/Windows** (`claude-desktop`, пути `%APPDATA%`, `cp --remove-destination`): нет среды для проверки, только Linux.
- **Не-UTF-8 путь `current_exe()`** (`to_string_lossy` исказит путь в конфиге): на Linux встречается крайне редко, не проверял.
- **`file://host/path` в roots** (хост не отбрасывается): клиенты присылают `file:///…`, реального сценария не нашел.
- **Длина URL при отправке телеметрии без `gh`** (6000 символов кириллицы → ~36 КБ после percent-encoding, выше практических лимитов GitHub): дайджест в основном ASCII. Спека задает «~6k символов», формально соблюдено.
- **XML-экранирование `name`/`summary` в паке** (значения от LLM или агента вставляются в атрибуты без экранирования): это косметика формата, а не граница безопасности. Не оценивал.
- **Точность бенчмарка регрессии** (90.6% против 85.9% на другом файле): сравнение на разных входах. Не перепроверял, принял объяснение ruling'а Task 9.
- **Гонка `set_var` в тестах под `cargo test`** (Ruling Task 8): под nextest безопасно, гейт использует nextest.
