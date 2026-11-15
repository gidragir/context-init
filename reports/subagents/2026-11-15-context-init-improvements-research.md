# Технический отчёт: улучшения упаковки, навигации и скорости синхронизации `context-init`

> **Репозиторий:** `/home/gidragir/projects/context-init`  
> **Область исследования:** `crates/ci-core`, `crates/ci-mcp`, `src/cli`, `.agents/skills/context-init/SKILL.md`  
> **Примечание к методике:** все пути, номера строк, константы, сигнатуры и замеры по кодовой базе верифицированы напрямую; расчётные оценки экономии токенов и ускорения для крупных монорепозиториев (500–5000 файлов, проекты класса `mimir`) явно помечены как *расчётная оценка*.

---

## Исполнительное резюме

Анализ реального цикла работы `context-init` (включая сценарии использования в Kotlin/Gradle-проектах класса `mimir`) выявил две взаимосвязанные группы проблем:

1. **Направление A — Потери токенов и обрывы навигации при упаковке контекста:**
   - Дефолтный формат `PackStyle::Xml` (`crates/ci-core/src/analysis/pack.rs:11-26`, `crates/ci-mcp/src/server.rs:474`, `src/cli/pack.rs:37`) оборачивает каждый файл в `<file path="...">\n<![CDATA[\n...\n]]>\n</file>` (~18–25 служебных токенов на файл), дублирует сводки подсистем и не удаляет импорты, комментарии/docstrings, `#[cfg(test)]` и приватные символы. По замерам из `reports/benchmark_latest.json` один только AST-сжатый файл `crates/ci-core/src/analysis/detector.rs` занимает `3275` токенов (`11 193` байт) — 81.9% лимита одной MCP-страницы (`DEFAULT_MAX_RESPONSE_TOKENS = 4000`, `crates/ci-core/src/config.rs:17`).
   - Отсутствуют точечная фильтрация файлов/символов (`files`, `symbols`), компактный индекс уровня L1.5 (`mode="toc"` / `--toc`), бюджет stdout в CLI (`--max-tokens` / `--max-lines`) и информативный футер пагинации (`cursor_calls = 0`).

2. **Направление B — Избыточный I/O, N+1 Git-подпроцессы и повторный BPE-пересчёт при синхронизации:**
   - Один и тот же исходный файл при `init`/`update`/`check` читается с диска до **4–5 раз**, а каждая L1-карточка в `run_check` / `update_with_options` — **2 раза**.
   - `generator::update_with_options` (`crates/ci-core/src/analysis/generator.rs:561-728`) порождает $O(N)$ подпроцессов `git status` и `git rev-parse` (через `is_git_stale` и `git_source_shas`), игнорируя батч-запросы (`git ls-tree`).
   - Поле `SubsystemInfo::tokens` помечено `#[serde(skip)]` (`crates/ci-core/src/storage.rs:295-297`): результаты дорогой однопоточной токенизации `tiktoken-rs` (`cl100k_base`) при `init`/`update` выбрасываются и пересчитываются заново при `context_get_subsystems` (при активной телеметрии — **дважды** в одном вызове!) и `context_pack`.

### Сводная таблица приоритетов

| ID | Направление | Ключевое изменение | Расчётный эффект | Сложность |
| :--- | :--- | :--- | :--- | :---: |
| **A.1** | Компактный вывод и зачистка шума | `PackStyle::Compact` по умолчанию, `strip_imports`, `exclude_tests`, `public_only`, однострочное `{ ⋮ }` | **-35%…-65%** токенов к текущему `--compress` | Средняя |
| **A.2** | Точечная выборка с телами | `files` (globset) + `symbols` + `lines` в `context_pack` / CLI с сохранением `safe_join` + `is_secret_path` | **1 вызов** вместо 5–15 `read_file` | Низкая |
| **A.3** | Необрезаемый индекс L1.5 (`toc`) | `mode="toc"` / `--toc`: `file (L): sym[Lstart-Lend]` по 1 строке | **~300–600 tok** (всегда 1 страница, `<100` строк) | Средняя |
| **A.4** | Бюджет CLI stdout | `--max-lines` / `--max-tokens` + лестница деградации `full -> compact -> toc -> paginate` | **0** повреждённых выводов при элизии `rtk` | Низкая |
| **A.5** | Пагинация UX и инструкции | `page X/Y`, `remaining_files`, готовый hint вызова в футере; `ServerConfig.instructions`; обновление `AGENTS.md`/`SKILL.md`/`rule_template.md` | Устранение `cursor_calls=0` и лишних чтений lazy-схем | Низкая |
| **B.1** | Батчинг Git (`git ls-tree`) | 1 `RepoGitSnapshot` + 1 `git ls-tree -r -z HEAD` вместо $O(N)$ форков `git status`/`rev-parse` | **-80%…-95%** времени на Git-проверках | Средняя |
| **B.2** | Единый проход по ФС + кэш файлов | Передача `&[String]` из `ScopeFilter` в `discover_subsystems`; `AnalyzedFileCache` + 1 проход `scan` | **-60%…-80%** syscalls (`open`/`read`/`stat`) | Средняя |
| **B.3** | Устранение лишнего BPE + `rayon` | Персистентный кэш метрик по `(len, mtime)` / `tokens` в `L0_index.json`; O(1) срез `CoreBPE` в `pagination.rs`; `rayon` | **3x–10x** ускорение `init`/`update`/`check` и CLI | Средняя |

---

## Направление A: Пять улучшений упаковки и навигации

### A.1. Компактный вывод (`style="compact"` по умолчанию) и минимизация синтаксического шума

#### Текущее состояние (observed)
1. **Дефолт XML и тяжёлая обёртка:**
   - `PackStyle` (`crates/ci-core/src/analysis/pack.rs:11-26`) имеет только два варианта: `Xml` (`#[default]`) и `Markdown`.
   - Значение по умолчанию `"xml"` жёстко задано в `crates/ci-mcp/src/server.rs:474` (`p.style.as_deref().unwrap_or("xml")`) и в `src/cli/pack.rs:37` (`#[arg(long, default_value = "xml")]`).
   - Ветка XML (`pack.rs:171-196`) добавляет XML-пролог (`<?xml version="1.0" encoding="UTF-8"?>`), корневой тег `<context_pack>`, пересказ `<subsystem id="..." name="..."><summary>...</summary></subsystem>`, а на каждый файл — `    <file path="{}">\n<![CDATA[\n...\n]]>\n    </file>\n`.
   - Ветка Markdown (`pack.rs:149-168`) также тратит токены на шапку `# 📦 AI Context Pack: ...` + `> Сгенерировано context-init (встроенный AST-пакер)` и заголовки `## File: `{}`\n```{}\n...\n```\n\n`.
