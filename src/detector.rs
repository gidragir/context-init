use crate::types::{ProjectAnalysis, ProjectType};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".agents",
    ".gemini",
    "node_modules",
    "target",
    "dist",
    "build",
    ".idea",
    ".vscode",
    ".cargo",
    "vendor",
    "__pycache__",
    ".venv",
];

pub fn analyze_project(root: &Path) -> Result<ProjectAnalysis> {
    let canonical = fs::canonicalize(root)
        .with_context(|| format!("Не удалось получить канонический путь для {:?}", root))?;

    let default_name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-project")
        .to_string();

    let (project_type, detected_name) = detect_type_and_name(&canonical);
    let name = detected_name.unwrap_or(default_name);
    let description = extract_description(&canonical, &name);
    let detected_modules = discover_modules(&canonical);
    let rules = generate_default_rules(&project_type);

    Ok(ProjectAnalysis {
        name,
        root_dir: canonical,
        project_type,
        description,
        detected_modules,
        rules,
    })
}

fn detect_type_and_name(root: &Path) -> (ProjectType, Option<String>) {
    if root.join("Cargo.toml").exists() {
        let name = read_cargo_pkg_name(&root.join("Cargo.toml"));
        return (ProjectType::Rust, name);
    }

    if root.join("package.json").exists() {
        let name = read_package_json_name(&root.join("package.json"));
        return (ProjectType::TypeScript, name);
    }

    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        return (ProjectType::Python, None);
    }

    if root.join("go.mod").exists() {
        return (ProjectType::Go, None);
    }

    if root.join("docker-compose.yml").exists() || root.join("compose.yaml").exists() {
        return (ProjectType::DevOps, None);
    }

    (ProjectType::Generic, None)
}

fn read_cargo_pkg_name(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name =") || trimmed.starts_with("name=") {
            let parts: Vec<&str> = trimmed.split('=').collect();
            if parts.len() >= 2 {
                return Some(
                    parts[1]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                );
            }
        }
    }
    None
}

fn read_package_json_name(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
    parsed
        .get("name")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
}

fn extract_description(root: &Path, fallback_name: &str) -> String {
    for readme_name in &["README.md", "readme.md", "README.txt"] {
        let readme_path = root.join(readme_name);
        if let Ok(content) = fs::read_to_string(readme_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                // Пропускаем заголовки и пустые строки, берем первое осмысленное предложение
                if !trimmed.is_empty()
                    && !trimmed.starts_with('#')
                    && !trimmed.starts_with("![")
                    && !trimmed.starts_with('[')
                {
                    if trimmed.len() > 15 {
                        return trimmed.chars().take(200).collect();
                    }
                }
            }
        }
    }
    format!("Проект {}", fallback_name)
}

fn discover_modules(root: &Path) -> Vec<String> {
    let mut modules = Vec::new();

    // 1. Проверяем стандартные подпапки src/ (например, src/api, src/models)
    let src_dir = root.join("src");
    if src_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&src_dir) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        if let Ok(name) = entry.file_name().into_string() {
                            if !IGNORED_DIRS.contains(&name.as_str()) {
                                modules.push(format!("src/{}", name));
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Сканируем директории верхнего уровня
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    if let Ok(name) = entry.file_name().into_string() {
                        if !IGNORED_DIRS.contains(&name.as_str()) && name != "src" {
                            modules.push(name);
                        }
                    }
                }
            }
        }
    }

    modules.sort();
    modules
}

fn generate_default_rules(pt: &ProjectType) -> Vec<String> {
    match pt {
        ProjectType::Rust => vec![
            "Все внешние крейты и зависимости объявляются в Cargo.toml.".to_string(),
            "Код должен проходить cargo clippy и cargo fmt без предупреждений.".to_string(),
            "Использование unsafe запрещено без строгого комментария с обоснованием безопасности (// SAFETY:).".to_string(),
        ],
        ProjectType::TypeScript => vec![
            "Строгая типизация (TypeScript strict mode) обязательна.".to_string(),
            "Все зависимости управляются через package.json (pnpm / bun).".to_string(),
            "Код должен форматироваться через biome / prettier.".to_string(),
        ],
        ProjectType::Python => vec![
            "Все зависимости фиксируются в pyproject.toml через uv.".to_string(),
            "Использовать type hints для всех функций и методов.".to_string(),
        ],
        ProjectType::Go => vec![
            "Код должен быть организован по структуре internal/ и pkg/.".to_string(),
            "Все ошибки должны обрабатываться явно (никаких проигнорированных err).".to_string(),
        ],
        ProjectType::DevOps => vec![
            "Вся инфраструктура декларативна (IaC). Никаких ручных изменений в проде.".to_string(),
            "Все секреты и пароли должны храниться через SOPS / Vault / Age.".to_string(),
        ],
        ProjectType::Generic => vec![
            "Поддерживать документацию модулей в актуальном состоянии.".to_string(),
            "Все изменения фиксируются через атомарные коммиты.".to_string(),
        ],
    }
}
