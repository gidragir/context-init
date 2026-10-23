use crate::analysis::types::{DiscoveredModule, ProjectAnalysis};
use crate::storage::{ProjectCommands, ProjectType, SubsystemFile};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

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
    "venv",
    ".next",
    ".nuxt",
    ".turbo",
    ".cache",
    "coverage",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".vault",
    "out",
    ".svelte-kit",
    ".astro",
    ".parcel-cache",
];

const CODE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "c", "cpp", "h", "hpp", "java", "kt", "rb", "php",
    "sh", "bash", "zsh", "sql", "yaml", "yml", "json", "toml",
];

pub fn analyze_project(root: &Path, deep: bool, with_rtk: bool) -> Result<ProjectAnalysis> {
    let canonical = fs::canonicalize(root)
        .with_context(|| format!("Не удалось получить канонический путь для {:?}", root))?;

    let default_name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-project")
        .to_string();

    let (project_type, detected_name, workspace_members) = detect_type_and_name(&canonical);
    let name = detected_name.unwrap_or(default_name);
    let description = extract_description(&canonical, &name);
    let commands = detect_commands(&canonical, &project_type, with_rtk);
    let root_entrypoints = detect_root_entrypoints(&canonical);
    let detected_modules = discover_subsystems(
        &canonical,
        &workspace_members,
        &project_type,
        deep,
        with_rtk,
    );
    let rules = generate_default_rules(&project_type, with_rtk);

    Ok(ProjectAnalysis {
        name,
        root_dir: canonical,
        project_type,
        description,
        detected_modules,
        root_entrypoints,
        commands,
        workspace_members,
        rules,
    })
}

fn detect_type_and_name(root: &Path) -> (ProjectType, Option<String>, Vec<String>) {
    // 1. Rust
    if root.join("Cargo.toml").exists() {
        let (name, ws_members) = parse_cargo_toml(&root.join("Cargo.toml"));
        return (ProjectType::Rust, name, ws_members);
    }

    // 2. TypeScript / JavaScript
    if root.join("package.json").exists() || root.join("pnpm-workspace.yaml").exists() {
        let (name, mut ws_members) = parse_package_json(&root.join("package.json"));
        let pnpm_ws = parse_pnpm_workspace_yaml(&root.join("pnpm-workspace.yaml"));
        for w in pnpm_ws {
            if !ws_members.contains(&w) {
                ws_members.push(w);
            }
        }
        return (ProjectType::TypeScript, name, ws_members);
    }

    // 3. Python
    if root.join("pyproject.toml").exists()
        || root.join("requirements.txt").exists()
        || root.join("Pipfile").exists()
    {
        let name = parse_pyproject_name(&root.join("pyproject.toml"));
        return (ProjectType::Python, name, Vec::new());
    }

    // 4. Go
    if root.join("go.mod").exists() {
        let name = parse_go_mod_name(&root.join("go.mod"));
        return (ProjectType::Go, name, Vec::new());
    }

    // 5. DevOps / IaC
    if root.join("docker-compose.yml").exists()
        || root.join("compose.yaml").exists()
        || root.join("Dockerfile").exists()
        || root.join("ansible.cfg").exists()
    {
        return (ProjectType::DevOps, None, Vec::new());
    }

    (ProjectType::Generic, None, Vec::new())
}

fn parse_cargo_toml(path: &Path) -> (Option<String>, Vec<String>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (None, Vec::new()),
    };

    if let Ok(parsed) = content.parse::<toml::Value>() {
        let pkg_name = parsed
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());
        let ws_members = parsed
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        return (pkg_name, ws_members);
    }

    let mut pkg_name = None;
    let mut ws_members = Vec::new();
    let mut in_workspace_members = false;
    let mut in_package_section = false;

    for line in content.lines() {
        let line_no_comment = line.split('#').next().unwrap_or(line);
        let trimmed = line_no_comment.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_package_section = trimmed == "[package]";
        }

        if trimmed.starts_with("members = [") || trimmed == "members = [" {
            in_workspace_members = true;
            // inline members
            if let Some(start) = trimmed.find('[') {
                if let Some(end) = trimmed.rfind(']') {
                    let slice = &trimmed[start + 1..end];
                    for item in slice.split(',') {
                        let m = item.trim().trim_matches('"').trim_matches('\'');
                        if !m.is_empty() {
                            ws_members.push(m.to_string());
                        }
                    }
                    in_workspace_members = false;
                }
            }
            continue;
        }

        if in_workspace_members {
            if trimmed.starts_with(']') {
                in_workspace_members = false;
            } else {
                let m = trimmed
                    .trim_end_matches(',')
                    .trim_matches('"')
                    .trim_matches('\'');
                if !m.is_empty() {
                    ws_members.push(m.to_string());
                }
            }
            continue;
        }

        if in_package_section
            && pkg_name.is_none()
            && (trimmed.starts_with("name =") || trimmed.starts_with("name="))
        {
            let parts: Vec<&str> = trimmed.split('=').collect();
            if parts.len() >= 2 {
                pkg_name = Some(
                    parts[1]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                );
            }
        }
    }

    (pkg_name, ws_members)
}

fn parse_package_json(path: &Path) -> (Option<String>, Vec<String>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (None, Vec::new()),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return (None, Vec::new()),
    };

    let name = parsed
        .get("name")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let mut ws = Vec::new();

    if let Some(workspaces) = parsed.get("workspaces") {
        if let Some(arr) = workspaces.as_array() {
            for v in arr {
                if let Some(s) = v.as_str() {
                    ws.push(s.to_string());
                }
            }
        } else if let Some(packages) = workspaces.get("packages").and_then(|p| p.as_array()) {
            for v in packages {
                if let Some(s) = v.as_str() {
                    ws.push(s.to_string());
                }
            }
        }
    }

    (name, ws)
}

fn parse_pnpm_workspace_yaml(path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut members = Vec::new();
    let mut in_packages = false;

    for line in content.lines() {
        // Убираем комментарии перед парсингом
        let line_no_comment = if let Some(idx) = line.find('#') {
            &line[..idx]
        } else {
            line
        };
        let trimmed = line_no_comment.trim();

        if trimmed.starts_with("packages:") {
            in_packages = true;
            continue;
        }

        if in_packages {
            if trimmed.is_empty() {
                continue;
            }

            // Если не пустая строка и нет отступа — выход из секции packages
            if !line.starts_with(' ') && !line.starts_with('\t') {
                in_packages = false;
                continue;
            }

            if trimmed.starts_with('-') {
                let item = trimmed
                    .trim_start_matches('-')
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('"')
                    .trim();
                // Игнорируем пустые элементы и отрицающие глобы вида '!**/test/**'
                if !item.is_empty() && !item.starts_with('!') {
                    members.push(item.to_string());
                }
            }
        }
    }

    members
}

fn parse_pyproject_name(path: &Path) -> Option<String> {
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

fn parse_go_mod_name(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("module ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let mod_path = parts[1];
                let short_name = mod_path.split('/').next_back().unwrap_or(mod_path);
                return Some(short_name.to_string());
            }
        }
    }
    None
}