2. **Недостаточная глубина сжатия по языкам (`LanguageAdapter::compress_source`):**
   - Трейт `LanguageAdapter` (`crates/ci-core/src/analysis/lang/mod.rs:22-32`) имеет только метод `compress_source(&self, source: &str) -> String` без флагов фильтрации. `ELIDED_BODY = "⋮----"` (`lang/mod.rs:19`).
   - **Rust** (`lang/rust.rs:21` -> `pack::compress_rust_ast` `pack.rs:211-415`):
     - В отличие от `extract_rust_symbols` (`crates/ci-core/src/analysis/detector.rs:1892, 2039, 2062`), который уже фильтрует `#[cfg(test)]` через `has_cfg_test` и приватные элементы через `is_syn_pub`, `format_rust_items` (`pack.rs:225-415`) сохраняет **все** `#![...]` (`parsed.attrs`), **все** `syn::Item::Use`, **все** приватные функции/структуры и **весь** `#[cfg(test)] mod tests` (что зафиксировано тестами `pack.rs:490` и `pack.rs:545`).
     - Каждая сжатая функция занимает минимум 3 строки (`{\n    ⋮----\n}`), а между элементами вставляется `"\n\n"` (`pack.rs:261, 412`).
     - `normalize_rust_tokens` (`pack.rs:418-454`) оставляет избыточные пробелы после `quote!`.
   - **Kotlin** (`crates/ci-core/src/analysis/lang/kotlin.rs:79-208`):
     - Оставляет `package ...`, все блоки `import ...`, все комментарии/KDoc, все пустые строки вне тел функций, все `private`-объявления и тела свойств внутри классов. Метод `method_bodies` (`kotlin.rs:210-212`) возвращает пустой `Vec::new()`.
   - **BraceLang (TS/JS/Java/C#/Go)** (`crates/ci-core/src/analysis/lang/brace.rs:25-141`):
     - Оставляет `import`/`package`, комментарии, полностью блоки `interface`/`type`, сжимая тела на глубине фигурных скобок `depth == 1 || depth == 2`. При этом `extract_symbols` (`brace.rs:21-23`) ошибочно хардкодит `"ts"` даже для `go`, `java` и `cs`! Метод `method_bodies` (`brace.rs:143-145`) также пустой.
   - **Python** (`crates/ci-core/src/analysis/lang/python.rs:20-98`):
     - Оставляет `import` / `from ... import`, комментарии, декораторы и **полные многострочные docstrings** внутри `def`/`async def` перед `    ⋮----`.
   - **BSL** (`crates/ci-core/src/analysis/lang/bsl.rs:165-202`):
     - Сохраняет все неэкспортные процедуры и функции (тела заменяет на `    ⋮----`).

#### Замеры по файлам `context-init` (прямой подсчёт)

| Файл (`context-init`) | Всего строк | Prod строк | Тестовых строк (`mod tests`) | `use`/импорты | `pub`-объявлений / прив. `fn` | Проблема при текущем `--compress` |
| :--- | ---: | ---: | ---: | ---: | :---: | :--- |
| `analysis/generator.rs` | 2220 | 1141 | **1079 (48.6%)** | 13 | 22 / 23 | Почти половина файла — тесты, включаемые в pack |
| `analysis/pack.rs` | 829 | 475 | **354 (42.7%)** | 3 | 10 / 1 | 354 строки тестов сжимаются в скелет, но не убираются |
| `analysis/detector.rs` | 2509 | 2509 | 0 (вынесены в `tests.rs`) | 15 | 12 / 37 | 37 приватных `fn` и 15 `use` дают `3275` ток. (`benchmark_latest.json`) |
| `ci-mcp/src/pagination.rs` | 542 | 372 | **170 (31.4%)** | 3 | 17 / — | Треть файла — unit-тесты с фикстурами |
| `lang/kotlin.rs` | 511 | 407 | **104 (20.4%)** | 6 | 1 / — | Плюс `package`/`import` и KDoc на каждом файле |

#### Проектируемое решение (Rust API)

```rust
// crates/ci-core/src/analysis/pack.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackStyle {
    #[default]
    Compact,   // Новый дефолт: минимальные заголовки, без CDATA/баннеров
    Markdown,
    Xml,       // Сохраняется для обратной совместимости по явному запросу
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackMode {
    Full,       // compress = false (полные тела)
    #[default]
    Compressed, // compress = true (сигнатуры и структуры)
    Toc,        // Уровень L1.5: однострочный индекс символов с номерами строк
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressOptions {
    pub strip_imports: bool,   // Схлопывать import/use/package в `// [N imports omitted]`
    pub public_only: bool,     // Оставлять только pub/export/public API
    pub exclude_tests: bool,   // Вырезать #[cfg(test)] и test-модули/файлы (кроме явного запроса)
    pub strip_comments: bool,  // Убирать обычные комментарии и обрезать docstrings > 1 строки
    pub inline_elide: bool,    // Однострочная элизия `{ ⋮ }` вместо 3-строчного блока
}

