# 🦀 Практическое руководство по Rust: Изучаем язык на примере CLI-утилиты `context-init`

> **Цель руководства:** На живом, работающем примере утилиты [`crates/context-init`](file:///home/gidragir/projects/dotfiles/crates/context-init/) разобрать ключевые идиомы современного Rust, архитектуру CLI-приложений и научиться писать быстрые, надежные консольные инструменты.

---

## 🧭 1. Что делает `context-init`

`context-init` — это консольная утилита, которая:
1. Сканирует любой проект (Rust, TypeScript, Python, Go, DevOps).
2. Определяет его тип по файлам-маркерам (`Cargo.toml`, `package.json`, `go.mod`, etc.).
3. Находит модули и папки.
4. Опционально запрашивает локальную Ollama (`qwen2.5-coder:14b`) для генерации умных описаний.
5. Создает готовую трехуровневую базу знаний `.agents/knowledge/` (`L0_index.json`, `subsystems/*.md`, `adr/`, `cli/`).

Утилита уже скомпилирована и установлена в `~/.local/bin/context-init` (доступна глобально в вашей системе).

---

## 🏗 2. Архитектура проекта в Rust

В Rust код организуется в **пакеты (packages)** и **крейты (crates)**. Наша утилита находится в [`crates/context-init/`](file:///home/gidragir/projects/dotfiles/crates/context-init/):

```
crates/context-init/
├── Cargo.toml                  # Манифест зависимостей и метаданных
└── src/
    ├── main.rs                 # Точка входа, парсинг аргументов CLI (clap)
    ├── types.rs                # Модели данных, структуры и перечисления (structs & enums)
    ├── detector.rs             # Анализ файловой системы и распознавание проектов
    ├── generator.rs            # Генерация файлов и каталогов L0-L2
    └── ollama.rs               # Сетевой клиент к локальной LLM (ureq + serde_json)
```

---

## 🧠 3. Разбор ключевых концепций Rust на примерах кода

### 1. Перечисления (`enum`) и сопоставление с образцом (`match`)
В отличие от других языков, `enum` в Rust — это мощный алгебраический тип данных.

В [`src/types.rs`](file:///home/gidragir/projects/dotfiles/crates/context-init/src/types.rs):
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectType {
    Rust,
    TypeScript,
    Python,
    Go,
    DevOps,
    Generic,
}
```

А в [`src/detector.rs`](file:///home/gidragir/projects/dotfiles/crates/context-init/src/detector.rs) мы сопоставляем тип с правилами через `match`:
```rust
fn generate_default_rules(pt: &ProjectType) -> Vec<String> {
    match pt {
        ProjectType::Rust => vec![
            "Все внешние зависимости объявляются в Cargo.toml.".to_string(),
            "Код должен проходить cargo clippy и cargo fmt.".to_string(),
        ],
        ProjectType::TypeScript => vec![
            "Строгая типизация (TypeScript strict) обязательна.".to_string(),
        ],
        // Компилятор Rust гарантирует: если вы добавите новый ProjectType,
        // проект НЕ скомпилируется, пока вы не обработаете его в match!
        _ => vec!["Соблюдать стандарты проекта.".to_string()],
    }
}
```
> [!TIP]
> **Паттерн:** `match` в Rust исчерпывающий (exhaustive). Вы физически не можете «забыть» обработать один из вариантов — компилятор укажет на ошибку.

---

### 2. Автоматическая реализация поведения: Derive-макросы
Строка `#[derive(Serialize, Deserialize, Debug, Clone)]` перед структурой заставляет компилятор сгенерировать сложный код за вас:
* `Debug`: позволяет выводить объект в консоль через `println!("{:?}", object)`.
* `Clone`: дает возможность создать глубокую копию через `.clone()`.
* `Serialize` и `Deserialize` (из библиотеки `serde`): мгновенно превращают структуру в JSON-строку и обратно:
  ```rust
  let json_string = serde_json::to_string_pretty(&l0_index)?;
  ```

---

### 3. Обработка ошибок без исключений: `Result<T, E>` и оператор `?`
В Rust нет `try-catch`. Функции, которые могут упасть (например, чтение файла или запрос к сети), возвращают тип `Result<Успех, Ошибка>`.

Оператор `?` (вопросительный знак) заменяет 5 строк boilerplate-кода:
```rust
// Вместо:
// let file = match fs::read_to_string(path) {
//     Ok(content) => content,
//     Err(e) => return Err(e.into()),
// };

// В Rust мы пишем элегантно:
let content = fs::read_to_string(path)
    .with_context(|| format!("Не удалось прочитать файл {:?}", path))?;
```
Если произошла ошибка, оператор `?` мгновенно прерывает функцию и пробрасывает ошибку наверх с контекстом от `anyhow`.

---

### 4. Парсинг CLI аргументов через `clap` (Derive-подход)
Вам не нужно вручную парсить `argv`. В [`src/main.rs`](file:///home/gidragir/projects/dotfiles/crates/context-init/src/main.rs):

```rust
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "context-init", version = "0.1.0")]
struct Args {
    /// Путь к проекту (позиционный аргумент, по умолчанию ".")
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Флаг --ai (короткий -a): подключить локальную Ollama
    #[arg(short, long, default_value_t = false)]
    ai: bool,

    /// Опция --model: выбор модели
    #[arg(short, long, default_value = "qwen2.5-coder:14b")]
    model: String,
}
```
Только за счет этой структуры `clap` автоматически генерирует:
* Форматированный `--help` со всеми типами и описаниями.
* Обработку флагов `--ai`, `-a`, `--model <NAME>`, `-m`.
* Валидацию переданных путей.

---

### 5. Сетевой HTTP-клиент без оверхеда: `ureq`
Вместо тяжелого асинхронного `reqwest + tokio` (который увеличивает время компиляции на 20 секунд и размер бинарника на 10 МБ), мы использовали легковесный синхронный `ureq`:
```rust
let res = ureq::post("http://127.0.0.1:11434/api/generate")
    .timeout(Duration::from_secs(60))
    .send_json(&req)?;
```
Он компилируется за пару секунд и выполняет задачу идеально прямолинейно.

---

## 🛠 4. Как собирать, тестировать и развивать инструмент

### Основные команды при разработке:
1. **Быстрая проверка типов без генерации бинарника (1 секунда):**
   ```bash
   cargo check
   ```
2. **Форматирование кода по стандартам Rust:**
   ```bash
   cargo fmt
   ```
3. **Линтер (поиск неидиоматичного кода и подсказки по улучшению):**
   ```bash
   cargo clippy
   ```
4. **Сборка релизного бинарника с максимальными оптимизациями:**
   ```bash
   cargo build --release
   ```
5. **Установка в систему:**
   ```bash
   cp target/release/context-init ~/.local/bin/
   # либо стандартно через cargo:
   cargo install --path . --root ~/.local
   ```

---

## 🎯 5. Практические задания для самостоятельного изучения Rust

Если вы хотите развить этот проект и закрепить навыки:

1. **Добавить новый флаг `--ignore`:**
   В `src/main.rs` в структуру `Args` добавьте:
   ```rust
   #[arg(short, long)]
   ignore: Vec<String>,
   ```
   И пробросьте игнорируемые директории в функцию `discover_modules` в `detector.rs`.

2. **Добавить поддержку формата YAML для L0:**
   Подключите крейт `serde_yaml = "0.9"` в `Cargo.toml`.
   Добавьте флаг `--yaml` в `Args` и сериализуйте `L0_index.yaml` вместо `L0_index.json`.

3. **Сделать цветной интерактивный опрос:**
   Используйте крейт `dialoguer` (`dialoguer = "0.11"`), чтобы утилита спрашивала подтверждение перед созданием файлов: *«Создать базу знаний для проекта X? [Y/n]»*.