fn extract_description(root: &Path, fallback_name: &str) -> String {
    for readme_name in &["README.md", "readme.md", "README.txt"] {
        let readme_path = root.join(readme_name);
        if let Ok(content) = fs::read_to_string(readme_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty()
                    && !trimmed.starts_with('#')
                    && !trimmed.starts_with("![")
                    && !trimmed.starts_with('[')
                    && trimmed.len() > 15
                {
                    return trimmed.chars().take(220).collect();
                }
            }
        }
    }
    format!("Проект {}", fallback_name)
}

pub fn prefix_with_rtk(cmd: &str) -> String {
    let trimmed = cmd.trim();
    if trimmed.is_empty() || trimmed.starts_with("rtk ") {
        trimmed.to_string()
    } else {
        format!("rtk {}", trimmed)
    }
}

static BPE_TOKENIZER: std::sync::OnceLock<Option<tiktoken_rs::CoreBPE>> =
    std::sync::OnceLock::new();

pub fn count_tokens(text: &str) -> usize {
    let bpe = BPE_TOKENIZER.get_or_init(|| tiktoken_rs::cl100k_base().ok());
    if let Some(bpe) = bpe.as_ref() {
        bpe.encode_with_special_tokens(text).len()
    } else {
        text.len() / 4
    }
}

fn detect_commands(root: &Path, pt: &ProjectType, with_rtk: bool) -> ProjectCommands {
    let mut cmds = ProjectCommands::default();

    // Проверяем mise.toml на приоритетные команды
    let mise_path = root.join("mise.toml");
    let has_mise = mise_path.exists();

    match pt {
        ProjectType::Rust => {
            cmds.build = Some("cargo build".to_string());
            cmds.test = Some("cargo test".to_string());
            cmds.lint = Some("cargo clippy".to_string());
            cmds.fmt = Some("cargo fmt --check".to_string());
            if root.join("src/main.rs").exists() {
                cmds.run = Some("cargo run".to_string());
            }
        }
        ProjectType::TypeScript => {
            let pm = if root.join("pnpm-lock.yaml").exists() {
                "pnpm"
            } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
                "bun"
            } else if root.join("yarn.lock").exists() {
                "yarn"
            } else {
                "npm"
            };

            let pkg_path = root.join("package.json");
            if let Ok(content) = fs::read_to_string(pkg_path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(scripts) = val.get("scripts").and_then(|s| s.as_object()) {
                        for (k, v) in scripts {
                            if let Some(cmd_str) = v.as_str() {
                                cmds.custom.insert(k.clone(), cmd_str.to_string());
                            }
                        }

                        if scripts.contains_key("build") {
                            cmds.build = Some(format!("{} run build", pm));
                        }
                        if scripts.contains_key("test") {
                            cmds.test = Some(format!("{} test", pm));
                        }
                        if scripts.contains_key("lint") {
                            cmds.lint = Some(format!("{} run lint", pm));
                        }
                        if scripts.contains_key("format") || scripts.contains_key("fmt") {
                            cmds.fmt = Some(format!("{} run format", pm));
                        }
                        if scripts.contains_key("dev") {
                            cmds.run = Some(format!("{} run dev", pm));
                        } else if scripts.contains_key("start") {
                            cmds.run = Some(format!("{} start", pm));
                        }
                    }
                }
            }

            if cmds.build.is_none() {
                cmds.build = Some(format!("{} run build", pm));
            }
            if cmds.test.is_none() {
                cmds.test = Some(format!("{} test", pm));
            }
        }
        ProjectType::Python => {
            let is_uv = root.join("uv.lock").exists()
                || fs::read_to_string(root.join("pyproject.toml"))
                    .map(|c| c.contains("[tool.uv]"))
                    .unwrap_or(false);
            let is_poetry = root.join("poetry.lock").exists();

            if is_uv {
                cmds.build = Some("uv build".to_string());
                cmds.test = Some("uv run pytest".to_string());
                cmds.lint = Some("uv run ruff check".to_string());
                cmds.fmt = Some("uv run ruff format --check".to_string());
                cmds.run = Some("uv run python main.py".to_string());
            } else if is_poetry {
                cmds.build = Some("poetry build".to_string());
                cmds.test = Some("poetry run pytest".to_string());
                cmds.lint = Some("poetry run flake8".to_string());
                cmds.fmt = Some("poetry run black --check .".to_string());
            } else {
                cmds.test = Some("pytest".to_string());
                cmds.lint = Some("ruff check || flake8".to_string());
                cmds.fmt = Some("ruff format --check || black --check .".to_string());
            }
        }
        ProjectType::Go => {
            cmds.build = Some("go build ./...".to_string());
            cmds.test = Some("go test -v ./...".to_string());
            cmds.lint = Some("golangci-lint run".to_string());
            cmds.fmt = Some("gofmt -l .".to_string());
            cmds.run = Some("go run .".to_string());
        }
        ProjectType::DevOps => {
            if root.join("compose.yaml").exists() || root.join("docker-compose.yml").exists() {
                cmds.run = Some("docker compose up -d".to_string());
                cmds.test = Some("docker compose config".to_string());
            }
        }
        ProjectType::Generic => {
            if root.join("Makefile").exists() {
                cmds.build = Some("make build".to_string());
                cmds.test = Some("make test".to_string());
                cmds.lint = Some("make lint".to_string());
            }
        }
    }

    if has_mise {
        cmds.custom
            .insert("runner".to_string(), "mise (task runner)".to_string());
    }

    if with_rtk {
        cmds.build = cmds.build.map(|c| prefix_with_rtk(&c));
        cmds.test = cmds.test.map(|c| prefix_with_rtk(&c));
        cmds.lint = cmds.lint.map(|c| prefix_with_rtk(&c));
        cmds.fmt = cmds.fmt.map(|c| prefix_with_rtk(&c));
        cmds.run = cmds.run.map(|c| prefix_with_rtk(&c));
        for (k, val) in cmds.custom.iter_mut() {
            if k != "runner" {
                *val = prefix_with_rtk(val);
            }
        }
    }

    cmds
}

fn detect_root_entrypoints(root: &Path) -> Vec<String> {
    let candidate_paths = &[
        "src/main.rs",
        "src/lib.rs",
        "src/index.ts",
        "src/main.ts",
        "src/app.ts",
        "index.ts",
        "index.js",
        "main.py",
        "app.py",
        "src/main.py",
        "main.go",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "Dockerfile",
        "compose.yaml",
        "docker-compose.yml",
        "mise.toml",
    ];

    let mut entrypoints = Vec::new();
    for p in candidate_paths {
        if root.join(p).exists() {
            entrypoints.push(p.to_string());
        }
    }

    // Сканируем src/bin/*.rs если есть
    let bin_dir = root.join("src").join("bin");
    if bin_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&bin_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".rs") {
                    entrypoints.push(format!("src/bin/{}", name));
                }
            }
        }
    }

    entrypoints
}

pub fn is_ignored_path(path: &Path, root: &Path) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components().any(|comp| {
        let s = comp.as_os_str().to_string_lossy();
        IGNORED_DIRS.contains(&s.as_ref())
    })
}

