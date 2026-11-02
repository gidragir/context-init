use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MiseTasks {
    pub build: Option<String>,
    pub test: Option<String>,
    pub lint: Option<String>,
    pub fmt: Option<String>,
    pub run: Option<String>,
    pub custom: HashMap<String, String>,
}

pub fn parse_mise_tasks(root: &Path) -> Option<MiseTasks> {
    let mise_path = if root.join("mise.toml").exists() {
        root.join("mise.toml")
    } else if root.join(".mise.toml").exists() {
        root.join(".mise.toml")
    } else {
        return None;
    };

    let content = fs::read_to_string(&mise_path).ok()?;
    let value: toml::Value = toml::from_str(&content).ok()?;
    let tasks_table = value.get("tasks")?.as_table()?;

    let mut tasks = MiseTasks::default();

    let has_task = |name: &str| -> bool { tasks_table.contains_key(name) };

    // Standard slot mapping:
    // 1. build <- build
    if has_task("build") {
        tasks.build = Some("mise run build".to_string());
    }

    // 2. test <- test, fallback check
    if has_task("test") {
        tasks.test = Some("mise run test".to_string());
    } else if has_task("check") {
        tasks.test = Some("mise run check".to_string());
    }

    // 3. lint <- lint, fallback clippy, checkstyle
    if has_task("lint") {
        tasks.lint = Some("mise run lint".to_string());
    } else if has_task("clippy") {
        tasks.lint = Some("mise run clippy".to_string());
    } else if has_task("checkstyle") {
        tasks.lint = Some("mise run checkstyle".to_string());
    }

    // 4. fmt <- fmt, fallback format
    if has_task("fmt") {
        tasks.fmt = Some("mise run fmt".to_string());
    } else if has_task("format") {
        tasks.fmt = Some("mise run format".to_string());
    }

    // 5. run <- run, fallback desktop, start, dev
    if has_task("run") {
        tasks.run = Some("mise run run".to_string());
    } else if has_task("desktop") {
        tasks.run = Some("mise run desktop".to_string());
    } else if has_task("start") {
        tasks.run = Some("mise run start".to_string());
    } else if has_task("dev") {
        tasks.run = Some("mise run dev".to_string());
    }

    // Custom tasks: all tasks whose name is not one of the standard primary slots
    for key in tasks_table.keys() {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !matches!(trimmed, "build" | "test" | "lint" | "fmt" | "run") {
            tasks
                .custom
                .insert(trimmed.to_string(), format!("mise run {trimmed}"));
        }
    }

    Some(tasks)
}
