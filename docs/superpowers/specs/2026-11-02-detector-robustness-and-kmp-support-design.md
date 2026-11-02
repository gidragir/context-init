# context-init: Надёжность детекции, модульная топология и поддержка Kotlin Multiplatform

**Дата:** 2026-11-02  
**Статус:** Согласован, готов к планированию реализации  
**Основание:** Регрессионный аудит генерации документации проекта `mimir` (`/data/projects/mimir`), ревью 6 направлений улучшений и верификация независимым рецензентом (Builder Check).

---

## 1. Цель и критерии успеха

Обеспечить корректную, детерминированную и расширяемую генерацию трехуровневой базы знаний OpenViking (L0–L2) для сложных полиглотных монорепозиториев и мультимодульных экосистем (Gradle/KMP, Mise, Cargo, pnpm). Устранить структурную деградацию L0/L1 карточек, предотвратить атомизацию тестов и извлекать точные контракты символов для современных языков.

### Критерии успеха

| # | Критерий | Как проверяется |
|---|---|---|
| **S1** | **Декларативные подсистемы Gradle:** Мультимодульные проекты Gradle/KMP разбиваются на подсистемы строго по декларациям `settings.gradle.kts` / `settings.gradle`, включая вложенные модули (`:parent:child`), без дублирования каталогов верхнего уровня. Немодульные инфраструктурные папки (`docs`, `scripts`, `packaging`) и обособленные приложения (`app-android`) сохраняются как отдельные подсистемы. | Юнит-тесты в `ci-core`, запуск `context-init init` на `/data/projects/mimir` даёт 11 модулей Gradle + инфраструктурные подсистемы вместо 59. |
| **S2** | **Неатомарность тестов:** Одиночные тестовые файлы (`*Test.kt`, `*.test.ts`, `tests/*.rs`) никогда не промоутятся в самостоятельные L1-подсистемы. Тесты группируются в `<subsystem>_tests` либо по таргетам (`<subsystem>_tests_common`, `<subsystem>_tests_jvm`). | `context-init init` на `mimir/core` формирует не более 1–2 парных тестовых подсистем вместо 29 карточек тестов. |
| **S3** | **Сборочные манифесты в Entrypoints:** `build.gradle.kts`, `settings.gradle.kts`, `pom.xml`, `CMakeLists.txt` отображаются в `L0_index.md:entrypoints`, а также распознаются как entrypoints модулей L1 в `is_file_entrypoint`. | Проверка `L0_index.json` и `L1` карточек на `mimir`. |
| **S4** | **Символы Kotlin через полноценный адаптер:** Реализован `KotlinAdapter: LanguageAdapter`, сигнатуры `fun`, `suspend fun`, `interface`, `sealed class`, `data class`, `object`, `value class`, включая `internal`, извлекаются в `key_symbols` карточки `core`; приватные сущности (`private`) игнорируются; устранён баг захвата 12 строк без `{`. | `context_get_subsystem("core")` возвращает интерфейсы `LocalVault`, `SyncEngine`, `RemoteStorage`. |
| **S5** | **Интеграция с `mise.toml`:** Задачи из `[tasks.*]` транслируются в `ProjectCommands` в каноническом формате `mise run <task>`, `mise run test` исполняет реальную команду (например, `allTests`), а псевдокоманда `"runner": "mise (task runner)"` удалена. | Проверка `commands.md` и `L0_index.json:commands` на `mimir` и `context-init`. |
| **S6** | **Безопасный тримминг L0:** Описание в L0 обрезается строго по границам предложений или слов с корректным сохранением многобайтового UTF-8 и многоточием; устранён обрыв слов наподобие «Локал». | Юнит-тесты на кириллицу > 220 символов, срез `mimir/README.md` завершается точкой на «...само.». |

---

## 2. Исходные факты и выявленные дефекты