fn discover_subsystems(
    root: &Path,
    workspace_members: &[String],
    pt: &ProjectType,
    deep: bool,
    with_rtk: bool,
) -> Vec<DiscoveredModule> {
    let mut dirs_to_inspect: Vec<PathBuf> = Vec::new();

    // 1. Проверяем explicit workspace members (crates/*, packages/*, apps/*, etc.)
    for member in workspace_members {
        let trimmed_member = member.trim().trim_start_matches("./").trim_end_matches('/');
        if trimmed_member.is_empty() || trimmed_member == "." {
            continue;
        }
        if trimmed_member.ends_with("/*") || trimmed_member.ends_with("/**") {
            let base_dir = trimmed_member
                .trim_end_matches("/*")
                .trim_end_matches("/**");
            let full_base = root.join(base_dir);
            if !is_ignored_path(&full_base, root) && full_base.is_dir() {
                if let Ok(entries) = fs::read_dir(&full_base) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if !IGNORED_DIRS.contains(&name.as_str())
                                && !is_ignored_path(&path, root)
                                && !dirs_to_inspect.contains(&path)
                            {
                                dirs_to_inspect.push(path);
                            }
                        }
                    }
                }
            }
        } else {
            let full = root.join(trimmed_member);
            if full.is_dir() && !is_ignored_path(&full, root) && !dirs_to_inspect.contains(&full) {
                dirs_to_inspect.push(full);
            }
        }
    }

    // 2. Стандартные папки монорепозиториев и сервисов
    for common_parent in &[
        "crates", "packages", "apps", "services", "modules", "plugins", "internal", "cmd", "pkg",
    ] {
        let parent_dir = root.join(common_parent);
        if parent_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&parent_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if !IGNORED_DIRS.contains(&name.as_str())
                            && !dirs_to_inspect.contains(&path)
                        {
                            dirs_to_inspect.push(path);
                        }
                    }
                }
            }
        }
    }

    // 3. Подпапки в src/ (например, src/api, src/db, src/services)
    let src_dir = root.join("src");
    let mut has_src_subdirs = false;
    if src_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&src_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !IGNORED_DIRS.contains(&name.as_str()) && !dirs_to_inspect.contains(&path) {
                        dirs_to_inspect.push(path);
                        has_src_subdirs = true;
                    }
                }
            }
        }
    }

    // 4. Если в src/ нет поддиректорий, но есть несколько файлов
    let mut flat_src_modules: Vec<DiscoveredModule> = Vec::new();
    if !has_src_subdirs && src_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&src_dir) {
            let mut code_files = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if CODE_EXTENSIONS.contains(&ext) {
                        code_files.push(path);
                    }
                }
            }
            code_files.sort();

            if code_files.len() > 2 {
                for file_path in code_files {
                    let stem = file_path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "module".to_string());
                    let mod_id = if stem == "main" || stem == "lib" {
                        "core".to_string()
                    } else {
                        stem.clone()
                    };
                    let target_dir_path = format!("src/{}", mod_id);
                    let rel = file_path
                        .strip_prefix(root)
                        .unwrap_or(&file_path)
                        .to_string_lossy()
                        .to_string();
                    let is_ep = is_file_entrypoint(&format!(
                        "{}.{}",
                        stem,
                        file_path.extension().and_then(|e| e.to_str()).unwrap_or("")
                    ));
                    let lines = count_lines(&file_path);
                    let symbols = extract_symbols_from_single_file(&file_path, deep);
                    let sub_file = SubsystemFile {
                        path: rel.clone(),
                        is_entrypoint: is_ep,
                        lines,
                    };

                    if let Some(existing) = flat_src_modules
                        .iter_mut()
                        .find(|m| m.dir_path == target_dir_path)
                    {
                        existing.name = mod_id;
                        existing.files.push(sub_file);
                        if is_ep && !existing.entrypoints.contains(&rel) {
                            existing.entrypoints.push(rel);
                        }
                        for sym in symbols {
                            if !existing.symbols.contains(&sym) {
                                existing.symbols.push(sym);
                            }
                        }
                    } else {
                        flat_src_modules.push(DiscoveredModule {
                            dir_path: target_dir_path,
                            name: stem,
                            files: vec![sub_file],
                            entrypoints: if is_ep { vec![rel] } else { Vec::new() },
                            symbols,
                            test_cmd: None,
                        });
                    }
                }
            } else if !dirs_to_inspect.contains(&src_dir) {
                dirs_to_inspect.push(src_dir);
            }
        }
    }

    // 5. Директории верхнего уровня (scripts, infra, docs, tests, etc.)
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !IGNORED_DIRS.contains(&name.as_str())
                    && name != "src"
                    && name != "crates"
                    && name != "packages"
                    && name != "apps"
                    && name != "services"
                    && name != "modules"
                    && !dirs_to_inspect.contains(&path)
                {
                    // Проверяем, есть ли там хотя бы один полезный файл с кодом/конфигом
                    if contains_relevant_files(&path) {
                        dirs_to_inspect.push(path);
                    }
                }
            }
        }
    }

    // Сортировка по пути
    dirs_to_inspect.sort();

    let mut result = Vec::new();
    for dir in dirs_to_inspect {
        if is_ignored_path(&dir, root) {
            continue;
        }
        let rel_path = dir
            .strip_prefix(root)
            .unwrap_or(&dir)
            .to_string_lossy()
            .to_string();
        let module_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| rel_path.clone());

        let files = collect_subsystem_files(&dir, root);
        if files.is_empty() {
            continue;
        }

        let entrypoints: Vec<String> = files
            .iter()
            .filter(|f| f.is_entrypoint)
            .map(|f| f.path.clone())
            .collect();

        let symbols = extract_symbols_from_files(&dir, &files, deep);

        let test_cmd = match pt {
            ProjectType::Rust => {
                if dir.join("Cargo.toml").exists() {
                    let pkg_name = parse_cargo_toml(&dir.join("Cargo.toml"))
                        .0
                        .unwrap_or_else(|| module_name.clone());
                    let cmd = format!("cargo test -p {}", pkg_name);
                    Some(if with_rtk { prefix_with_rtk(&cmd) } else { cmd })
                } else {
                    None
                }
            }
            ProjectType::TypeScript => {
                if dir.join("package.json").exists() {
                    let cmd = format!("pnpm --filter {} test", module_name);
                    Some(if with_rtk { prefix_with_rtk(&cmd) } else { cmd })
                } else {
                    None
                }
            }
            _ => None,
        };

        result.push(DiscoveredModule {
            dir_path: rel_path,
            name: module_name,
            files,
            entrypoints,
            symbols,
            test_cmd,
        });
    }

    result.extend(flat_src_modules);
    result.sort_by(|a, b| a.dir_path.cmp(&b.dir_path));

    result
}

