# context-init: Надёжность детекции, модульная топология и поддержка Kotlin Multiplatform — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Детерминированное выделение Gradle/KMP-подсистем, предотвращение атомизации тестов, извлечение контрактов Kotlin через `KotlinAdapter`, канонические команды `mise run` и безопасный тримминг описаний L0.

**Architecture:** Модульная декомпозиция в `crates/ci-core/src/analysis/`: изолированные модули `detector/gradle.rs`, `detector/mise.rs`, `detector/tests_classifier.rs`, полноценный `lang/kotlin.rs` (`LanguageAdapter`), общая утилита `truncate_to_boundary` в `openviking.rs`.

**Tech Stack:** Rust 2021, `ci-core`, `toml`, `serde_json`, `cargo-nextest`, `mise`, `rtk`.

**Spec:** `docs/superpowers/specs/2026-11-02-detector-robustness-and-kmp-support-design.md`

## Global Constraints

- Все шелл-команды выполняются строго с префиксом `rtk` (`rtk cargo ...`, `rtk git ...`).
- Тесты: `rtk cargo test -p ci-core` и `rtk cargo nextest run --workspace`.
- Перед коммитом: `rtk cargo clippy --workspace --all-targets -- -D warnings` и `rtk cargo fmt --all -- --check`.
- Никаких новых внешних зависимостей (crates): парсинг строковый, лексический, без regexp.
- Безопасность путей: запрет выхода за пределы `root` (`..`) при анализе сборок и манифестов.
- Язык генерируемой базы знаний — `ru` (по конфигурации), комментарии и идентификаторы кода — английские.

## Review Focus

1. Строка без единого пробела (длинный непрерывный URL > 250 символов) в README — `truncate_to_boundary` не паникует, безопасно срезает префикс по границе UTF-8 и добавляет `...`; тест в Task 1 (`test_truncate_to_boundary_unbroken_url`).
2. Манифесты `build.gradle.kts` и `pom.xml` присутствуют в подсистемах первого уровня — `is_file_entrypoint` помечает их как entrypoint подсистемы L1; тест в Task 2 (`test_is_file_entrypoint_gradle_maven`).
3. `mise.toml` содержит многострочный `run = """..."""` или задачи без `run` (`depends = ["a", "b"]`) — `parse_mise_tasks` генерирует строго канонический `mise run <task>`, не ломая markdown-таблицы; тест в Task 3 (`test_parse_mise_tasks_multiline_and_depends`).
4. Gradle-проект с Groovy DSL без скобок (`include 'core', 'drive'`) и вложенными путями (`include ':features:auth'`) — `parse_gradle_workspace` нормализует пути в `core`, `drive`, `features/auth`; тест в Task 4 (`test_parse_gradle_workspace_groovy_and_nested`).
5. Тестовый набор превышает 15 000 токенов в абсолютно плоской папке (без подпапок) — `SplitContext::split` удерживает тесты в единой `<subsystem>_tests`, не распадаясь на 30 карточек; тест в Task 5 (`test_prevent_test_file_atomization_flat_dir`).
6. Однострочные объявления в Kotlin (`class Foo(...) : Bar(...)`, абстрактные методы интерфейсов) без `{` и `;` — `KotlinAdapter` останавливает накопление сигнатуры на текущей строке, не поглощая 12 строк кода; тест в Task 6 (`test_extract_kotlin_symbols_multiline_no_braces`).

---

## Карта файлов

| Файл | Ответственность | Задачи |
|---|---|---|
| `crates/ci-core/src/openviking.rs` | Функция безопасного тримминга `truncate_to_boundary` и обновление `truncate_chars` | Task 1 |
| `crates/ci-core/src/analysis/detector.rs` | Интеграция `truncate_to_boundary` в `extract_description`, расширение `candidate_paths` и `is_file_entrypoint`, вызовы модулей `mise`, `gradle`, `tests_classifier`, `kotlin` | Tasks 1, 2, 3, 4, 5, 6 |
| `crates/ci-core/src/analysis/detector/mise.rs` (новый) | Парсер задач `mise.toml` и генерация `mise run <task>` | Task 3 |
| `crates/ci-core/src/analysis/detector/gradle.rs` (новый) | Парсер Gradle workspace (`include`, `includeBuild`), нормализация `:` $\to$ `/` | Task 4 |
| `crates/ci-core/src/analysis/detector/tests_classifier.rs` (новый) | Универсальный классификатор тестов `is_test_path` для Kotlin, Java, TS, Go, Rust, Python | Task 5 |
| `crates/ci-core/src/analysis/lang/kotlin.rs` (новый) | `KotlinAdapter: LanguageAdapter` (извлечение символов без бага 12 строк, AST-сжатие) | Task 6 |
| `crates/ci-core/src/analysis/lang/mod.rs` | Регистрация `KotlinAdapter` в `all()` | Task 6 |
| `crates/ci-core/src/analysis/lang/brace.rs` | Удаление `"kt"`, `"kts"` из `BRACE_EXTENSIONS` | Task 6 |
| `crates/ci-core/src/analysis/detector/tests.rs` | Модульные тесты для всех новых возможностей | Tasks 1, 2, 3, 4, 5, 6 |