impl CompressOptions {
    pub fn for_style(style: PackStyle) -> Self {
        match style {
            PackStyle::Compact => Self {
                strip_imports: true,
                public_only: false,
                exclude_tests: true,
                strip_comments: true,
                inline_elide: true,
            },
            PackStyle::Markdown | PackStyle::Xml => Self::legacy(),
        }
    }
}
```

В `crates/ci-core/src/analysis/lang/mod.rs`:
```rust
pub trait LanguageAdapter: Send + Sync {
    fn language_id(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn extract_symbols(&self, source: &str) -> Vec<String>;
    fn compress_source(&self, source: &str) -> String;
    /// Расширенное сжатие с учётом опций; по умолчанию делегирует в `compress_source`.
    fn compress_with(&self, source: &str, _opts: &CompressOptions) -> String {
        self.compress_source(source)
    }
    fn method_bodies(&self, source: &str) -> Vec<(String, String)>;
    /// Для L1.5 TOC и точечного drill-down по символам.
    fn extract_symbol_spans(&self, source: &str) -> Vec<SymbolSpan> { Vec::new() }
}
```

**Правила в адаптерах:**
- **`PackStyle::Compact` формат файла** (вместо `<file path="...">\n<![CDATA[...]]>\n</file>`):
  ```text
  === crates/ci-core/src/analysis/pack.rs (829L) ===
  // [3 use/imports omitted]
  pub enum PackStyle { Compact, Markdown, Xml }
  pub fn pack_subsystem(...) -> Result<PackedSubsystems> { ⋮ }
  ```
- **Rust (`compress_rust_ast_with`):** пропуск `Item::Use` (подсчёт числа `use` для 1-строчной сводки при `strip_imports`), пропуск `#[cfg(test)]` через существующий `has_cfg_test` (`detector.rs:2039`) при `exclude_tests`, фильтрация по `is_syn_pub` (`detector.rs:2062`) при `public_only`, однострочная заглушка `{ ⋮ }` вместо `{\n    ⋮----\n}` (экономит 2 `\n` и 4–8 пробелов на каждую функцию/метод) и одиночный `\n` между элементами в `impl`.
- **Kotlin (`KotlinAdapter::compress_with`):** схлопывание непрерывных преамбул `package` + `import` в `// [N imports omitted]`; при `public_only` — пропуск объявлений и блоков с модификаторами `private` / `internal` (по желанию флагом); при `strip_comments` — пропуск `//` и многострочных `/* ... */` (сохраняя 1-ю строку KDoc `/** ... */` только при `!strip_comments`).
- **BraceLang (`BraceLangAdapter`):** исправление бага `brace.rs:22` (передавать `self.ext` или реальное расширение файла вместо hardcoded `"ts"`!); схлопывание `import ... from ...` / `package` / `using`; однострочное `{ ⋮ }`.
- **Python (`PythonAdapter`):** схлопывание блока `import` / `from ... import`; при `strip_comments` не печатать многострочные docstrings внутри `def`/`class` (оставлять 1-ю строку или опускать полностью); при `public_only` пропускать `def _...` / `async def _...`.

#### Расчётная оценка экономии токенов (на подсистему из ~10 файлов)

| Конфигурация | Обёртка (10 файлов) | Шум (`use`/`import`, тесты, прив., `⋮----`) | Суммарно токенов *(оценка)* | Экономия к текущему `xml + compress` |
| :--- | ---: | ---: | ---: | ---: |
| Текущий `style="xml"`, `compress=true` | ~220–280 tok | 100% (`use`, `#[cfg(test)]`, 3-строчный `⋮----`, комменты) | ~2 800–4 500 tok | Базовый (1.0x) |
| `style="markdown"`, `compress=true` | ~120–160 tok | 100% (то же тело AST) | ~2 680–4 360 tok | ~4%–6% |
| `style="compact"`, `strip_imports=true`, `exclude_tests=true` | ~50–70 tok | Убраны `CDATA`, импорты, тесты, `strip_comments`, `{ ⋮ }` | **~1 400–2 200 tok** | **~45%–55%** *(оценка)* |
| `style="compact"`, `+ public_only=true` | ~50–70 tok | То же + скрыты `private`/`internal` методы | **~900–1 500 tok** | **~60%–70%** *(оценка)* |

---

### A.2. Точечная выборка с телами (`files` / `filter` + `compress=false` и `symbols`)

#### Текущее состояние (observed)
- `McpPackParams` (`crates/ci-mcp/src/tools.rs:90-108`) содержит только: `subsystem_id`, `path`, `project`, `compress`, `style`, `cursor`.
- `PackArgs` (`src/cli/pack.rs:18-39`) содержит только: `subsystem_id`, `path`, `compress`, `output`, `style`.
- `pack_subsystem` (`pack.rs:42-121`) безусловно собирает все `sub.files` + `sub.key_files` целиком (`pack.rs:95-106`).
- В результате запрос полных тел (`compress=false`) в модуле на 10–30 файлов переполняет `DEFAULT_MAX_RESPONSE_TOKENS = 4000`, и агент бросает MCP в пользу серии `read_file`.
- При этом механизмы безопасности (`safe_join` `storage.rs:347`, `is_secret_path` `pack.rs:124-136`), сопоставление глобов (`globset = "0.4"` уже в `crates/ci-core/Cargo.toml:24`) и извлечение тел отдельных методов (`LanguageAdapter::method_bodies`, `task.rs:45-116`) в проекте уже существуют.

#### Проектируемое решение (Rust API и CLI/MCP)

```rust
// crates/ci-core/src/analysis/pack.rs
#[derive(Debug, Clone, Default)]
pub struct PackRequest<'a> {
    pub subsystem_id: &'a str,
    pub mode: PackMode,
    pub style: PackStyle,
    pub compress_opts: CompressOptions,
    /// Список относительных путей или glob-масок (напр. ["pack.rs", "**/handlers/*.kt"]).
    pub files: &'a [String],
    /// Фильтр имён символов/методов для извлечения полных тел конкретных функций/классов.
    pub symbols: &'a [String],
}
```

В `McpPackParams` (`crates/ci-mcp/src/tools.rs:90`):
```rust
#[serde(default)]
pub files: Option<Vec<String>>,
#[serde(default)]
pub symbols: Option<Vec<String>>,
#[serde(default)]
pub mode: Option<String>, // "full" | "compressed" | "toc"
```
В `PackArgs` (`src/cli/pack.rs:18`):
- `--files <GLOB_OR_PATH>` (repeatable / comma-separated) и алиас `--filter <PATTERN>`
- `--symbols <NAME>` (repeatable)
- `--toc` / `--signatures-only` (`PackMode::Toc`)
- `--public-only`, `--keep-imports`, `--include-tests`

**Порядок сопоставления фильтров и инварианты безопасности:**
1. **Белый список кандидатов до фильтрации:** при `subsystem_id != "all"` кандидатами являются **только** файлы выбранной подсистемы `sub.files ∪ sub.key_files` (`pack.rs:95-106`). При `subsystem_id == "all"` (только с непустым `files`!) кандидатами являются файлы всех подсистем в `L0Index` (запретить голый `GlobSet` по всей ФС в обход `L0Index`/`ScopeFilter`!).
2. **Безопасность путей (Zero-Bypass):**
   - Каждый паттерн в `files` перед компиляцией `globset::GlobSet` проверяется: отклонять паттерны, начинающиеся с `/` или `\` (либо с Windows drive prefix) и содержащие компонент `..`.
   - Каждый совпавший файл-кандидат проходит обязательную тройную проверку из `pack.rs:102`: `!is_secret_path(path)` + `safe_join(root, path)` + `.canonicalize()` внутри канонизированного `root` (защита от symlink escape!).
   - **Расширение `is_secret_path` (`pack.rs:124-136`):** помимо `.env`, `mise.local.toml`, `.key`, `.token`, `.pem`, `.db`, `.sqlite`, `.sqlite3` добавить `*.p12`, `*.pfx`, `*.jks`, `*.keystore`, `id_rsa*`, `id_ed25519*`, `*secret*`, `*credential*`, `google-services.json`, чтобы фильтр `**/*.json` не раскрыл секреты.
3. **Умное разрешение коротких имён:** если в `files` передано `"pack.rs"` (без пути от корня), матчить как по полному `rel_path`, так и по suffix `/pack.rs` или относительно `sub.directory` (если совпало ровно в этой подсистеме). Если фильтр вернул `0` совпадений — возвращать явную ошибку со списком доступных файлов подсистемы (а не молчаливый пустой output или fallback на L1-карточку!).
4. **Извлечение `symbols`:** при непустом `symbols` для Rust/Python/BSL использовать `method_bodies` / AST-поиск (а для Kotlin и BraceLang — реализовать извлечение диапазонов методов в `LanguageAdapter::extract_symbol_spans` и вырезать точный срез строк с телом без чтения остального файла в ответ). При `files.is_some()` даже `PACK_ALL_MAX_PAGES` (`handlers.rs:571`) проверяется **после** фильтрации.

---

### A.3. Необрезаемый индекс уровня L1.5 (`mode="toc"` / `--toc` / `--signatures-only`)

#### Сравнение: почему L1 и `pack --compress` не закрывают нишу L1.5

| Свойство | Текущий L1 (`context_get_subsystem` / `generator.rs:905-964`) | Предлагаемый L1.5 (`mode="toc"` / `--toc`) | Текущий `pack --compress` (`pack.rs:204`) |
| :--- | :--- | :--- | :--- |
| Файлы подсистемы | Таблица `Основные файлы` (`representative_files` — макс. **15** файлов, `detector.rs:1193`), только имя и число строк | **Все** не-тестовые (или отмеченные) файлы подсистемы + число строк | Код всех файлов подсистемы целиком |
| Символы и сигнатуры | Плоский блок `key_symbols`: **макс. 10 (`deep=false`) / 30 (`deep=true`)** из первых `5`/`15` файлов (`detector.rs:1751-1756`) | Все публичные (или все top-level при `public_only=false`) символы с группировкой по файлам | Все AST-декларации, включая `use`, `private`, тесты, комментарии |
| Номера строк символов | **Отсутствуют** | **Да (`[L42-121]`)** — точная привязка к строкам для `read_file` или `files`+`symbols` | Отсутствуют (строки сдвинуты из-за элизии!) |
| Размер ответа | ~200–500 tok | **~300–600 tok (~25–65 строк)** | ~1 500–6 000+ tok (часто режется!) |
| Риск усечения MCP / CLI | Почти нулевой | **Почти нулевой** (<4000 tok и <100 строк `run_command`) | Высокий (пагинация + середина режется `rtk`) |

#### Проектируемое решение для L1.5 TOC

Формат ответа для `mode="toc"` (1–2 строки на файл, 1 строка на крупный класс с перечислением методов, либо 1 строка на публичный символ):
```text
[TOC] crates_ci-core_analysis_pack (1 file, 829L | ~210 tok)
pack.rs (829L):
  enum PackStyle [L11-15] | struct PackedSubsystems [L30-35]
  fn pack_subsystem(root, knowledge_dir, index, id, compress, style) -> Result<PackedSubsystems> [L42-121]
  fn pack_files(root, subs, files, compress, style) -> String [L140-198]
  fn compress_source(content, ext) -> String [L204-209]
  fn compress_rust_ast(content) -> Option<String> [L211-223]
```

**Техническая реализация `SymbolSpan`:**
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolSpan {
    pub kind: &'static str,   // "fn" | "struct" | "class" | "interface" | "trait" | "enum"
    pub signature: String,    // Сжатая до <= 120 символов однострочная сигнатура без тела
    pub is_public: bool,
    pub line_start: usize,    // 1-based
    pub line_end: usize,      // 1-based
}
```
- **Rust:** добавить feature `"span-locations"` к `syn` (в `Cargo.toml`) либо строить однопроходную таблицу byte-offset переносов строк (`line_starts`) + `Spanned::span()` через `proc_macro2`. Это даст точные `[Lstart-Lend]` за один разбор `syn::parse_file` без отдельного прохода!
- **Kotlin / BraceLang / Python / BSL:** сканеры в `lang/{kotlin,brace,python,bsl}.rs` уже итерируют построчно (`lines().enumerate()`), поэтому сбор `(line_start, line_end)` при подсчёте скобок `{}` или отступов (Python) / `КонецПроцедуры|EndFunction` (BSL) тривиален и имеет линейную сложность $O(\text{bytes})$.
- Если суммарный TOC слишком большой подсистемы (например, 80 файлов) превышает бюджет, автоматическое сжатие имён параметров в `(...)` (через уже существующую логику `clean_multiline_signature` `detector.rs:2267-2282`) гарантирует удержание ответа в 1 странице и `< 100` строках.

---

### A.4. Бюджет вывода в CLI (`--max-tokens` / `--max-lines`)

#### Текущее состояние (observed)
- `src/cli/pack.rs:106-110`:
  ```rust
  if let Some(out) = &args.output {
      std::fs::write(out, &packed.content)?;
  } else {
      std::io::stdout().write_all(packed.content.as_bytes())?;
  }
  ```
- При выводе в `stdout` полностью игнорируются: `max_response_tokens` из `Config`, флаг `--compact` из `GlobalArgs` (`src/cli/mod.rs:70`), а также жёсткое ограничение терминальных обёрток вроде `run_command` / `rtk` (~100 строк stdout, свыше которых вырезается середина).
- В `PackArgs` (`src/cli/pack.rs:18-39`) нет параметров `--max-tokens`, `--max-lines` и `--cursor`, хотя на уровне ядра уже есть `ci_mcp::pagination::paginate_content` (или стоит вынести `pagination` в `ci-core`, чтобы CLI использовал его без зависимости на MCP!).

#### Проектируемое решение в `src/cli/pack.rs`

Добавить флаги в `PackArgs`:
- `--max-lines <usize>` (при неинтерактивном выводе `!std::io::stdout().is_terminal()` или при `--compact` разумный дефолт или явное указание в инструкциях агентам `--max-lines 90`)
- `--max-tokens <usize>` (по умолчанию `None` при записи в `-o file`, либо `config.max_response_tokens` при stdout для агентов / `--budget`)
- `--cursor <String>` (для добора последующих страниц в CLI-сессиях субагентов без MCP)

**Детерминированная лестница авто-деградации (вместо слепого обрезания середины):**
1. Замер: если задан `--output`, пишем полный результат (если не заданы явные `--max-*`). Если вывод в `stdout` и задан `--max-lines L` / `--max-tokens T`:
2. **Уровень 0 (запрошенный режим):** если укладывается в `lines <= L` и `tokens <= T` — выводим целиком.
3. **Уровень 1 (авто-сжатие):** если исходно был `--no-compress` или `Xml`/`Markdown` — автоматически переключаем на `PackStyle::Compact` + `CompressOptions::for_style(Compact)` и добавляем 1-строчный префикс `[auto-compressed: full exceeded budget -> compact]`.
4. **Уровень 2 (авто-TOC L1.5):** если сжатый код всё ещё превышает `L` или `T` — автоматически перестраиваем в `PackMode::Toc` с 1-строчной подсказкой `[auto-toc: compressed exceeded 90 lines; drill down via --files <file> --no-compress or --cursor <cur>]`.
5. **Уровень 3 (честная постраничная пагинация):** если даже TOC (или явно запрошенный `--files foo.rs --no-compress`) превышает бюджет — вызываем построчную пагинацию с обрезкой строго на границе строки/символа (сохраняя закрытие code fence) и печатаем в последней строке точный CLI-курсор `--cursor "cur_..."` для добора.

---

### A.5. Улучшение UX пагинации (`next_cursor`), MCP-инструкций, `AGENTS.md` и скиллов

#### Почему субагенты игнорируют `next_cursor` (`cursor_calls = 0`) — анализ `pagination.rs`
1. **Пассивный футер без прогресса и контекста (`crates/ci-mcp/src/pagination.rs:117-133`):**
   - Сейчас `format_footer` выдаёт лишь:
     `[truncated: true | next_cursor: "cur_..." | stale: false | tokens: 3950]`
   - В `CursorData` (`pagination.rs:10-16`) хранятся только `{ byte_offset, page }`. Агент не видит: (а) какую долю ответа он получил (`page 1/3`, `3950 / 11400 tok`), (б) **какие именно файлы** остались на следующих страницах (стоит ли вообще тратить ещё один ход!), (в) что конкретно нужно передать дальше.
2. **Нет подсказок на уровне MCP-сервера (`crates/ci-mcp/src/server.rs:694-703`):**
   - `ServerHandler::get_info(&self)` возвращает `rmcp::model::ServerConfig` без установки поля `instructions`.
   - При этом в хост-харнессах (напр. Antigravity) все 12 инструментов `context-init` загружаются в режиме **lazy-loaded MCP**: агент сначала видит только имена инструментов и вынужден тратить лишние tool-вызовы на чтение JSON-схем `<appDataDir>/mcp/context-init/*.json`, иначе он ошибается с обязательностью `"path"` (без `"path"` сервер ищет базу в CWD процесса `/home/gidragir` и падает/промахивается) и не знает про `"style": "markdown"` (или `"compact"`) и `"cursor"`.
3. **Устаревшие шаблоны `AGENTS.md`, `RULE_TEMPLATE` и `SKILL.md`:**
   - `crates/ci-core/src/analysis/generator.rs:1058-1065` (`render_root_agents_md`): шаг 3 гласит лишь `3. context_pack(subsystem_id, compress=true) — AST-сжатый код подсистемы.`, умалчивая про `path`, `style="compact"`/`"markdown"`, `mode="toc"`, `files` и `cursor`.
   - `src/cli/setup/rule_template.md` (18 строк; тест `src/cli/setup/rules.rs:74-87` требует `<= 25` строк и наличие 4 MCP-имён, `"Subagents and scripts"`, `"context-init pack <subsystem_id> --compress"`): не содержит готовых JSON-сигнатур для lazy MCP, не упоминает `style`, `toc`, `files` и добор по `cursor`.
   - `.agents/skills/context-init/SKILL.md` (строки 40, 54, 91, 94) и `src/cli/setup/rules.rs:29-35` (`skill_file`) прямо предлагают `context-init pack ... -o /tmp/context.xml` и перечисляют `style`: `"xml" | "markdown"` без приоритета компактного формата и без протокола работы с `next_cursor`.

#### Проектируемые улучшения

**1. Новый `CursorData` и `format_footer` (`crates/ci-mcp/src/pagination.rs`):**
- Передавать опциональные метаданные оставшихся секций/файлов (в `pack` уже известен порядок файлов; легко собрать список `## File:` / `=== file:` заголовков за `byte_offset`):
  ```text
  [truncated: true | page: 1/3 (~3950/11200 tok) | remaining: lang/brace.rs, lang/kotlin.rs (+2) | next_cursor: "cur_..." | next: call context_pack(..., cursor="cur_...") or narrow with files=[...] | stale: false]
  ```
- Добавить краткие `instructions` в `ContextInitServer::get_info` (`server.rs:695`):
  - Всегда передавать `path="<repo_root>"`;
  - Рабочий цикл: `context_get_subsystems` -> `context_pack(mode="toc")` -> `context_pack(files=[...], compress=false)`;
  - При `truncated: true` — добирать через `cursor` либо сужать `files=[...]`.

**2. Компактный блок правил для устранения оверхеда lazy MCP (`rule_template.md`, 22 строки ≤ 25):**
```markdown
## context-init Knowledge Base (L0-L2)

Always use `context-init` before manual recursive file reads. Pass explicit `"path": "<project_root>"` in MCP calls (schemas already known — skip reading MCP `.json` schema files):
1. `context_get_subsystems` `{"path": "<root>"}` — L0 map (~200-500 tok).
2. `context_get_subsystem` `{"path": "<root>", "subsystem_id": "<id>"}` — L1 contract.
3. `context_pack` `{"path": "<root>", "subsystem_id": "<id>", "mode": "toc", "style": "compact"}` — L1.5 line index (never truncates).
4. `context_pack` `{"path": "<root>", "subsystem_id": "<id>", "files": ["file.rs"], "compress": false}` — exact bodies; for full skeleton use `"compress": true, "style": "compact"`.
5. `context_search` `{"path": "<root>", "query": "..."}` — when module is unknown.
6. `context_check` `{"path": "<root>"}` — drift audit before finishing.

Pagination: if `[truncated: true | next_cursor: "..."]` appears, either call again with `"cursor": "<next_cursor>"` or narrow via `"files": [...]`.
Subagents and scripts (no MCP):
- `{rtk}context-init search "<query>" --compact` then `{rtk}context-init pack <subsystem_id> --compress --style compact --max-lines 90` (or `--toc` first, `--files <f> --no-compress` for bodies). Never dump raw XML to stdout.
```
*Проверка совместимости с тестом `src/cli/setup/rules.rs:74-93`:* строка `<= 25` соблюдена, все 4 требуемых имени MCP-инструментов присутствуют, `"Subagents and scripts"`, `"context-init pack <subsystem_id> --compress"` и `"{rtk}context-init search"` сохранены. Аналогично синхронизируются `render_root_agents_md` (`generator.rs:1058-1065`), `skill_file` (`rules.rs:29-35`) и `.agents/skills/context-init/SKILL.md` (удаление `-o /tmp/context.xml`, рекомендация `compact`/`markdown` и `cursor`/`--max-lines`).

---

## Направление B: Глубокий анализ скорости синхронизации базы знаний

### B.1. Инвентаризация узких мест по модулям и цепочкам вызовов

#### 1. Пайплайн оркестрации (`crates/ci-mcp/src/pipeline.rs`)
- **`run_check` (`pipeline.rs:140-191`):**
  1. Загружает `L0_index.json` (`L141`).
  2. Вызывает `detector::analyze_project_with(deep=false)` (`L146`) — полный проход по дереву и парсинг символов.
  3. Вызывает `generator::audit` (`L147`, `generator.rs:416-511`), который **повторно** загружает `L0_index.json` с диска (`generator.rs:425`), **первый раз** читает каждую L1-карточку (`L439`) и вызывает `.exists()` для файлов.
  4. Строит `repo_snapshot` (`L162`: `head_commit` + `git status --porcelain=v1 -z`).
  5. В цикле по всем подсистемам (`L165-179`) вызывает `check_subsystem_staleness_with_snapshot`, который **второй раз** читает каждую L1-карточку с диска, повторно парсит frontmatter `OkfCard` и порождает **отдельный** дочерний процесс `git rev-parse` (`blob_sha`) на каждую подсистему.
- **`run_update` (`pipeline.rs:95-138`):**
  - Вызывает `analyze` (`L276`) -> `detect` -> `build_deterministic` (с полным BPE-пересчётом) -> `generator::update_with_options` (ещё $O(N)$ `git status` и `git rev-parse`) -> `apply_enrichment`.
- **`refresh_stale_card` (`pipeline.rs:209-219`):**
  - Вызывается из `ContextHandler::get_subsystem` (`handlers.rs:468-474`) при `auto_refresh=true`, когда устарела **одна** подсистема. Вместо точечного обновления одного модуля запускает **полный** `detect(project, false)` + `build_deterministic` (пересчитывая все подсистемы проекта!) + `generator::update` + `restamp_card_source`. После этого `get_subsystem` **ещё раз** вызывает `check_subsystem_staleness` (`L472`), повторно читает файл карточки (`L478`) и пересчитывает `live_source_tokens` (`L527`).

#### 2. Двойной обход и I/O в `scan` (`crates/ci-core/src/analysis/scan.rs`)
- **`scan_project_with` (`scan.rs:140-355`):**
  - Строит `ScopeFilter` **дважды**: сперва `ScopeFilter::build_diagnostic` (`L146`), затем `ScopeFilter::build` (`L241`) — оба запускают независимый `ignore::WalkBuilder` по всему дереву.
  - На каждый файл выполняется: `fs::metadata` (`L165`) -> `is_likely_binary` (`L22-52`, **отдельный `File::open` и чтение 8 КБ**) -> `fs::read` до `TOKENIZE_MAX_BYTES = 512 * 1024` (`L106, L174`) -> однопоточный `count_tokens` (`tiktoken-rs cl100k_base`).

#### 3. Потеря работы и избыточные чтения в `detector.rs` и `subsystems.rs`
- **`ScopeFilter` игнорируется (`detector.rs:163-188, 822`):**
  - `analyze_project_with` (`L163`) уже обходит весь репозиторий внутри `ScopeFilter::build`, заполняя `scope.files()` (`scope.rs:262`). Но `discover_subsystems` (`L822`) использует `scope.files()` только для 1С `mdo` (`L188`), а во всех остальных проектах **снова** рекурсивно вызывает `fs::read_dir` в `discover_subsystems`, `contains_relevant_files_depth` (`L1155`) и `collect_files_recursive_excluding` (`L1637`).
- **Последовательное множественное чтение одного файла:**
  1. `subsystem_file` (`detector.rs:1701-1710`) вызывает `count_lines(path)` (`L1737-1741`) -> `fs::read_to_string(path)`.
  2. `tokens_if_over` (`detector.rs:1214-1254`): сначала суммирует `metadata().len()`, при превышении `threshold` читает все файлы директории для `estimate_tokens_fast` (`L1225-1235`), и при превышении оценки передаёт их в `tokenize_files` (`L1256-1270`), который через `FileMemo` снова делает `stat` + чтение + `count_tokens`.
  3. `extract_symbols_from_single_file` (`detector.rs:1801-1824`): содержит **собственный независимый** `static MEMO: FileMemo<Vec<String>>`, не разделяемый с `tokenize_files` (в холодном CLI-процессе файл читается повторно!) и выполняет поиск файла через `ancestors().find_map(|p| p.join(&f.path).exists())` (`L1771-1780`) + $O(K^2)$ `symbols.contains(&s)` (`L1785`).
  4. **`subsystems::build_deterministic` (`crates/ci-core/src/analysis/subsystems.rs:96-98, 130-141`):** по завершении детектора проходит по **всем** подсистемам и последовательно вызывает `subsystem_tokens(&analysis.root_dir, sub)`, который **в очередной раз читает с диска и токенизирует через BPE все файлы каждой подсистемы**, игнорируя данные из `detector.rs`.
  5. **`#[serde(skip)]` на `SubsystemInfo::tokens` (`crates/ci-core/src/storage.rs:295-297`) и `SubsystemFile::lines` (`L254`):** все вычисленные токены и числа строк **не сохраняются** в `L0_index.json` и выбрасываются по завершении команды!

#### 4. Повторный BPE в `handlers.rs` и `pagination.rs`
- `ContextHandler::get_subsystems` (`crates/ci-mcp/src/handlers.rs:357-426`):
  - Для каждой подсистемы вызывает `subsystem_tokens` (`L376`, дисковое чтение + BPE всех файлов).
  - Если включена телеметрия (`tel.is_active()`, `L412-415`), вызывает `live_source_tokens` (`L216-231`), который **второй раз** в том же запросе читает все файлы всех подсистем и гонит BPE!
- `ContextHandler::pack` (`handlers.rs:541-616`):
  - Читает файлы в `pack_subsystem`, затем на `L564` вызывает `live_source_tokens` (**повторное чтение с диска + BPE** исходных файлов), при `"all"` вызывает `count_tokens(&packed_content)`, а затем на `L591` строит `repo_snapshot` и проверяет staleness по каждой подсистеме.
- `paginate_content` (`crates/ci-mcp/src/pagination.rs:250-307`):
  - На каждой итерации цикла `L250-294` вызывает `find_split_point` (который в бинарном поиске `~10–15` раз вызывает `count_tokens(&text[..mid])`!), плюс ещё `2` вызова `count_tokens(&rendered)` в теле цикла (`10–30` полных BPE-кодирований одного и того же текста на один ответ).

#### 5. N+1 Git-подпроцессы и повторные чтения карточек (`generator.rs` + `git.rs`)
- В `git.rs` только `head_commit` (`L217, L354`) и `remote_url` (`L266`) читаются in-process. Методы `blob_sha` (`L414-467`: `git hash-object` или `git rev-parse <head>:<dir>`), `diff_name_only` (`L469-515`: `git diff --name-only -z`) и `repo_snapshot` (`L95-103`: `git status --porcelain=v1 -z`) порождают дочерние процессы `git`.
- В `generator::update_with_options` (`crates/ci-core/src/analysis/generator.rs:561-728`):
  1. `card_is_pristine` (`L614`) читает карточку с диска (#1).
  2. Для `owned` карточек (`L676-685`) `card_content` (`L150-163`) вызывает `render_card_for` -> `git_source_shas` (`L76-81`), создающий `SystemGitBackend::new()` и вызывающий `blob_sha` (`git rev-parse HEAD:<dir>`) **для каждой карточки**.
  3. `card_content` читает ту же карточку с диска (#2); если `same_card` == `true`, вызывает `is_git_stale(disk, root)` (`L244-251`).
  4. **`is_git_stale` создаёт новый `SystemGitBackend::new()` и вызывает `check_card_staleness` -> `repo_snapshot` (`git status --porcelain=v1 -z`) + `blob_sha` + `diff_name_only` ДЛЯ КАЖДОЙ НЕИЗМЕНЁННОЙ КАРТОЧКИ!** Для $N$ подсистем: $N$ запусков `git status`, $2N$ запусков `git rev-parse`, до $N$ запусков `git diff`.
  5. `render_l0_json(&index)` вызывается дважды подряд (`L696` и `L708`), а `write_if_changed` (`L29-40`) снова читает файл перед записью.

---

### B.2. Сводная матрица дублирования работы (за 1 холодный запуск `update` / `check`)

| Ресурс / Операция | Текущее поведение (`init`/`update`/`check`) | После предлагаемых оптимизаций |
| :--- | :--- | :--- |
| Обход дерева (`ignore::WalkBuilder` / `read_dir`) | `2x` в `scan`; `1x` `ScopeFilter::build` + `1x` рекурсивный `read_dir` в `detector` | **1x** единый проход `WalkBuilder` (`ScopeFilter` + опц. диагностика) |
| Чтение каждого исходного файла с диска | **3–5 раз** (`count_lines`, `estimate_tokens_fast`, `FileMemo` BPE, `FileMemo` symbols, `build_deterministic::subsystem_tokens`) | **1 раз** (или **0 раз** при попадании в персистентный кэш по `len`/`mtime`) |
| Чтение каждой L1-карточки в `update` / `check` | **2 раза** (`card_is_pristine` + `card_content`; или `audit` + `check_subsystem_staleness`) | **1 раз** в `CardCache` в памяти |
| Вычисления BPE `cl100k_base` (`count_tokens`) | Однопоточно в `scan`, `build_deterministic`, `get_subsystems` (`2x` при telemetry), `pack` (`live_source_tokens`); результат выкидывается (`#[serde(skip)]`) | `rayon` + кэш по `(len, mtime)` + сохранение `tokens` в `L0_index.json` (с `source_sha`) |
| Подпроцессы `git` при $N$ подсистемах | `update`: $\approx 3N..4N$ (`N` `git status`, `2N` `git rev-parse`, до `N` `git diff`); `check`: $1 + N$ | **2 процесса всего** на весь запуск (`1x git status -z` + `1x git ls-tree -r -z HEAD` + при необходимости 1 `git diff`) |
| BPE при пагинации 1 ответа (`paginate_content`) | `10–30` полных кодирований `cl100k_base` | **1** вызов `encode_with_special_tokens` + срез массива токенов $O(1)$ |

---

### B.3. Конкретные алгоритмические и I/O-оптимизации

#### 1. Батчинг Git (`RepoGitSnapshot` + `git ls-tree`) в `git.rs` и `generator.rs`
Расширить `RepoGitSnapshot` (`crates/ci-core/src/git.rs:35`):
```rust
pub struct RepoGitSnapshot {
    pub head: Option<String>,
    pub dirty_paths: Vec<GitStatusEntry>, // Из 1x `git status --porcelain=v1 -z`
    pub tree_shas: BTreeMap<String, String>, // Из 1x `git ls-tree -r -t -z HEAD` (dir & blob SHA)
}
```
- За 1 вызов `git ls-tree -r -t -z HEAD` (~2–5 мс даже на тысячах файлов) получаем `tree_sha` **всех** директорий и `blob_sha` всех файлов без $2N$ форков `git rev-parse`.
- Для `paths_tree_sha` (`git.rs:105`, подсистемы, заданные списком файлов, а не одной директорией): вычисляем SHA-256 по отсортированным `(path, tree_shas.get(path))` **полностью в памяти**, не порождая процессы!
- В `generator::update_with_options` и `audit`: принимать или единожды строить `&RepoGitSnapshot`, передавая его внутрь `card_content` -> `render_card_for` и `is_git_stale`, полностью удалив создание `SystemGitBackend::new()` внутри цикла по подсистемам.
- Для старых коммитов в dirty/stale-подсистемах при `diff_name_only`: группировать подсистемы с одинаковым `card.source.commit` (в норме это единый `HEAD` прошлого `update`), делая 1 вызов `git diff --name-only -z <prev_head> HEAD` вместо $N$.

#### 2. Единый проход по дереву и `AnalyzedFileCache` (`scope.rs`, `scan.rs`, `detector.rs`)
- **В `scope.rs` / `detector.rs`:** перевести `discover_subsystems` (`detector.rs:822`) на группировку готового среза `scope.files()` (`&[String]`) и метаданных, полученных при одном проходе `ignore::WalkBuilder`, вместо повторных `fs::read_dir` / `contains_relevant_files_depth` / `collect_files_recursive_excluding`. Детерминизм гарантируется сортировкой в `BTreeMap<String, Vec<ScopedFile>>` (как уже сделано в `scope.rs`).
- **В `scan.rs`:** объединить `ScopeFilter::build_diagnostic` и `ScopeFilter::build` в один метод `ScopeFilter::build_with_diagnostic` (1 `WalkBuilder`). Убрать отдельный вызов `is_likely_binary(path)` (2-й `File::open`): открывать файл 1 раз (`File::open`), читать в буфер (первый 8KB-чанк проверять на `NUL` + базовую валидность до дальнейшего чтения до `512 KB`).
- **Единый `AnalyzedFileCache` в памяти и опциональный дисковый кэш:**
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct FileMetricsEntry {
      pub len: u64,
      pub modified_nanos: u128,
      pub blake3_hex: Option<String>, // Для надёжности при coarse mtime на ФС
      pub lines: usize,
      pub bpe_tokens: usize,
      pub symbols: Vec<String>,
  }
  ```
  - В памяти одного запуска (`init`/`update`/`check`) один файл читается **строго 1 раз**: по прочитанному `&str` сразу считаются `lines`, `symbols`, при необходимости `bpe_tokens` (`count_tokens`).
  - Дисковый кэш хранить строго **вне** декларативного дерева KB (либо в `.context-init/.cache/metrics_v1.json` с обязательным добавлением в `.gitignore`, либо в `StoragePaths::cache_dir/<project_hash>/metrics_v1.json`), чтобы никогда не загрязнять коммиты (`L0_index.json` / OKF-карточки) и сохранять скорость при повторных CLI-вызовах (`context-init update` / `check`).

#### 3. Персистирование `tokens` / `lines`, устранение дублей BPE и `rayon`
- Убрать `#[serde(skip)]` с `SubsystemFile::lines` (`storage.rs:254`) и `SubsystemInfo::tokens` (`storage.rs:297`), заменив на `#[serde(default)]` (100% обратная совместимость при чтении старых `L0_index.json` и минимум байт в JSON!).
- В `subsystems::build_deterministic` (`subsystems.rs:96-98`): переиспользовать `tokens` из `ProjectAnalysis` (либо предвычисленной карты `BTreeMap<String, usize>`), полностью убрав повторный `subsystem_tokens`.
- В `handlers::get_subsystems` (`handlers.rs:376, 414`) и `handlers::pack` (`handlers.rs:564`): убрать двойной проход `live_source_tokens`; для неизменённых (non-stale) подсистем брать `sub.tokens` из `L0Index` за $O(1)$ без чтения ФС!
- В `detector::tokens_if_over` (`detector.rs:1214`): для отсечения мелких модулей доверять `estimate_tokens_fast`, вызывая точный `count_tokens` только при попадании в пограничную зону `0.75 * threshold .. 1.35 * threshold`.
- Подключить `rayon` в `crates/ci-core/Cargo.toml`: распараллелить `par_iter()` в `scan_project_with` (чтение + `count_tokens`), `detector` (парсинг `syn`/адаптеров + `count_tokens`) и `generator` (чтение/рендер карточек) с финальной сборкой в `BTreeMap` для сохранения побайтового детерминизма выходных файлов.

#### 4. Точечное обновление в `refresh_stale_card` (`pipeline.rs:209`)
Вместо вызова полного `detect(project, false)` по всему репозиторию при автообновлении одной карточки:
- Запустить анализ только для `sub.directory` (или списка `sub.files` + поиск новых файлов внутри этой директории по `ScopeFilter`), обновить одну L1-карточку и одну запись в `L0_index.json`.

#### 5. $O(1)$ пагинация через прямой срез токенов (`pagination.rs`)
В `crates/ci-mcp/src/pagination.rs`: вместо бинарного поиска с `10–30` вызовами `count_tokens(&rendered)` кодировать остаток текста `rest` **один раз** через `bpe.encode_with_special_tokens(rest)`, взять срез `&tokens[..body_budget.min(tokens.len())]`, декодировать `bpe.decode(...)` и откатить к ближайшему `\n` в пределах 15%. Сложность сокращается в ~10–15 раз при гарантированном попадании в `max_response_tokens`.

---

### B.4. Сводная таблица ожидаемого ускорения (наблюдения и расчётные оценки)

В `reports/benchmark_latest.json` зафиксированы замеры на маленьком горячем репозитории `context-init` (35 подсистем, 400 файлов в `scan`): `drift_metrics.avg_ms = 15.10 ms`, `scan (400 files) = 20.33 ms`, `stale_check_avg_ms = 7.70 ms`. Ниже приведена оценка масштабирования и эффекта оптимизаций (для крупных проектов и холодных CLI-вызовов — *расчётная оценка*):

| Операция | Маленький проект (35 подсистем, ~150 файлов, как `context-init`) | Средний проект (80 подсистем, ~1 000 файлов) *(оценка)* | Крупный монорепо (200 подсистем, ~4 000 файлов, класс `mimir`) *(оценка)* | Главный драйвер ускорения |
| :--- | :--- | :--- | :--- | :--- |
| **`context-init check` / `run_check`** | Факт: `~15.1 ms` -> Целевой: **~4–6 ms** | `~250–500 ms` -> **~25–45 ms** (`~8–12x`) | `~1.2–2.5 s` -> **~70–130 ms** (`~15–20x`) | 1x `git ls-tree` + 1x `WalkBuilder` + `rayon` + 1x чтение L1-карточек |
| **`context-init update` (инкремент., 1–3 файла изменены)** | `~25–45 ms` -> **~6–10 ms** | `~600–1 200 ms` -> **~35–70 ms** (`~15x`) | `~2.5–5.5 s` -> **~100–200 ms** (`~20–25x`) | Удаление $N$ `git status`/`rev-parse` в `is_git_stale` + кэш метрик + `rayon` |
| **`context-init scan`** | Факт (400 ф.): `20.33 ms` -> **~5–8 ms** | `~150–350 ms` -> **~25–50 ms** (`~6x`) | `~0.8–2.0 s` -> **~90–180 ms** (`~8–12x`) | 1x `WalkBuilder`, 1x `open`, `rayon` для `cl100k_base` |
| **`context_get_subsystems`** | Факт: `3.30 ms` (1 подсистема); при 35 с телеметрией: ~2x BPE | `~80–180 ms` -> **~5–10 ms** (`~15x`) | `~300–700 ms` -> **~10–20 ms** (`~30x`) | Хранение `tokens` в `L0_index.json` + 1x `RepoGitSnapshot` |
| **`refresh_stale_card` (`auto_refresh=true` на 1 модуле)** | `~25–40 ms` -> **~5–8 ms** | `~400–800 ms` -> **~15–30 ms** (`~20x`) | `~1.5–3.5 s` -> **~20–40 ms** (`~50x+`) | Инкрементальный анализ только целевой директории |

---

## Рекомендуемая последовательность внедрения

1. **Волна 1 (быстрые победы по токенам и UX, минимальный риск регрессий):**
   - `PackStyle::Compact` + дефолт `"compact"` (или `"markdown"` как переходный шаг) в `server.rs:474` и `src/cli/pack.rs:37`; удаление XML-шапок в CLI/MCP-инструкциях.
   - Фильтр `files` (`globset` + `safe_join` + расширенный `is_secret_path`) в `pack_subsystem`, `McpPackParams` и `PackArgs`.
   - Улучшенный футер пагинации (`page X/Y`, `remaining`, подсказка следующего вызова) + обновление `rule_template.md`, `render_root_agents_md`, `SKILL.md` и `ServerConfig.instructions`.
2. **Волна 2 (глубокое сжатие, L1.5 TOC и CLI бюджет):**
   - `CompressOptions` (`strip_imports`, `exclude_tests`, `public_only`, `strip_comments`, `{ ⋮ }`) в `LanguageAdapter::compress_with` (включая исправление бага `"ts"` в `brace.rs:22`).
   - `PackMode::Toc` (`SymbolSpan` с `[Lstart-Lend]` через `syn` `"span-locations"` и строковые сканеры адаптеров).
   - `--max-lines`, `--max-tokens`, `--cursor` и лестница авто-деградации в `src/cli/pack.rs`.
3. **Волна 3 (ускорение синхронизации в `ci-core` / `ci-mcp`):**
   - Батч `RepoGitSnapshot` + `git ls-tree -r -t -z HEAD` в `git.rs` и проброс в `generator::update_with_options` / `audit`.
   - Единый `WalkBuilder` (`scope.rs` -> `detector.rs` + `scan.rs`), `tokens`/`lines` в `L0_index.json`, устранение двойного `live_source_tokens` в `handlers.rs`.
   - `rayon` + дисковый кэш метрик по `(len, mtime)` + O(1) срез в `pagination.rs` + точечный `refresh_stale_card`.