fn contains_relevant_files(dir: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if CODE_EXTENSIONS.contains(&ext) || ext == "md" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn collect_subsystem_files(subsystem_dir: &Path, root: &Path) -> Vec<SubsystemFile> {
    let mut files = Vec::new();
    collect_files_recursive(subsystem_dir, root, &mut files, 0, 4);

    // Сортировка: точки входа первыми, затем по объему строк
    files.sort_by(|a, b| {
        b.is_entrypoint
            .cmp(&a.is_entrypoint)
            .then_with(|| b.lines.cmp(&a.lines))
    });

    // Ограничиваем список 15 наиболее репрезентативными файлами для компактности
    if files.len() > 15 {
        files.truncate(15);
    }

    files
}

fn collect_files_recursive(
    current: &Path,
    root: &Path,
    acc: &mut Vec<SubsystemFile>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }

    let entries = match fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if !IGNORED_DIRS.contains(&name.as_str()) {
                collect_files_recursive(&path, root, acc, depth + 1, max_depth);
            }
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if CODE_EXTENSIONS.contains(&ext.as_str()) || ext == "md" {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                let is_entrypoint = is_file_entrypoint(&name);
                let lines = count_lines(&path);

                acc.push(SubsystemFile {
                    path: rel,
                    is_entrypoint,
                    lines,
                });
            }
        }
    }
}

fn is_file_entrypoint(name: &str) -> bool {
    matches!(
        name,
        "main.rs"
            | "lib.rs"
            | "mod.rs"
            | "index.ts"
            | "index.tsx"
            | "index.js"
            | "main.ts"
            | "app.ts"
            | "server.ts"
            | "main.py"
            | "app.py"
            | "__init__.py"
            | "main.go"
            | "Cargo.toml"
            | "package.json"
    )
}

fn count_lines(path: &Path) -> usize {
    fs::read_to_string(path)
        .map(|content| content.lines().count())
        .unwrap_or(0)
}

fn extract_symbols_from_files(
    subsystem_dir: &Path,
    files: &[SubsystemFile],
    deep: bool,
) -> Vec<String> {
    let mut symbols = Vec::new();
    let max_files = if deep {
        files.len().min(15)
    } else {
        files.len().min(5)
    };
    let max_symbols = if deep { 30 } else { 10 };

    for f in files.iter().take(max_files) {
        let full_path = subsystem_dir.join(
            Path::new(&f.path)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("")),
        );
        let target_path = if full_path.exists() {
            full_path
        } else {
            subsystem_dir
                .ancestors()
                .find_map(|p| {
                    let candidate = p.join(&f.path);
                    if candidate.exists() {
                        Some(candidate)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| subsystem_dir.to_path_buf())
        };

        if let Ok(content) = fs::read_to_string(&target_path) {
            let ext = target_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let file_symbols = if ext == "rs" {
                let rust_syms = extract_rust_symbols(&content, deep);
                if rust_syms.is_empty() {
                    extract_multiline_symbols(&content, &ext, deep)
                } else {
                    rust_syms
                }
            } else {
                extract_multiline_symbols(&content, &ext, deep)
            };

            for s in file_symbols {
                if !symbols.contains(&s) {
                    symbols.push(s);
                }
                if symbols.len() >= max_symbols {
                    break;
                }
            }
        }

        if symbols.len() >= max_symbols {
            break;
        }
    }

    symbols
}

fn extract_symbols_from_single_file(path: &Path, deep: bool) -> Vec<String> {
    if let Ok(content) = fs::read_to_string(path) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext == "rs" {
            let syms = extract_rust_symbols(&content, deep);
            if !syms.is_empty() {
                return syms;
            }
        }
        extract_multiline_symbols(&content, &ext, deep)
    } else {
        Vec::new()
    }
}