---

### Task 1: Безопасный тримминг описаний L0 (`truncate_to_boundary`)

**Files:**
- Modify: `crates/ci-core/src/openviking.rs:400-430`
- Modify: `crates/ci-core/src/analysis/detector.rs:561-596`
- Test: `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces: `pub fn truncate_to_boundary(text: &str, max_chars: usize) -> String` в `ci_core::openviking`.
- Consumes: Стандартные методы `char_indices`, `chars`, `rfind`.

- [ ] **Step 1: Написать падающие юнит-тесты на тримминг**

В `crates/ci-core/src/analysis/detector/tests.rs` добавить тесты:
```rust
#[test]
fn test_truncate_to_boundary_sentence_priority() {
    let text = "Один открытый инструмент синхронизации Obsidian-хранилища на всех платформах (Linux, Windows, Android). Он ощущается как Google Drive Desktop: один раз указал папку vault и пароль, дальше всё синхронизируется само. Локально лежат обычные .md.";
    let res = crate::openviking::truncate_to_boundary(text, 220);
    assert_eq!(
        res,
        "Один открытый инструмент синхронизации Obsidian-хранилища на всех платформах (Linux, Windows, Android). Он ощущается как Google Drive Desktop: один раз указал папку vault и пароль, дальше всё синхронизируется само."
    );
}

#[test]
fn test_truncate_to_boundary_unbroken_url() {
    let url = "https://example.com/very/long/unbroken/path/without/spaces/that/exceeds/the/character/limit/completely/and/keeps/going/until/it/is/truncated/safely";
    let res = crate::openviking::truncate_to_boundary(url, 50);
    assert!(res.ends_with("..."));
    assert!(res.chars().count() <= 50);
}

#[test]
fn test_truncate_to_boundary_trailing_punctuation() {
    let text = "Первая часть текста: вторая часть текста, которая обрезается.";
    let res = crate::openviking::truncate_to_boundary(text, 25);
    assert!(!res.contains(":,"));
    assert!(res.ends_with("..."));
}
```

- [ ] **Step 2: Запустить тесты и убедиться в ошибке сборки**

Run: `rtk cargo test -p ci-core -- test_truncate_to_boundary`  
Expected: FAIL (функция `truncate_to_boundary` отсутствует в `openviking`).

- [ ] **Step 3: Реализовать `truncate_to_boundary` в `openviking.rs` и внедрить в `detector.rs`**

В `crates/ci-core/src/openviking.rs`:
```rust
pub fn truncate_to_boundary(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    let char_count = trimmed.chars().count();
    if char_count <= max_chars {
        return trimmed.to_string();
    }
    if max_chars <= 3 {
        return trimmed.chars().take(max_chars).collect();
    }

    let budget = max_chars - 3;
    let prefix_char_count = trimmed.chars().take(budget).count();
    let byte_boundary = trimmed
        .char_indices()
        .nth(prefix_char_count)
        .map(|(idx, _)| idx)
        .unwrap_or(trimmed.len());
    let prefix = &trimmed[..byte_boundary];

    let min_useful_chars = budget / 2;
    if let Some(pos) = prefix.rfind(['.', '!', '?']) {
        let candidate = prefix[..=pos].trim_end();
        if candidate.chars().count() >= min_useful_chars {
            return candidate.to_string();
        }
    }

    if let Some(pos) = prefix.rfind(|c: char| c.is_whitespace()) {
        let word_prefix = &prefix[..pos];
        let cleaned = word_prefix.trim_end_matches([' ', '\t', ',', ';', ':', '-', '—', '–', '.']);
        if !cleaned.is_empty() {
            return format!("{}...", cleaned);
        }
    }

    format!("{}...", prefix.trim_end())
}
```
Обновить `truncate_chars` в `openviking.rs` и `extract_description` в `detector.rs:590` вызовом `truncate_to_boundary(trimmed, 220)`.

- [ ] **Step 4: Проверить прохождение тестов**

Run: `rtk cargo test -p ci-core -- test_truncate_to_boundary`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/openviking.rs crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/detector/tests.rs
rtk git commit -m "feat(ci-core): add safe truncate_to_boundary for L0 descriptions"
```