1. **Gradle `parse_gradle` всегда пуст:** `crates/ci-core/src/analysis/detector.rs:311` жестко возвращает `Some((ptype, name, Vec::new()))`. Модули `mimir` обнаруживаются случайно по эвристике Шага 5 (сканирование корня).
2. **Слепой вторичный сплиттер тестов:** В `detector.rs:1436-1450` при превышении лимита в 15 000 токенов и отсутствии pinned entrypoint все файлы $\ge 300$ токенов промоутятся в отдельные подсистемы. 29 тестовых файлов `core/src/commonTest/...` стали отдельными L1-карточками. Детектор `is_test_path` (строка 1639) не знает про `commonTest`, `jvmTest`, `*Test.kt`.
3. **Отсутствие Gradle/Maven манифестов в точках входа:** `candidate_paths` (`detector.rs:766`) и `is_file_entrypoint` (`detector.rs:1610`) содержат `Cargo.toml` и `package.json`, но не содержат `build.gradle.kts` / `pom.xml`. В корне `mimir` точками входа ложно объявляются `package.json` и `mise.toml`.
4. **Слепота к Kotlin и баг терминатора 12 строк:** Ветка `_ =>` в `is_symbol_start` не знает ключевых слов Kotlin. В строке 2049 завершением сигнатуры считается `{` или `;`. Однострочные объявления без тела в Kotlin захватывают до 12 последующих строк кода (как в `RemoteExceptions.kt`). В `BraceLangAdapter` метод `extract_symbols` захардкожен на `"ts"`.
5. **Фиктивная интеграция `mise`:** `detector.rs:745` пишет строковую заглушку `"runner": "mise (task runner)"`, требующую костыля `if k != "runner"` для `rtk`. Реальные команды `mise run test` (`allTests`) не считываются.
6. **Разрыв слов при срезе:** `trimmed.chars().take(220).collect()` в `detector.rs:590` и `openviking.rs:truncate_chars` слепо режут строку на полуслове без знаков препинания.

---

## 3. Архитектурный подход

Выбрана **модульная декомпозиция (Best Practices & Extensibility)**:
Вместо раздувания 2400-строчного файла `detector.rs` специализированная логика выносится в изолированные модули крейта `crates/ci-core/src/analysis/`:
- `detector/gradle.rs` — парсер мультимодульных Gradle-деклараций;
- `detector/mise.rs` — извлечение и валидация задач `mise.toml`;
- `detector/tests_classifier.rs` — единая логика классификации и группировки тестовых наборов для всех экосистем;
- `lang/kotlin.rs` — полноценный `KotlinAdapter`, реализующий `LanguageAdapter` (`crates/ci-core/src/analysis/lang/mod.rs`), с удалением `"kt"`, `"kts"` из `BraceLangAdapter`.
- В `crates/ci-core/src/openviking.rs` реализуется переиспользуемая утилита безопасного тримминга `truncate_to_boundary`.

Порядок внедрения разбит на **3 последовательные фазы**:
- **Фаза 1:** Точечные безопасные контракты (L0-срез, манифесты entrypoints, интеграция `mise`).
- **Фаза 2:** Топология подсистем и сплиттер (Gradle workspace, защита от атомизации тестов).
- **Фаза 3:** Лексический анализ и экстракция контрактов Kotlin через `KotlinAdapter`.

---

## 4. Фаза 1 — Контракты точек входа, команд и L0-описания

### 4.1. Безопасный тримминг: `openviking::truncate_to_boundary`

В `crates/ci-core/src/openviking.rs` реализуется функция:
```rust
/// Обрезает строку до `max_chars`, не разрывая границы UTF-8, предложений и слов.
pub fn truncate_to_boundary(text: &str, max_chars: usize) -> String
```