pub fn extract_rust_symbols(content: &str, deep: bool) -> Vec<String> {
    let mut symbols = Vec::new();
    let file = match syn::parse_file(content) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let max_symbols = if deep { 30 } else { 10 };

    for item in file.items {
        match item {
            syn::Item::Fn(f) => {
                if is_syn_pub(&f.vis) {
                    let vis = &f.vis;
                    let sig = &f.sig;
                    let full = quote::quote!(#vis #sig).to_string();
                    let cleaned = normalize_signature_whitespace(&full);
                    if let Some(sanitized) = sanitize_symbol(&cleaned) {
                        if !symbols.contains(&sanitized) {
                            symbols.push(sanitized);
                        }
                    }
                }
            }
            syn::Item::Struct(s) => {
                if is_syn_pub(&s.vis) {
                    let vis = &s.vis;
                    let ident = &s.ident;
                    let generics = &s.generics;
                    let full = quote::quote!(#vis struct #ident #generics).to_string();
                    let cleaned = normalize_signature_whitespace(&full);
                    if let Some(sanitized) = sanitize_symbol(&cleaned) {
                        if !symbols.contains(&sanitized) {
                            symbols.push(sanitized);
                        }
                    }
                }
            }
            syn::Item::Enum(e) => {
                if is_syn_pub(&e.vis) {
                    let vis = &e.vis;
                    let ident = &e.ident;
                    let generics = &e.generics;
                    let full = quote::quote!(#vis enum #ident #generics).to_string();
                    let cleaned = normalize_signature_whitespace(&full);
                    if let Some(sanitized) = sanitize_symbol(&cleaned) {
                        if !symbols.contains(&sanitized) {
                            symbols.push(sanitized);
                        }
                    }
                }
            }
            syn::Item::Trait(t) => {
                if is_syn_pub(&t.vis) {
                    let vis = &t.vis;
                    let ident = &t.ident;
                    let generics = &t.generics;
                    let full = quote::quote!(#vis trait #ident #generics).to_string();
                    let cleaned = normalize_signature_whitespace(&full);
                    if let Some(sanitized) = sanitize_symbol(&cleaned) {
                        if !symbols.contains(&sanitized) {
                            symbols.push(sanitized);
                        }
                    }
                    if deep {
                        for trait_item in &t.items {
                            if let syn::TraitItem::Fn(trait_fn) = trait_item {
                                let sig = &trait_fn.sig;
                                let full = quote::quote!(#sig).to_string();
                                let cleaned =
                                    format!("    {}", normalize_signature_whitespace(&full));
                                if let Some(sanitized) = sanitize_symbol(&cleaned) {
                                    if !symbols.contains(&sanitized) {
                                        symbols.push(sanitized);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            syn::Item::Type(t) => {
                if is_syn_pub(&t.vis) {
                    let vis = &t.vis;
                    let ident = &t.ident;
                    let generics = &t.generics;
                    let ty = &t.ty;
                    let full = quote::quote!(#vis type #ident #generics = #ty).to_string();
                    let cleaned = normalize_signature_whitespace(&full);
                    if let Some(sanitized) = sanitize_symbol(&cleaned) {
                        if !symbols.contains(&sanitized) {
                            symbols.push(sanitized);
                        }
                    }
                }
            }
            syn::Item::Const(c) => {
                if is_syn_pub(&c.vis) {
                    let vis = &c.vis;
                    let ident = &c.ident;
                    let ty = &c.ty;
                    let expr = &c.expr;
                    let full = quote::quote!(#vis const #ident: #ty = #expr).to_string();
                    let cleaned = normalize_signature_whitespace(&full);
                    if let Some(sanitized) = sanitize_symbol(&cleaned) {
                        if !symbols.contains(&sanitized) {
                            symbols.push(sanitized);
                        }
                    }
                }
            }
            syn::Item::Impl(imp) if deep => {
                for item in &imp.items {
                    if let syn::ImplItem::Fn(method) = item {
                        if is_syn_pub(&method.vis) {
                            let vis = &method.vis;
                            let sig = &method.sig;
                            let full = quote::quote!(#vis #sig).to_string();
                            let cleaned = normalize_signature_whitespace(&full);
                            if let Some(sanitized) = sanitize_symbol(&cleaned) {
                                if !symbols.contains(&sanitized) {
                                    symbols.push(sanitized);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        if symbols.len() >= max_symbols {
            break;
        }
    }

    symbols
}

fn is_syn_pub(vis: &syn::Visibility) -> bool {
    matches!(
        vis,
        syn::Visibility::Public(_) | syn::Visibility::Restricted(_)
    )
}

fn normalize_signature_whitespace(s: &str) -> String {
    let mut clean = s
        .replace(" < ", "<")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace(" > ", "> ")
        .replace(" ( ", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" : ", ": ")
        .replace(" ;", ";")
        .replace(" , ", ", ")
        .replace(" ,", ", ")
        .replace("& mut ", "&mut ")
        .replace(" & mut ", " &mut ")
        .replace("& '", "&'")
        .replace(" & '", " &'")
        .replace("( &", "(&")
        .replace("(& ", "(&")
        .replace("& ", "&")
        .replace(" & ", " &");

    clean = clean.replace("(& ", "(&");
    clean = clean.replace("( &", "(&");

    let words: Vec<&str> = clean.split_whitespace().collect();
    let joined = words.join(" ");
    joined
        .replace(" (", "(")
        .replace(":(", ": (")
        .replace("->(", "-> (")
        .replace("+(", "+ (")
}

pub fn extract_multiline_symbols(content: &str, ext: &str, deep: bool) -> Vec<String> {
    let mut symbols = Vec::new();
    let max_lines = if deep { usize::MAX } else { 200 };
    let max_symbols = if deep { 30 } else { 10 };

    let lines: Vec<&str> = content.lines().take(max_lines).collect();
    let mut idx = 0;

    while idx < lines.len() {
        let trimmed = lines[idx].trim();

        if is_symbol_start(trimmed, ext) {
            let mut accumulated = String::new();
            let mut parens: i32 = 0;
            let mut brackets: i32 = 0;
            let mut line_count = 0;

            while idx < lines.len() && line_count < 12 {
                let current_line = lines[idx].trim();
                if !accumulated.is_empty() {
                    accumulated.push(' ');
                }
                accumulated.push_str(current_line);

                for ch in current_line.chars() {
                    match ch {
                        '(' => parens += 1,
                        ')' => parens = (parens - 1).max(0),
                        '[' => brackets += 1,
                        ']' => brackets = (brackets - 1).max(0),
                        _ => {}
                    }
                }

                line_count += 1;
                idx += 1;

                if ext == "py" {
                    if parens == 0 && brackets == 0 && current_line.ends_with(':') {
                        break;
                    }
                } else if parens == 0
                    && brackets == 0
                    && (current_line.contains('{') || current_line.ends_with(';'))
                {
                    break;
                }
            }

            let cleaned = clean_multiline_signature(&accumulated, ext);
            if !cleaned.is_empty() {
                if let Some(sanitized) = sanitize_symbol(&cleaned) {
                    if !symbols.contains(&sanitized) {
                        symbols.push(sanitized);
                    }
                }
            }

            if symbols.len() >= max_symbols {
                break;
            }
        } else {
            idx += 1;
        }
    }

    symbols
}

fn is_symbol_start(line: &str, ext: &str) -> bool {
    match ext {
        "ts" | "tsx" | "js" | "jsx" => {
            line.starts_with("export async function ")
                || line.starts_with("export function ")
                || line.starts_with("export class ")
                || line.starts_with("export abstract class ")
                || line.starts_with("export interface ")
                || line.starts_with("export type ")
                || line.starts_with("export enum ")
                || line.starts_with("export const ")
        }
        "py" => {
            (line.starts_with("def ")
                || line.starts_with("async def ")
                || line.starts_with("class "))
                && !line.starts_with("def _")
                && !line.starts_with("async def _")
        }
        "go" => {
            (line.starts_with("func ") || line.starts_with("type "))
                && line
                    .chars()
                    .nth(5)
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
        }
        "rs" => {
            line.starts_with("pub struct ")
                || line.starts_with("pub enum ")
                || line.starts_with("pub trait ")
                || line.starts_with("pub fn ")
                || line.starts_with("pub async fn ")
                || line.starts_with("pub type ")
                || line.starts_with("pub const ")
        }
        _ => {
            line.starts_with("pub ")
                || line.starts_with("export ")
                || line.starts_with("public ")
                || line.starts_with("class ")
                || line.starts_with("def ")
                || line.starts_with("func ")
        }
    }
}

fn find_param_parens(s: &str) -> Option<(usize, usize)> {
    let start_search = if s.starts_with("func (") {
        // Go method receiver: skip receiver (recv Type)
        let mut depth = 0;
        let mut recv_end = 0;
        for (i, ch) in s.char_indices() {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    recv_end = i + 1;
                    break;
                }
            }
        }
        recv_end
    } else {
        0
    };

    let open_idx = s[start_search..].find('(').map(|i| start_search + i)?;
    let mut depth = 0;
    for (i, ch) in s[open_idx..].char_indices() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth == 0 {
                return Some((open_idx, open_idx + i));
            }
        }
    }
    None
}

fn clean_multiline_signature(raw: &str, ext: &str) -> String {
    let mut sig = raw.trim();
    if ext == "py" {
        sig = sig.trim_end_matches(':').trim();
    } else {
        if let Some(pos) = sig.find('{') {
            sig = &sig[..pos];
        }
        sig = sig.trim_end_matches(';').trim();
    }
    let words: Vec<&str> = sig.split_whitespace().collect();
    let joined = words.join(" ");
    if joined.len() > 280 {
        if let Some((open_idx, close_idx)) = find_param_parens(&joined) {
            let truncated = format!(
                "{}(...){}",
                joined[..open_idx].trim_end(),
                &joined[close_idx + 1..]
            );
            if truncated.len() > 280 {
                let mut end = 277;
                while !truncated.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &truncated[..end])
            } else {
                truncated
            }
        } else {
            let mut end = 277;
            while !joined.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &joined[..end])
        }
    } else {
        joined
    }
}

#[allow(dead_code)]
pub fn clean_symbol_line(line: &str) -> String {
    let without_brace = line.split('{').next().unwrap_or(line).trim();
    let without_semicolon = without_brace.trim_end_matches(';').trim();
    if without_semicolon.len() > 120 {
        format!("{}...", &without_semicolon[..117])
    } else {
        without_semicolon.to_string()
    }
}

pub fn sanitize_symbol(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_private_key(trimmed) {
        return None;
    }
    let redacted_assign = redact_sensitive_assignment(trimmed);
    let fully_redacted = redact_secret_patterns(&redacted_assign);
    Some(fully_redacted)
}

fn is_private_key(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("-----begin")
        || lower.contains("private key")
        || lower.contains("ssh-rsa")
        || lower.contains("ssh-ed25519")
        || lower.contains("pgp private")
}

fn redact_sensitive_assignment(s: &str) -> String {
    if let Some(eq_idx) = s.find('=') {
        let left = &s[..eq_idx];
        let right = s[eq_idx + 1..].trim();
        let upper_left = left.to_uppercase();

        let sensitive_words = &[
            "API_KEY",
            "SECRET",
            "TOKEN",
            "PASSWORD",
            "PASSWD",
            "AUTH",
            "CREDENTIAL",
            "PRIVATE_KEY",
            "ACCESS_KEY",
        ];

        let has_sensitive_word = sensitive_words.iter().any(|&w| upper_left.contains(w));
        let is_quoted = right.starts_with('"') || right.starts_with('\'');

        if has_sensitive_word && is_quoted {
            let left_trimmed = left.trim_end();
            return format!("{} = \"[REDACTED]\"", left_trimmed);
        }
    }
    s.to_string()
}

fn redact_secret_patterns(s: &str) -> String {
    let mut result = s.to_string();

    let prefixes = &[
        "sk-", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "AKIA", "AIza", "eyJh",
    ];
    for &prefix in prefixes {
        while let Some(start_idx) = result.find(prefix) {
            let after = &result[start_idx..];
            let token_len = after
                .find(|c: char| {
                    c.is_whitespace()
                        || c == '"'
                        || c == '\''
                        || c == ';'
                        || c == ','
                        || c == ')'
                        || c == '}'
                        || c == ']'
                })
                .unwrap_or(after.len());

            if token_len >= prefix.len() + 6 {
                result.replace_range(start_idx..start_idx + token_len, "[REDACTED]");
            } else {
                break;
            }
        }
    }

    let lower = result.to_lowercase();
    if let Some(bearer_idx) = lower.find("bearer ") {
        let after_bearer = &result[bearer_idx + 7..];
        let token_len = after_bearer
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ';' || c == ',')
            .unwrap_or(after_bearer.len());
        if token_len > 4 {
            result.replace_range(bearer_idx + 7..bearer_idx + 7 + token_len, "[REDACTED]");
        }
    }

    result
}

fn generate_default_rules(pt: &ProjectType, with_rtk: bool) -> Vec<String> {
    match pt {
        ProjectType::Rust => {
            let clippy = if with_rtk {
                "rtk cargo clippy --all-targets"
            } else {
                "cargo clippy --all-targets"
            };
            let fmt = if with_rtk {
                "rtk cargo fmt --check"
            } else {
                "cargo fmt --check"
            };
            vec![
                "Все внешние крейты и зависимости строго объявляются в Cargo.toml.".to_string(),
                format!("Код должен проходить `{}` и `{}` без ошибок.", clippy, fmt),
                "Использование `unsafe` запрещено без исчерпывающего комментария безопасности `// SAFETY:`.".to_string(),
                "Публичные структуры и функции должны сопровождаться docstring (`///`).".to_string(),
            ]
        }
        ProjectType::TypeScript => {
            let fmt_lint = if with_rtk {
                "rtk biome / rtk prettier"
            } else {
                "biome / prettier"
            };
            vec![
                "Строгая типизация (`TypeScript strict: true`) обязательна, запрещен `any`."
                    .to_string(),
                "Все зависимости фиксируются строго в lockfile (`pnpm-lock.yaml` / `bun.lock`)."
                    .to_string(),
                format!(
                    "Форматирование и линтинг через `{}` без отключения проверок.",
                    fmt_lint
                ),
            ]
        }
        ProjectType::Python => {
            let ruff_check = if with_rtk {
                "rtk ruff check"
            } else {
                "ruff check"
            };
            let ruff_fmt = if with_rtk {
                "rtk ruff format"
            } else {
                "ruff format"
            };
            vec![
                "Все зависимости фиксируются через `pyproject.toml` (uv/poetry).".to_string(),
                "Обязательны type hints для аргументов и возвращаемых значений функций."
                    .to_string(),
                format!("Код валидируется через `{}` и `{}`.", ruff_check, ruff_fmt),
            ]
        }
        ProjectType::Go => {
            let gofmt = if with_rtk { "rtk gofmt" } else { "gofmt" };
            let golangci = if with_rtk {
                "rtk golangci-lint"
            } else {
                "golangci-lint"
            };
            vec![
                "Структура пакетов должна следовать канону: `cmd/`, `internal/`, `pkg/`."
                    .to_string(),
                "Все ошибки должны обрабатываться явно (никаких проигнорированных `err`)."
                    .to_string(),
                format!(
                    "Соблюдать форматирование `{}` и проверки `{}`.",
                    gofmt, golangci
                ),
            ]
        }
        ProjectType::DevOps => vec![
            "Вся инфраструктура строго декларативна (IaC), запрещены ручные мутации в проде."
                .to_string(),
            "Все пароли и API-ключи шифруются через SOPS / Age / Vault, запрещен коммит `.env`."
                .to_string(),
        ],
        ProjectType::Generic => vec![
            "Поддерживать актуальность карт модулей и документации архитектуры.".to_string(),
            "Все изменения фиксируются через атомарные и типизированные коммиты.".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_workspace() {
        let toml_sample = r#"
[workspace]
members = [
    "crates/core",
    "crates/cli",
]

[package]
name = "my-workspace"
version = "0.1.0"
"#;
        let temp_dir = std::env::temp_dir().join("test_cargo_ws");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("Cargo.toml");
        fs::write(&file_path, toml_sample).unwrap();

        let (name, members) = parse_cargo_toml(&file_path);
        assert_eq!(name, Some("my-workspace".to_string()));
        assert_eq!(members, vec!["crates/core", "crates/cli"]);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_package_json() {
        let pkg_sample = r#"{
  "name": "@my-org/web",
  "workspaces": ["packages/*", "apps/*"]
}"#;
        let temp_dir = std::env::temp_dir().join("test_pkg_ws");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("package.json");
        fs::write(&file_path, pkg_sample).unwrap();

        let (name, members) = parse_package_json(&file_path);
        assert_eq!(name, Some("@my-org/web".to_string()));
        assert_eq!(members, vec!["packages/*", "apps/*"]);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_pnpm_workspace_yaml() {
        let pnpm_sample = r#"# PNPM Workspace Configuration
packages:
  # Internal plugins
  - 'plugins/*' # inline comment
  - "packages/*"
  - apps/web
  - '!**/tests/**'
# Column 0 comment
  - modules/legacy

catalog:
  typescript: 7.0.2
"#;
        let temp_dir = std::env::temp_dir().join("test_pnpm_ws");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("pnpm-workspace.yaml");
        fs::write(&file_path, pnpm_sample).unwrap();

        let members = parse_pnpm_workspace_yaml(&file_path);
        assert_eq!(
            members,
            vec!["plugins/*", "packages/*", "apps/web", "modules/legacy"]
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_clean_symbol_line() {
        assert_eq!(
            clean_symbol_line("pub fn process_data(input: &str) -> Result<()> {"),
            "pub fn process_data(input: &str) -> Result<()>"
        );
        assert_eq!(
            clean_symbol_line("export class ContextEngine {"),
            "export class ContextEngine"
        );
    }

    #[test]
    fn test_rust_syn_extraction() {
        let code = r#"
pub struct Engine<T: Clone> {
    data: T,
}

pub enum Status {
    Active,
    Idle,
}

pub async fn run_task<T>(id: &str, payload: T) -> Result<Status, Error> {
    Ok(Status::Active)
}
"#;
        let symbols = extract_rust_symbols(code, true);
        assert!(symbols
            .iter()
            .any(|s| s.contains("struct Engine < T : Clone >")
                || s.contains("struct Engine<T: Clone>")));
        assert!(symbols.iter().any(|s| s.contains("enum Status")));
        assert!(symbols.iter().any(|s| s.contains("run_task")
            && s.contains("Result < Status , Error >")
            || s.contains("Result<Status, Error>")));
    }

    #[test]
    fn test_multiline_ts_extraction() {
        let code = r#"
export async function initializeClient<T>(
    endpoint: string,
    timeoutMs: number
): Promise<Client<T>> {
    return new Client();
}
"#;
        let symbols = extract_multiline_symbols(code, "ts", true);
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].contains("initializeClient<T>"));
        assert!(symbols[0].contains("endpoint: string"));
        assert!(symbols[0].contains("timeoutMs: number"));
        assert!(symbols[0].contains("Promise<Client<T>>"));
    }

    #[test]
    fn test_multiline_python_extraction() {
        let code = r#"
def execute_pipeline(
    config: PipelineConfig,
    debug: bool = False,
) -> ExecutionResult:
    pass
"#;
        let symbols = extract_multiline_symbols(code, "py", true);
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].contains("execute_pipeline"));
        assert!(symbols[0].contains("config: PipelineConfig"));
        assert!(symbols[0].contains("ExecutionResult"));
    }

    #[test]
    fn test_secret_sanitizer() {
        assert_eq!(
            sanitize_symbol("pub const API_KEY: &str = \"sk-1234567890abcdef123456\";"),
            Some("pub const API_KEY: &str = \"[REDACTED]\"".to_string())
        );
        assert_eq!(
            sanitize_symbol("export const GITHUB_TOKEN = \"ghp_1234567890abcdef1234567890\";"),
            Some("export const GITHUB_TOKEN = \"[REDACTED]\"".to_string())
        );
        assert_eq!(
            sanitize_symbol(
                "export const AUTH = \"Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.xyz\";"
            ),
            Some("export const AUTH = \"[REDACTED]\"".to_string())
        );
        assert_eq!(
            sanitize_symbol("pub fn set_header(val: &str = \"Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.xyz\")"),
            Some("pub fn set_header(val: &str = \"Bearer [REDACTED]\")".to_string())
        );
        assert_eq!(sanitize_symbol("-----BEGIN RSA PRIVATE KEY-----"), None);
    }

    #[test]
    fn test_rtk_prefixing() {
        assert_eq!(prefix_with_rtk("cargo test"), "rtk cargo test");
        assert_eq!(prefix_with_rtk("rtk cargo test"), "rtk cargo test");
        assert_eq!(prefix_with_rtk(""), "");
    }

    #[test]
    fn test_count_tokens() {
        let tokens = count_tokens("Hello world! Context-init BPE tokenometry.");
        assert!(tokens > 0);
    }

    #[test]
    fn test_multiline_go_extraction() {
        let code = r#"
func ProcessRequest(
    ctx context.Context,
    req *DataRequest,
) (*DataResponse, error) {
    return nil, nil
}
"#;
        let symbols = extract_multiline_symbols(code, "go", true);
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].contains("func ProcessRequest"));
        assert!(symbols[0].contains("ctx context.Context"));
        assert!(symbols[0].contains("(*DataResponse, error)"));
    }

    #[test]
    fn test_rust_impl_and_trait_extraction() {
        let code = r#"
pub trait Service {
    fn handle(&self, req: Request) -> Response;
}

pub struct MyService;

impl MyService {
    pub fn new() -> Self {
        Self
    }
    fn private_helper(&self) {}
}
"#;
        let symbols = extract_rust_symbols(code, true);
        assert!(symbols.iter().any(|s| s.contains("trait Service")));
        assert!(symbols
            .iter()
            .any(|s| s.contains("fn handle") && s.contains("&self") && s.contains("Response")));
        assert!(symbols.iter().any(|s| s.contains("struct MyService")));
        assert!(symbols
            .iter()
            .any(|s| s.contains("pub fn new") && s.contains("-> Self")));
        // private_helper should not be extracted
        assert!(!symbols.iter().any(|s| s.contains("private_helper")));
    }

    #[test]
    fn test_secret_sanitizer_edge_cases() {
        // SSH key
        assert_eq!(
            sanitize_symbol("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI..."),
            None
        );
        // PGP Private
        assert_eq!(
            sanitize_symbol("-----BEGIN PGP PRIVATE KEY BLOCK-----"),
            None
        );
        // Slack token
        let slack = "pub const SLACK_TOKEN: &str = \"xoxb-1234567890-1234567890123-abcdef\";";
        assert_eq!(
            sanitize_symbol(slack),
            Some("pub const SLACK_TOKEN: &str = \"[REDACTED]\"".to_string())
        );
        // Regular harmless const
        let normal = "pub const MAX_RETRIES: usize = 5;";
        assert_eq!(
            sanitize_symbol(normal),
            Some("pub const MAX_RETRIES: usize = 5;".to_string())
        );
    }

    #[test]
    fn test_normalize_signature_whitespace() {
        let sig = "pub fn count_tokens ( text : & str ) -> usize";
        assert_eq!(
            normalize_signature_whitespace(sig),
            "pub fn count_tokens(text: &str) -> usize"
        );

        let sig_method = "pub fn parse < 'a > ( input : & 'a str ) -> ( & 'a str , usize )";
        assert_eq!(
            normalize_signature_whitespace(sig_method),
            "pub fn parse<'a>(input: &'a str) -> (&'a str, usize)"
        );

        let sig_mut = "pub fn mutate ( & mut self , val : & mut String ) -> Result < ( ) , Error >";
        assert_eq!(
            normalize_signature_whitespace(sig_mut),
            "pub fn mutate(&mut self, val: &mut String) -> Result<(), Error>"
        );

        let sig_trait = "pub fn bound ( ) -> impl Trait + ( Send )";
        assert_eq!(
            normalize_signature_whitespace(sig_trait),
            "pub fn bound() -> impl Trait + (Send)"
        );
    }

    #[test]
    fn test_clean_multiline_signature_truncation() {
        let long_ts = format!(
            "export async function processData(param1: {}, param2: String): Promise<Result>",
            "A".repeat(300)
        );
        let cleaned = clean_multiline_signature(&long_ts, "ts");
        assert_eq!(
            cleaned,
            "export async function processData(...): Promise<Result>"
        );

        let long_py = format!(
            "async def handle_request(arg1: {}, arg2: int) -> bool:",
            "B".repeat(300)
        );
        let cleaned_py = clean_multiline_signature(&long_py, "py");
        assert_eq!(cleaned_py, "async def handle_request(...) -> bool");
    }

    #[test]
    fn test_ignored_dirs_in_subsystem_discovery() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_ignored_dirs_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&temp_dir);

        // Create ignored directories with files in them
        let ignored_test_dirs = [
            ".vault",
            ".turbo",
            "out",
            ".svelte-kit",
            ".astro",
            ".parcel-cache",
            "node_modules",
            "target",
        ];
        for dir_name in &ignored_test_dirs {
            let dir_path = temp_dir.join(dir_name);
            let _ = fs::create_dir_all(&dir_path);
            let _ = fs::write(dir_path.join("file.ts"), "export const x = 1;");
            let _ = fs::write(dir_path.join("main.rs"), "fn main() {}");

            // Also test nested directories inside ignored dirs
            let nested_dir = dir_path.join("nested_sub");
            let _ = fs::create_dir_all(&nested_dir);
            let _ = fs::write(nested_dir.join("sub.rs"), "pub fn sub() {}");
        }

        // Test ignored directories inside packages/
        let pkg_vault = temp_dir.join("packages").join(".vault");
        let _ = fs::create_dir_all(&pkg_vault);
        let _ = fs::write(pkg_vault.join("plugin.ts"), "export const p = 1;");

        // Create a legitimate subsystem directory
        let legit_dir = temp_dir.join("src").join("core");
        let _ = fs::create_dir_all(&legit_dir);
        let _ = fs::write(legit_dir.join("mod.rs"), "pub fn run() {}");

        // Also test root-level file with workspace members pointing into ignored directories
        let _ = fs::write(
            temp_dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\".vault/nested_sub\", \"packages/.vault\"]\n\n[package]\nname = \"dummy\"\nversion = \"0.1.0\"\n",
        );

        let analysis = analyze_project(&temp_dir, false, false).expect("Analysis should succeed");

        assert!(
            !analysis.detected_modules.is_empty(),
            "Legitimate subsystems must be discovered"
        );
        assert!(
            analysis
                .detected_modules
                .iter()
                .any(|m| m.name == "core" || m.dir_path == "src/core"),
            "Module 'core' should have been discovered"
        );

        for m in &analysis.detected_modules {
            for ignored in &ignored_test_dirs {
                assert!(
                    !m.dir_path.split('/').any(|seg| seg == *ignored) && !m.name.contains(ignored),
                    "Module {} (path: {}) should not have matched ignored dir {}",
                    m.name,
                    m.dir_path,
                    ignored
                );
            }
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_workspace_root_dot_member_does_not_create_empty_subsystem() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_ws_dot_member_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let crate_dir = temp_dir.join("crates").join("ci-core").join("src");
        let root_src = temp_dir.join("src");
        let _ = fs::create_dir_all(&crate_dir);
        let _ = fs::create_dir_all(&root_src);

        let _ = fs::write(
            temp_dir.join("Cargo.toml"),
            "[workspace]\nresolver = \"2\"\nmembers = [\"crates/ci-core\", \".\"]\n\n[package]\nname = \"context-init\"\nversion = \"0.3.0\"\n",
        );
        let _ = fs::write(
            temp_dir.join("crates").join("ci-core").join("Cargo.toml"),
            "[package]\nname = \"ci-core\"\nversion = \"0.3.0\"\n",
        );
        let _ = fs::write(crate_dir.join("lib.rs"), "pub fn core_fn() {}");
        let _ = fs::write(root_src.join("main.rs"), "fn main() {}");

        let analysis = analyze_project(&temp_dir, false, false).expect("Analysis should succeed");
        assert!(
            analysis
                .detected_modules
                .iter()
                .all(|m| !m.name.is_empty() && !m.dir_path.is_empty() && m.dir_path != "."),
            "No module should have an empty name or '.' dir_path: {:?}",
            analysis.detected_modules
        );
        assert!(
            analysis
                .detected_modules
                .iter()
                .any(|m| m.dir_path == "crates/ci-core"),
            "crates/ci-core must be discovered"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_flat_src_merges_main_and_lib_into_single_core_module() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_flat_src_core_merge_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root_src = temp_dir.join("src");
        let _ = fs::create_dir_all(&root_src);

        let _ = fs::write(
            temp_dir.join("Cargo.toml"),
            "[package]\nname = \"dual-target\"\nversion = \"0.3.0\"\n",
        );
        let _ = fs::write(
            root_src.join("main.rs"),
            "pub fn run_cli() {}\nfn main() {}",
        );
        let _ = fs::write(root_src.join("lib.rs"), "pub fn run_lib() {}");
        let _ = fs::write(root_src.join("detector.rs"), "pub fn detect() {}");
        let _ = fs::write(root_src.join("service.rs"), "pub fn serve() {}");

        let analysis = analyze_project(&temp_dir, false, false).expect("Analysis should succeed");
        let core_modules: Vec<_> = analysis
            .detected_modules
            .iter()
            .filter(|m| m.dir_path == "src/core")
            .collect();

        assert_eq!(
            core_modules.len(),
            1,
            "main.rs and lib.rs must be merged into a single src/core module, got: {:?}",
            core_modules
        );
        let core = core_modules[0];
        assert_eq!(core.name, "core");
        assert_eq!(core.files.len(), 2);
        assert!(core.files.iter().any(|f| f.path == "src/lib.rs"));
        assert!(core.files.iter().any(|f| f.path == "src/main.rs"));
        assert!(core.entrypoints.contains(&"src/lib.rs".to_string()));
        assert!(core.entrypoints.contains(&"src/main.rs".to_string()));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_cargo_toml_workspace_inheritance_and_sections() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_parse_cargo_ws_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let subcrate_dir = temp_dir.join("crates").join("ci-core");
        let _ = fs::create_dir_all(subcrate_dir.join("src"));

        let root_toml = temp_dir.join("Cargo.toml");
        let _ = fs::write(
            &root_toml,
            "[workspace]\nresolver = \"2\"\nmembers = [\n    \"crates/ci-core\", # core crate\n    \".\",\n]\n\n[workspace.package]\nversion = \"0.3.0\"\nedition = \"2021\"\n\n[workspace.metadata.custom]\nname = \"should-not-match-metadata-name\"\n\n[package]\nname = \"context-init\"\nversion.workspace = true\nedition.workspace = true\n",
        );

        let subcrate_toml = subcrate_dir.join("Cargo.toml");
        let _ = fs::write(
            &subcrate_toml,
            "[package]\nname = \"ci-core\"\nversion.workspace = true\nedition.workspace = true\n",
        );
        let _ = fs::write(
            subcrate_dir.join("src").join("lib.rs"),
            "pub const V: &str = \"0.3.0\";",
        );

        let (root_name, members) = parse_cargo_toml(&root_toml);
        assert_eq!(root_name.as_deref(), Some("context-init"));
        assert_eq!(members, vec!["crates/ci-core".to_string(), ".".to_string()]);

        let (sub_name, sub_members) = parse_cargo_toml(&subcrate_toml);
        assert_eq!(sub_name.as_deref(), Some("ci-core"));
        assert!(sub_members.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