---

### Task 2: Регистрация сборочных манифестов в точках входа L0 и L1

**Files:**
- Modify: `crates/ci-core/src/analysis/detector.rs:765-810, 1610-1630`
- Test: `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces: Обновленные `detect_root_entrypoints` и `is_file_entrypoint` с поддержкой Gradle/Maven/CMake/Makefile/pnpm.

- [ ] **Step 1: Написать падающий тест на регистрацию манифестов**

В `crates/ci-core/src/analysis/detector/tests.rs`:
```rust
#[test]
fn test_is_file_entrypoint_gradle_maven() {
    assert!(super::is_file_entrypoint("build.gradle.kts"));
    assert!(super::is_file_entrypoint("build.gradle"));
    assert!(super::is_file_entrypoint("pom.xml"));
    assert!(super::is_file_entrypoint("CMakeLists.txt"));
}

#[test]
fn test_detect_root_entrypoints_gradle_maven() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
    std::fs::write(dir.path().join("settings.gradle.kts"), "").unwrap();
    std::fs::write(dir.path().join("pom.xml"), "").unwrap();
    let entrypoints = super::detect_root_entrypoints(dir.path());
    assert!(entrypoints.contains(&"build.gradle.kts".to_string()));
    assert!(entrypoints.contains(&"settings.gradle.kts".to_string()));
    assert!(entrypoints.contains(&"pom.xml".to_string()));
}
```

- [ ] **Step 2: Запустить тест и убедиться в ошибке**

Run: `rtk cargo test -p ci-core -- test_is_file_entrypoint_gradle_maven`  
Expected: FAIL (`is_file_entrypoint("build.gradle.kts")` возвращает `false`).

- [ ] **Step 3: Расширить `candidate_paths` и `is_file_entrypoint` в `detector.rs`**

В `detect_root_entrypoints` (`detector.rs:766`) добавить манифесты:
`"build.gradle.kts"`, `"settings.gradle.kts"`, `"build.gradle"`, `"settings.gradle"`, `"pom.xml"`, `"CMakeLists.txt"`, `"Makefile"`, `"pnpm-workspace.yaml"`, `"Justfile"`, `"justfile"`.

В `is_file_entrypoint` (`detector.rs:1610`) добавить:
`"build.gradle.kts" | "build.gradle" | "pom.xml" | "CMakeLists.txt"`.

- [ ] **Step 4: Проверить прохождение тестов**

Run: `rtk cargo test -p ci-core -- test_detect_root_entrypoints_gradle_maven`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/detector/tests.rs
rtk git commit -m "feat(ci-core): register Gradle, Maven, and build manifests in entrypoints"
```

---

### Task 3: Модуль канонического парсинга задач `mise.toml` (`detector/mise.rs`)