#### Алгоритм:
1. `trimmed = text.trim()`. Если `char_count <= max_chars` — возврат исходной строки.
2. Резервируется бюджет `budget = max_chars - 3` под многоточие `...`.
3. Срез безопасного префикса строго по `char_indices` (исключает панику на кириллице).
4. **Приоритет 1 (Предложение):** Поиск последнего символа из `['.', '!', '?']`. Если позиция покрывает $\ge 50\%$ бюджета — срез по знаку препинания без добавления многоточия.
5. **Приоритет 2 (Слово):** Поиск последнего пробела `is_whitespace()`. Срез очищается от висячих знаков `trim_end_matches([' ', '\t', ',', ';', ':', '-', '—', '–', '.'])` и снабжается `...`.
6. **Fallback (Токен без пробелов):** Срез префикса + `...`.
7. Замена логики в `crates/ci-core/src/analysis/detector.rs:extract_description` и `crates/ci-core/src/openviking.rs:truncate_chars`.

### 4.2. Регистрация сборочных манифестов

1. **Корень (`detect_root_entrypoints` в `detector.rs`):**
   Расширить `candidate_paths` манифестами:
   `"build.gradle.kts"`, `"settings.gradle.kts"`, `"build.gradle"`, `"settings.gradle"`, `"pom.xml"`, `"CMakeLists.txt"`, `"Makefile"`, `"pnpm-workspace.yaml"`, `"Justfile"`, `"justfile"`.
2. **Подсистемы (`is_file_entrypoint` в `detector.rs:1610`):**
   Добавить:
   ```rust
   matches!(
       name,
       "main.rs" | "lib.rs" | "mod.rs" | "index.ts" | "main.ts" | "app.ts"
       | "server.ts" | "main.py" | "app.py" | "main.go" | "Cargo.toml"
       | "package.json" | "build.gradle.kts" | "build.gradle" | "pom.xml"
       | "CMakeLists.txt"
   )
   ```

### 4.3. Модуль интеграции задач `detector/mise.rs`

Создать модуль `crates/ci-core/src/analysis/detector/mise.rs`:
```rust
pub struct MiseTasks {
    pub build: Option<String>,
    pub test: Option<String>,
    pub lint: Option<String>,
    pub fmt: Option<String>,
    pub run: Option<String>,
    pub custom: HashMap<String, String>,
}

pub fn parse_mise_tasks(root: &Path) -> Option<MiseTasks>
```

#### Правила парсинга и слияния с ProjectType:
1. Читать `mise.toml` через `toml::Value`. При отсутствии `[tasks]` возвращать `None`.
2. **Канонический запуск:** Запрещено извлекать сырое тело `run = "..."`. Команды генерируются строго как `mise run <task>`.
3. **Маппинг стандартных слотов:**
   - `build` $\leftarrow$ задача `build`.
   - `test` $\leftarrow$ задача `test` (если в `mise.toml` объявлена `test`, она запускает прописанный в ней скрипт, например `./gradlew allTests`), fallback на `check`.
   - `lint` $\leftarrow$ задача `lint` (fallback: `clippy`, `checkstyle`).
   - `fmt` $\leftarrow$ задача `fmt` (fallback: `format`).
   - `run` $\leftarrow$ задача `run` (fallback: `desktop`, `start`, `dev`).
4. **Слияние команд:** Если задача присутствует в `mise.toml`, она переопределяет языковой дефолт `ProjectType`. Если слот в `mise.toml` отсутствует (например, нет `lint`), сохраняется дефолт языка.
5. **Секция `custom`:** Все остальные задачи маппятся в `custom` как `task_name => "mise run <task_name>"`.
6. **Ликвидация заглушек:** Полное удаление `"runner": "mise (task runner)"` и исключающего блока `if k != "runner"` при префиксации `rtk`.

---

## 5. Фаза 2 — Топология подсистем и изоляция тестов

### 5.1. Модуль Gradle Workspace `detector/gradle.rs`

Создать `crates/ci-core/src/analysis/detector/gradle.rs`:
```rust
pub fn parse_gradle_workspace(root: &Path) -> (Option<String>, Vec<String>)
```