**Files:**
- Create: `crates/ci-core/src/analysis/detector/mise.rs`
- Modify: `crates/ci-core/src/analysis/detector.rs:609-763`
- Test: `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces: `pub struct MiseTasks { pub build: Option<String>, pub test: Option<String>, pub lint: Option<String>, pub fmt: Option<String>, pub run: Option<String>, pub custom: HashMap<String, String> }`
- Produces: `pub fn parse_mise_tasks(root: &Path) -> Option<MiseTasks>`
- Consumes: `toml::Value`, `std::fs`.

- [ ] **Step 1: Написать падающий тест на парсинг `mise.toml`**

В `crates/ci-core/src/analysis/detector/tests.rs`:
```rust
#[test]
fn test_parse_mise_tasks_multiline_and_depends() {
    let dir = tempfile::tempdir().unwrap();
    let toml = r#"
[tasks.build]
run = "./gradlew build"

[tasks.test]
description = "Run all tests"
run = "./gradlew allTests"

[tasks.desktop]
run = "./gradlew :app-desktop:run"

[tasks.check]
depends = ["build", "test"]
"#;
    std::fs::write(dir.path().join("mise.toml"), toml).unwrap();
    let tasks = super::mise::parse_mise_tasks(dir.path()).expect("mise tasks must be parsed");
    assert_eq!(tasks.build.as_deref(), Some("mise run build"));
    assert_eq!(tasks.test.as_deref(), Some("mise run test"));
    assert_eq!(tasks.custom.get("desktop").map(String::as_str), Some("mise run desktop"));
    assert_eq!(tasks.custom.get("check").map(String::as_str), Some("mise run check"));
}
```

- [ ] **Step 2: Запустить тест и убедиться в ошибке**

Run: `rtk cargo test -p ci-core -- test_parse_mise_tasks_multiline_and_depends`  
Expected: FAIL (модуль `super::mise` не найден).

- [ ] **Step 3: Реализовать `crates/ci-core/src/analysis/detector/mise.rs` и внедрить в `detector.rs`**

Реализовать безопасный разбор таблицы `[tasks]`, маппинг в канонический формат `mise run <name>`, изъять заглушку `"runner": "mise (task runner)"` из `detector.rs` и удалить костыль `if k != "runner"`. Внедрить объединение `MiseTasks` с `ProjectCommands`.

- [ ] **Step 4: Проверить прохождение тестов**

Run: `rtk cargo test -p ci-core -- test_parse_mise_tasks`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/detector/mise.rs crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/detector/tests.rs
rtk git commit -m "feat(ci-core): add canonical mise.toml task parser and runner commands"
```

---

### Task 4: Модуль парсинга Gradle Workspace и защита от Parent Overlap (`detector/gradle.rs`)

**Files:**
- Create: `crates/ci-core/src/analysis/detector/gradle.rs`
- Modify: `crates/ci-core/src/analysis/detector.rs:267-312, 829-862, 1022-1045`
- Test: `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces: `pub fn parse_gradle_workspace(root: &Path) -> (Option<String>, Vec<String>)`
- Consumes: `rootProject.name`, `include(...)`, `includeBuild(...)`, нормализация `:` $\to$ `/`.

- [ ] **Step 1: Написать падающий тест на парсинг Gradle workspace**

В `crates/ci-core/src/analysis/detector/tests.rs`:
```rust
#[test]
fn test_parse_gradle_workspace_groovy_and_nested() {
    let dir = tempfile::tempdir().unwrap();
    let kts = r#"
rootProject.name = "demo"
include(":core", ":drive")
include(":features:auth")
includeBuild("build-logic")
"#;
    std::fs::write(dir.path().join("settings.gradle.kts"), kts).unwrap();
    std::fs::create_dir_all(dir.path().join("core")).unwrap();
    std::fs::create_dir_all(dir.path().join("drive")).unwrap();
    std::fs::create_dir_all(dir.path().join("features/auth")).unwrap();
    std::fs::create_dir_all(dir.path().join("build-logic")).unwrap();

    let (name, members) = super::gradle::parse_gradle_workspace(dir.path());
    assert_eq!(name.as_deref(), Some("demo"));
    assert!(members.contains(&"core".to_string()));
    assert!(members.contains(&"drive".to_string()));
    assert!(members.contains(&"features/auth".to_string()));
    assert!(members.contains(&"build-logic".to_string()));
}
```

- [ ] **Step 2: Запустить тест и убедиться в ошибке**

Run: `rtk cargo test -p ci-core -- test_parse_gradle_workspace_groovy_and_nested`  
Expected: FAIL (модуль `super::gradle` не существует).

- [ ] **Step 3: Реализовать `crates/ci-core/src/analysis/detector/gradle.rs` и обновить `parse_gradle`**

Реализовать парсинг `include` для Kotlin и Groovy DSL, замену внутренних двоеточий `:` на `/`, тримминг двоеточий, проверку `root.join(&normalized).is_dir()`.  
В `detector.rs:parse_gradle` возвращать найденные `members`.  
В `discover_subsystems` (Шаг 5) добавить фильтр Parent Overlap:
`&& !dirs_to_inspect.iter().any(|d| d.starts_with(&path) && d != &path)`.

- [ ] **Step 4: Проверить прохождение тестов**

Run: `rtk cargo test -p ci-core -- test_parse_gradle_workspace`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/detector/gradle.rs crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/detector/tests.rs
rtk git commit -m "feat(ci-core): add Gradle workspace parser and parent overlap guard"
```

---

### Task 5: Классификатор тестов и инвариант неатомарности (`detector/tests_classifier.rs`)

**Files:**
- Create: `crates/ci-core/src/analysis/detector/tests_classifier.rs`
- Modify: `crates/ci-core/src/analysis/detector.rs:1360-1550, 1639-1650`
- Test: `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces: `pub fn is_test_path(rel: &str) -> bool` с охватом `commonTest`, `jvmTest`, `*Test.kt`, `*.test.ts`, `*_test.go`.
- Produces: Инвариант запрета пофайлового промоушена одиночных тестов в `SplitContext::split`.

- [ ] **Step 1: Написать падающий тест на неатомарность тестов**

В `crates/ci-core/src/analysis/detector/tests.rs`:
```rust
#[test]
fn test_prevent_test_file_atomization_flat_dir() {
    let dir = tempfile::tempdir().unwrap();
    // Создаем модуль с кодом и 25 тестами
    let src_dir = dir.path().join("core/src/commonMain/kotlin");
    let test_dir = dir.path().join("core/src/commonTest/kotlin");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(src_dir.join("Main.kt"), "class Main").unwrap();
    for i in 0..25 {
        let content = format!("class Test{} {{\n    fun test() {{ assert(true) }}\n}}\n", i);
        // Заполняем строками > 300 токенов
        let padded = format!("{}\n{}", content, "// comment\n".repeat(100));
        std::fs::write(test_dir.join(format!("MyTest{}.kt", i)), padded).unwrap();
    }
    let analysis = super::analyze_project(dir.path(), false, false).unwrap();
    let modules = &analysis.detected_modules;
    // Тесты не должны распасться на 25 независимых подсистем
    let single_file_test_modules: Vec<_> = modules
        .iter()
        .filter(|m| m.files.len() == 1 && super::tests_classifier::is_test_path(&m.files[0].path))
        .collect();
    assert_eq!(single_file_test_modules.len(), 0, "No single test file may become an L1 subsystem");
}
```

- [ ] **Step 2: Запустить тест и убедиться в ошибке**

Run: `rtk cargo test -p ci-core -- test_prevent_test_file_atomization`  
Expected: FAIL (тест обнаруживает распавшиеся тестовые модули).

- [ ] **Step 3: Реализовать `tests_classifier.rs` и обновить `SplitContext::split`**

Создать `crates/ci-core/src/analysis/detector/tests_classifier.rs` с полным списком тестовых сегментов и суффиксов.  
В `detector.rs:module_entries` агрегировать файлы тестов.  
В `SplitContext::split` (строки 1404–1419 и 1436–1450) принудительно запретить промоушен одиночного файла тестов (`child.dir.is_none() && is_test_path`).  
Группировать тесты модуля в `<subsystem>_tests` (или `<subsystem>_tests_<target>`).

- [ ] **Step 4: Проверить прохождение тестов**

Run: `rtk cargo test -p ci-core -- test_prevent_test_file_atomization`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/detector/tests_classifier.rs crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/detector/tests.rs
rtk git commit -m "feat(ci-core): isolate tests into classifier and prevent file-level atomization"
```

---

### Task 6: Полноценный адаптер `KotlinAdapter` (`lang/kotlin.rs`)

**Files:**
- Create: `crates/ci-core/src/analysis/lang/kotlin.rs`
- Modify: `crates/ci-core/src/analysis/lang/mod.rs`
- Modify: `crates/ci-core/src/analysis/lang/brace.rs:7-12`
- Modify: `crates/ci-core/src/analysis/detector.rs:1707-1729`
- Test: `crates/ci-core/src/analysis/detector/tests.rs`

**Interfaces:**
- Produces: `pub struct KotlinAdapter` implementing `LanguageAdapter`.
- Consumes: `LanguageAdapter`, `ELIDED_BODY`.

- [ ] **Step 1: Написать падающий тест на извлечение символов Kotlin без бага 12 строк**