#### Алгоритм извлечения подпроектов:
1. Поиск `settings.gradle.kts` и `settings.gradle`.
2. Извлечение имени `rootProject.name`.
3. Парсинг вызовов `include(...)`:
   - Поддержка синтаксиса Kotlin DSL: `include(":core", ":drive")`.
   - Поддержка синтаксиса Groovy DSL: `include ':core', ':drive'`.
4. Парсинг `includeBuild(...)`: включение путей композитных сборок при условии, что они находятся строго внутри `root` (`target.strip_prefix(root).is_ok()`).
5. **Нормализация путей:**
   - Замена внутренних двоеточий `:` на `/`.
   - Удаление ведущих и замыкающих `:` и пробелов (`":features:auth"` $\to$ `"features/auth"`).
   - Проверка физического существования директории `root.join(&normalized).is_dir()`.
6. **Защита от Parent Overlap при сохранении инфраструктурных подсистем:**
   В `discover_subsystems` (Шаг 5 сканирования корня) каталоги верхнего уровня пропускаются **только если** они являются родителями уже обнаруженного подпроекта:
   ```rust
   && !dirs_to_inspect.iter().any(|d| d.starts_with(&path) && d != &path)
   ```
   *Результат для mimir:* Подсистемы `core`, `drive`, `ui` регистрируются из Gradle workspace; инфраструктурные каталоги `docs/`, `scripts/`, `packaging/` и независимое приложение `app-android/` корректно регистрируются на Шаге 5; фиктивные промежуточные каталоги не создаются.

### 5.2. Классификатор тестов `detector/tests_classifier.rs` и архитектура группировки

1. **Расширение `is_test_path` (`tests_classifier.rs`):**
   - Сегменты пути: `tests`, `test`, `commonTest`, `jvmTest`, `jsTest`, `src/test`, `__tests__`, `e2e`, `spec`, `specs`.
   - Имена файлов: `*Test.kt`, `*Tests.kt`, `*Spec.kt`, `*Test.java`, `*TestCase.java`, `*.test.*`, `*.spec.*`, `*_test.go`, `tests.rs`, `*_test.rs`, `test_*.py`.
2. **Алгоритм группировки тестов в `module_entries` и `SplitContext`:**
   - **Фаза сбора (`module_entries`):** Все файлы модуля, для которых `is_test_path` возвращает `true`, объединяются в специальный `ModuleEntry` с ключом `tests`. Если тесты разделены по таргет-директориям (`commonTest`, `jvmTest`), они группируются по таргету: `tests_common`, `tests_jvm`.
   - **Фаза сплита (`SplitContext::split`):**
     1. В первичном цикле промоушена (строки 1404–1419) и вторичном цикле (строки 1436–1450) вводится инвариант: одиночный тестовый файл (`child.dir.is_none() && is_test_path`) **запрещено промоутировать в L1-подсистему**.
     2. Если модуль с тестами укладывается в `split_threshold_tokens = 15000`, тесты остаются в модуле.
     3. Если превышает — промоутится целиком группа тестов как `<subsystem>_tests` (или `<subsystem>_tests_<target>`).
     4. Если плоский каталог тестов без подпапок превышает лимит, он остаётся единой подсистемой `<subsystem>_tests`, а размер L1-карточки ограничивается через `representative_files` (максимум 15 файлов), исключая развал на одиночные файлы.

---

## 6. Фаза 3 — Экстракция контрактов Kotlin через `KotlinAdapter`

### 6.1. Полноценный адаптер `lang/kotlin.rs`

Создать `crates/ci-core/src/analysis/lang/kotlin.rs`:
```rust
pub struct KotlinAdapter;

impl LanguageAdapter for KotlinAdapter {
    fn language_id(&self) -> &'static str { "kotlin" }
    fn extensions(&self) -> &'static [&'static str] { &["kt", "kts"] }
    fn extract_symbols(&self, source: &str) -> Vec<String>;
    fn compress_source(&self, source: &str) -> String;
}
```