В `crates/ci-core/src/analysis/detector/tests.rs`:
```rust
#[test]
fn test_extract_kotlin_symbols_multiline_no_braces() {
    let source = r#"
package dev.mimir.core

sealed interface SyncStatus

class RemoteUnavailableException(message: String) : Exception(message)

internal fun doInternalSync(force: Boolean = false): Boolean

private fun ignoredHelper() {
    println("secret")
}

data class Account(val id: String, val name: String)
"#;
    let adapter = crate::analysis::lang::adapter_for_ext("kt").expect("Kotlin adapter must be registered");
    let symbols = adapter.extract_symbols(source);
    assert!(symbols.iter().any(|s| s.contains("sealed interface SyncStatus")));
    assert!(symbols.iter().any(|s| s.contains("class RemoteUnavailableException")));
    assert!(symbols.iter().any(|s| s.contains("internal fun doInternalSync")));
    assert!(symbols.iter().any(|s| s.contains("data class Account")));
    assert!(!symbols.iter().any(|s| s.contains("ignoredHelper")), "private functions must be ignored");
}
```

- [ ] **Step 2: Запустить тест и убедиться в ошибке**

Run: `rtk cargo test -p ci-core -- test_extract_kotlin_symbols_multiline_no_braces`  
Expected: FAIL (адаптер для `"kt"` не возвращает интерфейсы или захватывает лишнее).

- [ ] **Step 3: Реализовать `crates/ci-core/src/analysis/lang/kotlin.rs` и зарегистрировать в `lang/mod.rs`**

Реализовать `KotlinAdapter`:
- `is_symbol_start`: поддержка `class`, `interface`, `object`, `value class`, `data class`, `sealed class`, `sealed interface`, `fun`, `suspend fun`, `inline fun`, `internal`, `public`. Игнорирование `private`.
- Терминация однострочных объявлений: остановка при `parens == 0 && brackets == 0` без операторов продолжения (`=`, `,`, `+`).
- `compress_source`: сохранение сигнатур классов/интерфейсов на `depth == 1`, вырезание тел методов на `depth >= 2`.  
Удалить `"kt"`, `"kts"` из `BraceLangAdapter::BRACE_EXTENSIONS`.  
Зарегистрировать в `crates/ci-core/src/analysis/lang/mod.rs:all()`.  
В `detector.rs:extract_symbols_from_single_file` вызывать `adapter_for_ext(&ext)`.

- [ ] **Step 4: Проверить прохождение тестов**

Run: `rtk cargo test -p ci-core -- test_extract_kotlin_symbols`  
Expected: PASS.

- [ ] **Step 5: Коммит**

```bash
rtk git add crates/ci-core/src/analysis/lang/kotlin.rs crates/ci-core/src/analysis/lang/mod.rs crates/ci-core/src/analysis/lang/brace.rs crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/detector/tests.rs
rtk git commit -m "feat(ci-core): add dedicated KotlinAdapter with accurate symbol extraction and AST compression"
```

---

### Task 7: Сквозная интеграционная верификация на `mimir` и валидация воркспейса

**Files:**
- Test: `/data/projects/mimir`
- Verify: Весь воркспейс `context-init` (`cargo test`, `clippy`, `fmt`)

**Interfaces:**
- Consumes: Бинарник `context-init`, репозиторий `mimir`.

- [ ] **Step 1: Прогнать полную проверку качества кода воркспейса context-init**

Run: `rtk cargo clippy --workspace --all-targets -- -D warnings`  
Expected: 0 warnings.  
Run: `rtk cargo fmt --all -- --check`  
Expected: clean.  
Run: `rtk cargo nextest run --workspace`  
Expected: ALL PASS.

- [ ] **Step 2: Собрать бинарник context-init в release**

Run: `rtk cargo build --release`  
Expected: бинарник `target/release/context-init` собран успешно.

- [ ] **Step 3: Запустить сквозную инициализацию на проекте mimir**

Run: `rtk target/release/context-init init /data/projects/mimir --force --deep --with-rtk --no-agents-md`  
Expected:
1. Количество сгенерированных подсистем: от 14 до 16 (вместо 59).
2. Карточка `core.md` содержит реальные контракты (`LocalVault`, `SyncEngine`, `RcloneCipher`).
3. В `L0_index.md` точки входа включают `build.gradle.kts` и `mise.toml`.
4. В `L0_index.md` описание проекта завершается точкой на «...само.».
5. В `cli/commands.md` команды сборки и тестов используют `rtk mise run build` и `rtk mise run test`.

- [ ] **Step 4: Проверить архитектурный дрейф**

Run: `rtk target/release/context-init check /data/projects/mimir`  
Expected: 0 drift (все карточки Up-to-date).

- [ ] **Step 5: Финальный коммит интеграционных тестов/документации**

```bash
rtk git add .
rtk git commit -m "chore(release): verify detector robustness on mimir multi-project codebase"
```