1. **Регистрация в `crates/ci-core/src/analysis/lang/mod.rs`:**
   - Добавить `KotlinAdapter` в список адаптеров `all()`.
   - Удалить `"kt"`, `"kts"` из `BRACE_EXTENSIONS` в `brace.rs`.
2. **Маршрутизация детектора в `detector.rs:extract_symbols_from_single_file`:**
   ```rust
   if let Some(adapter) = crate::analysis::lang::adapter_for_ext(&ext) {
       let syms = adapter.extract_symbols(content);
       if !syms.is_empty() { return syms; }
   }
   ```
3. **Логика `KotlinAdapter::extract_symbols`:**
   - **Токены начала символа:**
     `class `, `interface `, `object `, `value class `, `data class `, `sealed class `, `sealed interface `, `enum class `, `fun interface `, `typealias `, `fun `, `suspend fun `, `inline fun `, `override fun `.
   - **Модификаторы видимости:** поддержка `internal ` и `public `.
   - **Фильтр исключений:** Строки с `private ` и аннотации игнорируются.
   - **Фикс бага терминатора 12 строк:** Сигнатура завершена, если `parens == 0 && brackets == 0` и:
     1. Строка содержит `{`;
     2. Либо строка заканчивается идентификатором, типом или закрывающей скобкой и не содержит операторов продолжения (`=`, `,`, `+`, `->`).
4. **Логика `KotlinAdapter::compress_source`:**
   - В отличие от TypeScript, где на `depth == 1` вырезается весь класс, для Kotlin `depth == 1` — это тело класса/интерфейса. Тела функций и методов вырезаются на уровне `depth >= 2`, сохраняя объявления методов и свойств интерфейсов.

---

## 7. Верификация и план тестирования

### 7.1. Модульные тесты (`crates/ci-core/src/analysis/detector/tests.rs`)
1. `test_truncate_to_boundary_cyrillic_and_punctuation`: проверка среза строк с точками, пробелами и непрерывными URL.
2. `test_detect_root_entrypoints_gradle_maven`: проверка регистрации `build.gradle.kts` и `pom.xml`.
3. `test_parse_gradle_multi_project_kts_and_groovy`: парсинг `include(":a", ":b:c")` с нормализацией путей.
4. `test_prevent_test_file_atomization`: симуляция каталога с 30 тестами Kotlin; проверка агрегации в `<subsystem>_tests` и отсутствия одиночных L1-карточек тестов.
5. `test_extract_kotlin_symbols_multiline`: извлечение `sealed interface`, `internal fun`, `data class` и проверка отсутствия склеивания 12 строк.
6. `test_parse_mise_tasks_runner`: проверка маппинга задач и формата `mise run <task>`.

### 7.2. Сквозная интеграционная верификация на `/data/projects/mimir`
Выполнить прогон:
```bash
rtk cargo run -p context-init -- init /data/projects/mimir --force --deep --with-rtk --no-agents-md
```
Критерии приемки:
1. Число подсистем сократилось с 59 до целевых **14–16** (11 модулей Gradle + `app-android` + 3 инфраструктурных каталога `docs`, `scripts`, `packaging` + объединенные парные сьюты тестов).
2. Карточка `core.md` содержит реальные контракты `LocalVault`, `SyncEngine`, `RcloneCipher`.
3. В `L0_index.md` точки входа включают `build.gradle.kts`, `settings.gradle.kts` и `mise.toml`.
4. В `L0_index.md` описание проекта завершается законченным предложением на «...само.».
5. В `cli/commands.md` команды сборки и тестов используют `rtk mise run build` и `rtk mise run test`.
6. Вызов `rtk context-init check /data/projects/mimir` рапортует `0 drift`.
