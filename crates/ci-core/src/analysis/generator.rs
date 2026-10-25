use crate::analysis::subsystems::GENERATED_SUMMARY_PREFIXES;
use crate::analysis::ProjectAnalysis;
use crate::storage::{
    now_rfc3339, safe_join, DriftReport, L0Index, SubsystemInfo, CURRENT_L0_SCHEMA_VERSION,
};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Options for [`generate`].
#[derive(Debug, Clone, Copy, Default)]
pub struct GenerateOptions {
    /// Overwrite an existing knowledge base.
    pub force: bool,
    /// Validate only; write nothing.
    pub dry_run: bool,
    /// Create or refresh the navigation section of the repo's `AGENTS.md`.
    pub with_agents_md: bool,
}

/// Writes a fresh L0–L2 knowledge base into `knowledge_dir`. Only `AGENTS.md` goes into
/// the repository (`analysis.root_dir`).
pub fn generate(
    analysis: &ProjectAnalysis,
    subsystems: &[SubsystemInfo],
    knowledge_dir: &Path,
    opts: &GenerateOptions,
) -> Result<()> {
    let GenerateOptions {
        force,
        dry_run,
        with_agents_md,
    } = *opts;
    let base_dir = knowledge_dir.to_path_buf();
    let subsystems_dir = base_dir.join("subsystems");
    let cli_dir = base_dir.join("cli");
    let adr_dir = base_dir.join("adr");

    if base_dir.exists() && !force {
        anyhow::bail!(
            "Директория {:?} уже существует. Используйте флаг --force для перезаписи или команду update.",
            base_dir
        );
    }
    if dry_run {
        return Ok(());
    }

    if force && subsystems_dir.exists() {
        if let Ok(entries) = fs::read_dir(&subsystems_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md") {
                    let _ = fs::remove_file(p);
                }
            }
        }
    }

    fs::create_dir_all(&subsystems_dir)
        .with_context(|| format!("Не удалось создать {:?}", subsystems_dir))?;
    fs::create_dir_all(&cli_dir).with_context(|| format!("Не удалось создать {:?}", cli_dir))?;
    fs::create_dir_all(&adr_dir).with_context(|| format!("Не удалось создать {:?}", adr_dir))?;

    let today = chrono_like_date();

    // 1. Создаем L0_index.json
    let l0_index = L0Index {
        schema: "https://json-schema.org/draft/2020-12/schema".to_string(),
        schema_version: CURRENT_L0_SCHEMA_VERSION,
        project_id: None,
        project: analysis.name.clone(),
        description: analysis.description.clone(),
        updated_at: today.clone(),
        project_type: analysis.project_type.display_name().to_string(),
        entrypoints: analysis.root_entrypoints.clone(),
        commands: analysis.commands.clone(),
        subsystems: subsystems.to_vec(),
        core_rules: analysis.rules.clone(),
        stow_packages: Vec::new(),
    };

    let l0_json = format!("{}\n", serde_json::to_string_pretty(&l0_index)?);
    let l0_json_path = base_dir.join("L0_index.json");
    fs::write(&l0_json_path, l0_json)
        .with_context(|| format!("Не удалось записать {:?}", l0_json_path))?;

    // 2. Создаем компактный L0_index.md (оптимальный для контекстного окна LLM)
    let l0_md_content = render_l0_markdown(&l0_index, &today);
    let l0_md_path = base_dir.join("L0_index.md");
    fs::write(&l0_md_path, l0_md_content)
        .with_context(|| format!("Не удалось записать {:?}", l0_md_path))?;

    // 3. Создаем карточки подсистем L1
    for sub in subsystems {
        let sub_file = base_dir.join(&sub.l1_path);
        let content = render_subsystem_card(sub, analysis);
        fs::write(&sub_file, content)
            .with_context(|| format!("Не удалось записать {:?}", sub_file))?;
    }

    // 4. Создаем актуальный справочник команд L2 (cli/commands.md)
    let cli_path = cli_dir.join("commands.md");
    let cli_content = render_commands_markdown(analysis);
    fs::write(&cli_path, cli_content)
        .with_context(|| format!("Не удалось записать {:?}", cli_path))?;

    // 5. Создаем начальный ADR (adr/0001_initial_architecture.md)
    let adr_path = adr_dir.join("0001_initial_architecture.md");
    let adr_content = render_adr_markdown(analysis, subsystems, &today);
    fs::write(&adr_path, adr_content)
        .with_context(|| format!("Не удалось записать {:?}", adr_path))?;

    // 6. Опционально генерируем / обновляем корневой AGENTS.md
    if with_agents_md {
        update_or_create_root_agents_md(&analysis.root_dir, &analysis.name, subsystems)?;
    }

    Ok(())
}

/// Compares the knowledge base in `knowledge_dir` against the analyzed source tree.
pub fn audit(analysis: &ProjectAnalysis, knowledge_dir: &Path) -> Result<DriftReport> {
    let base_dir = knowledge_dir.to_path_buf();
    let l0_path = base_dir.join("L0_index.json");
    if !l0_path.exists() {
        anyhow::bail!(
            "Knowledge base not found at {:?}. Run `context-init init` first.",
            l0_path
        );
    }
    let index = L0Index::load_from_file(&l0_path)?;

    let existing_sub_ids: Vec<String> = index.subsystems.iter().map(|s| s.id.clone()).collect();
    let fs_module_ids: Vec<String> = analysis
        .detected_modules
        .iter()
        .map(|m| m.id.clone())
        .collect();

    let mut missing_in_fs = Vec::new();
    let mut up_to_date_count = 0;

    for sub in &index.subsystems {
        let exists = if sub.parent.is_some() {
            // A split-off module lives only while its crate is still split.
            fs_module_ids.contains(&sub.id)
        } else if !sub.files.is_empty() {
            sub.files
                .iter()
                .any(|f| analysis.root_dir.join(&f.path).exists())
        } else if !sub.key_files.is_empty() {
            sub.key_files
                .iter()
                .any(|kf| analysis.root_dir.join(kf).exists())
        } else {
            let sub_fs_dir = analysis.root_dir.join(&sub.id);
            let sub_src_file_rs = analysis.root_dir.join("src").join(format!("{}.rs", sub.id));
            let sub_src_file_ts = analysis.root_dir.join("src").join(format!("{}.ts", sub.id));
            let sub_src_file_py = analysis.root_dir.join("src").join(format!("{}.py", sub.id));
            let sub_src_file_go = analysis.root_dir.join("src").join(format!("{}.go", sub.id));
            let sub_l1_file = base_dir.join(&sub.l1_path);
            sub_fs_dir.exists()
                || sub_src_file_rs.exists()
                || sub_src_file_ts.exists()
                || sub_src_file_py.exists()
                || sub_src_file_go.exists()
                || ((sub.id == "core" || sub.id == "src_core")
                    && (analysis.root_dir.join("src/main.rs").exists()
                        || analysis.root_dir.join("src/lib.rs").exists()))
                || fs_module_ids.contains(&sub.id)
                || fs_module_ids.contains(&format!("src_{}", sub.id))
                || sub_l1_file.exists()
        };

        if !exists {
            missing_in_fs.push(sub.id.clone());
        } else {
            up_to_date_count += 1;
        }
    }

    let mut untracked_in_knowledge = Vec::new();
    for fs_id in fs_module_ids {
        let is_tracked = existing_sub_ids.contains(&fs_id)
            || existing_sub_ids.iter().any(|ex| {
                fs_id == format!("src_{}", ex)
                    || ex == fs_id.trim_start_matches("src_")
                    || ex == &fs_id
            });
        if !is_tracked {
            untracked_in_knowledge.push(fs_id);
        }
    }

    Ok(DriftReport {
        missing_in_fs,
        untracked_in_knowledge,
        up_to_date_count,
    })
}

/// Incrementally merges `new_subsystems` into the knowledge base in `knowledge_dir`
/// (creating it if missing). Returns `(added, updated)`.
pub fn update(
    analysis: &ProjectAnalysis,
    new_subsystems: &[SubsystemInfo],
    knowledge_dir: &Path,
    with_agents_md: bool,
) -> Result<(usize, usize)> {
    let base_dir = knowledge_dir.to_path_buf();
    let l0_path = base_dir.join("L0_index.json");

    if !l0_path.exists() {
        // Если базы не было, создаем с нуля
        let opts = GenerateOptions {
            with_agents_md,
            ..GenerateOptions::default()
        };
        generate(analysis, new_subsystems, knowledge_dir, &opts)?;
        return Ok((new_subsystems.len(), 0));
    }

    let mut index = L0Index::load_from_file(&l0_path)?;

    let today = chrono_like_date();
    index.updated_at = today.clone();
    index.commands = analysis.commands.clone();
    index.entrypoints = analysis.root_entrypoints.clone();
    index.core_rules = analysis.rules.clone();

    let mut added_count = 0;
    let mut updated_count = 0;

    for new_sub in new_subsystems {
        if let Some(existing) = index.subsystems.iter_mut().find(|s| {
            s.id == new_sub.id
                || new_sub.id == format!("src_{}", s.id)
                || s.id == new_sub.id.trim_start_matches("src_")
                || (s.id == "core"
                    && (new_sub.id == "src_core"
                        || new_sub.id == "main"
                        || new_sub.id == "src_main"))
        }) {
            // Обновляем технические поля, сохраняя ручное резюме если оно есть
            existing.files = new_sub.files.clone();
            existing.entrypoints = new_sub.entrypoints.clone();
            existing.key_files = new_sub.key_files.clone();
            existing.key_symbols = new_sub.key_symbols.clone();
            existing.test_command = new_sub.test_command.clone();
            existing.tokens = new_sub.tokens;
            existing.parent = new_sub.parent.clone();
            if GENERATED_SUMMARY_PREFIXES
                .iter()
                .any(|p| existing.summary.starts_with(p))
            {
                existing.summary = new_sub.summary.clone();
            }
            updated_count += 1;
        } else {
            // Новая подсистема: генерируем L1 файл
            let sub_file = base_dir.join(&new_sub.l1_path);
            let content = render_subsystem_card(new_sub, analysis);
            let _ = fs::write(sub_file, content);

            index.subsystems.push(new_sub.clone());
            added_count += 1;
        }
    }

    // Split-off modules whose crate is no longer split are gone, cards included.
    let mut superseded = Vec::new();
    index.subsystems.retain(|s| {
        let keep = s.parent.is_none() || new_subsystems.iter().any(|n| n.id == s.id);
        if !keep {
            superseded.push(s.l1_path.clone());
        }
        keep
    });
    for l1_path in superseded {
        if let Some(card) = safe_join(&base_dir, &l1_path) {
            let _ = fs::remove_file(card);
        }
    }

    // Сохраняем обновленные индексы
    let l0_json = format!("{}\n", serde_json::to_string_pretty(&index)?);
    fs::write(&l0_path, l0_json)?;

    let l0_md_content = render_l0_markdown(&index, &today);
    fs::write(base_dir.join("L0_index.md"), l0_md_content)?;

    // Обновляем commands.md
    let cli_path = base_dir.join("cli/commands.md");
    let cli_content = render_commands_markdown(analysis);
    fs::write(cli_path, cli_content)?;

    if with_agents_md {
        update_or_create_root_agents_md(&analysis.root_dir, &analysis.name, &index.subsystems)?;
    }

    Ok((added_count, updated_count))
}

fn render_l0_markdown(index: &L0Index, date: &str) -> String {
    let mut md = format!(
        "# 🗺 Карта архитектуры: {} (L0)\n\n> **Стек:** {} | **Обновлено:** {}\n> **Описание:** {}\n\n",
        index.project, index.project_type, date, index.description
    );

    if !index.entrypoints.is_empty() {
        md.push_str("## 🚪 Точки входа (Root Entrypoints)\n");
        for ep in &index.entrypoints {
            md.push_str(&format!("- `{}`\n", ep));
        }
        md.push('\n');
    }

    md.push_str("## ⚡️ Быстрые команды\n```bash\n");
    if let Some(cmd) = &index.commands.build {
        md.push_str(&format!("# Сборка\n{}\n\n", cmd));
    }
    if let Some(cmd) = &index.commands.test {
        md.push_str(&format!("# Тесты\n{}\n\n", cmd));
    }
    if let Some(cmd) = &index.commands.lint {
        md.push_str(&format!("# Линтер\n{}\n\n", cmd));
    }
    if let Some(cmd) = &index.commands.run {
        md.push_str(&format!("# Запуск\n{}\n\n", cmd));
    }
    md.push_str("```\n\n");

    md.push_str("## 🧩 Каталог подсистем (L1)\n\n");
    md.push_str("| Подсистема | Роль / Назначение | Точки входа | L1 Карточка |\n");
    md.push_str("| :--- | :--- | :--- | :--- |\n");

    for s in &index.subsystems {
        let ep_str = if s.entrypoints.is_empty() {
            "-".to_string()
        } else {
            s.entrypoints.join(", ")
        };
        md.push_str(&format!(
            "| **{}** | {} | `{}` | [{}]({}) |\n",
            s.name, s.summary, ep_str, s.l1_path, s.l1_path
        ));
    }

    md.push_str("\n## 📋 Инварианты и правила проекта\n");
    for rule in &index.core_rules {
        md.push_str(&format!("- {}\n", rule));
    }

    md
}

fn render_subsystem_card(sub: &SubsystemInfo, analysis: &ProjectAnalysis) -> String {
    let mut md = format!(
        "# Подсистема: {} (L1)\n\n> **Идентификатор:** `{}`\n> **Теги:** {}\n\n## 🎯 Назначение и роль\n{}\n\n",
        sub.name,
        sub.id,
        sub.tags.join(", "),
        sub.summary
    );

    // Точки входа и ключевые файлы
    md.push_str("## 📁 Точки входа и структура файлов\n\n");
    if !sub.files.is_empty() {
        md.push_str("| Файл | Статус |\n| :--- | :--- |\n");
        for file in &sub.files {
            let badge = if file.is_entrypoint {
                "⭐ Точка входа"
            } else {
                "Исходный код"
            };
            md.push_str(&format!("| `{}` | {} |\n", file.path, badge));
        }
        md.push('\n');
    } else if !sub.key_files.is_empty() {
        md.push_str("| Файл | Статус |\n| :--- | :--- |\n");
        for file in &sub.key_files {
            let is_ep = sub.entrypoints.iter().any(|ep| ep == file);
            let badge = if is_ep {
                "⭐ Точка входа"
            } else {
                "Исходный код"
            };
            md.push_str(&format!("| `{}` | {} |\n", file, badge));
        }
        md.push('\n');
    } else {
        md.push_str(&format!("- Директория: `{}`\n\n", sub.id));
    }

    // Сигнатуры и символы
    if !sub.key_symbols.is_empty() {
        md.push_str("## 🔑 Ключевой контракт и экспорты\n```text\n");
        for sym in &sub.key_symbols {
            md.push_str(&format!("{}\n", sym));
        }
        md.push_str("```\n\n");
    }

    // Проверка и тестирование
    md.push_str("## 🧪 Запуск и тестирование\n```bash\n");
    if let Some(cmd) = &sub.test_command {
        md.push_str(&format!("{}\n", cmd));
    } else if let Some(cmd) = &analysis.commands.test {
        md.push_str(&format!("{}\n", cmd));
    } else {
        md.push_str("# Проверьте общие тесты проекта\n");
    }
    md.push_str("```\n");

    md
}

fn render_commands_markdown(analysis: &ProjectAnalysis) -> String {
    let mut md = format!(
        "# 🛠 Справочник команд: {} (L2)\n\n> Автоматически извлечено из конфигурации проекта ({})\n\n",
        analysis.name,
        analysis.project_type.display_name()
    );

    md.push_str("## Стандартные команды жизненного цикла\n\n");
    md.push_str("| Задача | Команда |\n| :--- | :--- |\n");

    if let Some(cmd) = &analysis.commands.build {
        md.push_str(&format!("| **Сборка** | `{}` |\n", cmd));
    }
    if let Some(cmd) = &analysis.commands.test {
        md.push_str(&format!("| **Тестирование** | `{}` |\n", cmd));
    }
    if let Some(cmd) = &analysis.commands.lint {
        md.push_str(&format!("| **Линтинг** | `{}` |\n", cmd));
    }
    if let Some(cmd) = &analysis.commands.fmt {
        md.push_str(&format!("| **Форматирование** | `{}` |\n", cmd));
    }
    if let Some(cmd) = &analysis.commands.run {
        md.push_str(&format!("| **Запуск** | `{}` |\n", cmd));
    }

    if !analysis.commands.custom.is_empty() {
        md.push_str("\n## Пользовательские скрипты и задачи\n\n");
        md.push_str("| Скрипт | Выполняемая команда |\n| :--- | :--- |\n");
        for (name, cmd) in &analysis.commands.custom {
            md.push_str(&format!("| `{}` | `{}` |\n", name, cmd));
        }
    }

    md.push_str("\n## Быстрый запуск\n```bash\n");
    if let Some(cmd) = &analysis.commands.build {
        md.push_str(&format!("{}\n", cmd));
    }
    if let Some(cmd) = &analysis.commands.test {
        md.push_str(&format!("{}\n", cmd));
    }
    md.push_str("```\n");

    md
}

fn render_adr_markdown(
    analysis: &ProjectAnalysis,
    subsystems: &[SubsystemInfo],
    today: &str,
) -> String {
    let mut sub_list = String::new();
    for s in subsystems {
        sub_list.push_str(&format!("- **{}** (`{}`): {}\n", s.name, s.id, s.summary));
    }

    format!(
        "# ADR 0001: Начальная архитектура и топология {}\n\n- **Статус:** Принято\n- **Дата:** {}\n- **Стек:** {}\n\n## Контекст\nИнициализация кодовой базы `{}`. Для обеспечения надежной автономной навигации ИИ-ассистентов зафиксирована трехуровневая модель контекста (OpenViking L0-L2).\n\n## Архитектурные границы подсистем\n{}\n\n## Решение\n1. `L0_index.json` и `L0_index.md` служат единой точкой входа (Mental Map) с минимальным потреблением токенов.\n2. Каждая подсистема изолирована в карточке L1 (`subsystems/*.md`) с перечислением точек входа и сигнатур.\n3. Справочник команд L2 (`cli/commands.md`) фиксирует фактические команды сборки и верификации.\n\n## Последствия\n- Устраняется необходимость рекурсивного сканирования всего репозитория при каждом промпте.\n- Агенты обращаются к точечным карточкам L1 по мере необходимости.\n",
        analysis.name, today, analysis.project_type.display_name(), analysis.name, sub_list
    )
}

fn update_or_create_root_agents_md(
    root: &Path,
    project_name: &str,
    subsystems: &[SubsystemInfo],
) -> Result<()> {
    let agents_path = root.join("AGENTS.md");
    let section_header = "## 🗺 Архитектурная карта и навигация (OpenViking L0-L2)";

    let mut map_content = format!(
        "\n\n{}\n\nПроект `{}` описан трехуровневой базой знаний context-init (L0-L2), которая хранится вне репозитория. \
Для навигации используйте её вместо чтения исходников целиком.\n\n\
### ⚡ MCP-First Protocol (context-init)\n\n\
1. `context_get_subsystems` — карта подсистем проекта (L0).\n\
2. `context_get_subsystem(subsystem_id)` — компактная L1-карточка подсистемы.\n\
3. `context_pack(subsystem_id, compress=true)` — AST-сжатый код подсистемы.\n\
4. `context_check` — проверка архитектурного дрейфа перед сдачей задачи.\n\n\
Без доступа к MCP (субагенты, скрипты): `context-init pack <subsystem_id> --compress` и `context-init check --compact`.\n\n\
### 📦 Доступные подсистемы:\n",
        section_header, project_name
    );

    // Sorted by id, so split-off modules (`<crate>_<module>`) follow their crate.
    let mut sorted: Vec<&SubsystemInfo> = subsystems.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    for s in sorted {
        map_content.push_str(&format!("- `{}` ({}) — {}\n", s.id, s.name, s.summary));
    }

    if agents_path.exists() {
        let existing = fs::read_to_string(&agents_path).unwrap_or_default();
        let updated = if let Some(idx) = existing.find("## 🗺 Архитектурная карта и навигация")
        {
            let before = existing[..idx].trim_end();
            format!("{}\n\n{}", before, map_content.trim_start())
        } else {
            let mut u = existing;
            u.push_str(&map_content);
            u
        };
        fs::write(&agents_path, updated)
            .with_context(|| format!("Не удалось обновить {:?}", agents_path))?;
    } else {
        let mut initial = format!(
            "# Инструкции для ИИ-агентов в проекте {}\n\nДанный проект настроен для автономной работы ассистентов.",
            project_name
        );
        initial.push_str(&map_content);
        fs::write(&agents_path, initial)
            .with_context(|| format!("Не удалось создать {:?}", agents_path))?;
    }

    Ok(())
}

/// Turns a module path into a subsystem id matching `SUBSYSTEM_ID_PATTERN`.
///
/// Separators and other characters become `_`; ids with no ASCII alphanumerics left
/// (e.g. Cyrillic names) get a stable hash-based id so distinct modules never collide.
pub fn sanitize_id(raw: &str) -> String {
    let mut id = String::with_capacity(raw.len());
    for c in raw.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
            id.push(c);
        } else if !id.ends_with('_') {
            id.push('_');
        }
    }
    let id = id.trim_matches(|c| c == '_' || c == '-');
    if !id.chars().any(|c| c.is_ascii_alphanumeric()) {
        let digest = Sha256::digest(raw.as_bytes());
        let hex: String = digest[..4].iter().map(|b| format!("{b:02x}")).collect();
        return format!("m_{hex}");
    }
    id.chars()
        .take(64)
        .collect::<String>()
        .trim_end_matches(['_', '-'])
        .to_string()
}

fn chrono_like_date() -> String {
    now_rfc3339()[..10].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("src/detector"), "src_detector");
        assert_eq!(sanitize_id("packages\\core"), "packages_core");
    }

    #[test]
    fn test_l0_index_json_trailing_newline() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_l0_newline_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&temp_dir);

        let analysis = ProjectAnalysis {
            name: "test-proj".to_string(),
            description: "Test project".to_string(),
            root_dir: temp_dir.clone(),
            project_type: crate::storage::ProjectType::Rust,
            rules: Vec::new(),
            root_entrypoints: Vec::new(),
            commands: crate::storage::ProjectCommands::default(),
            detected_modules: Vec::new(),
            workspace_members: Vec::new(),
        };

        let sub = SubsystemInfo {
            id: "core".to_string(),
            name: "Core".to_string(),
            summary: "Core module".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            files: Vec::new(),
            entrypoints: Vec::new(),
            key_files: Vec::new(),
            key_symbols: Vec::new(),
            tags: vec!["core".to_string()],
            test_command: None,
            tokens: 100,
            parent: None,
        };

        // 1. Test generate_knowledge_base
        generate(
            &analysis,
            std::slice::from_ref(&sub),
            &temp_dir.join("kb"),
            &GenerateOptions::default(),
        )
        .expect("generate should succeed");

        let kd = temp_dir.join("kb");
        let l0_path = kd.join("L0_index.json");
        assert!(l0_path.exists());
        let content = fs::read_to_string(&l0_path).expect("read L0_index.json");
        assert!(
            content.ends_with('\n'),
            "generate must produce L0_index.json ending with newline"
        );
        assert!(
            !content.ends_with("\n\n"),
            "L0_index.json should not end with multiple newlines"
        );

        // 2. Test update_knowledge_base
        let mut sub2 = sub.clone();
        sub2.tokens = 150;
        update(&analysis, &[sub2], &kd, false).expect("update should succeed");

        let updated_content = fs::read_to_string(&l0_path).expect("read updated L0_index.json");
        assert!(
            updated_content.ends_with('\n'),
            "update must produce L0_index.json ending with newline"
        );
        assert!(
            !updated_content.ends_with("\n\n"),
            "L0_index.json should not end with multiple newlines"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    fn named_sub(id: &str, summary: &str) -> SubsystemInfo {
        SubsystemInfo {
            id: id.to_string(),
            name: id.to_string(),
            summary: summary.to_string(),
            l1_path: format!("subsystems/{id}.md"),
            files: Vec::new(),
            entrypoints: Vec::new(),
            key_files: Vec::new(),
            key_symbols: Vec::new(),
            tags: Vec::new(),
            test_command: None,
            tokens: 1,
            parent: None,
        }
    }

    #[test]
    fn test_agents_md_map_is_sorted_so_children_follow_their_crate() {
        let temp_dir = std::env::temp_dir().join("test_agents_md_sorted");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let subs = [
            named_sub("tests", "t"),
            named_sub("crates_a_storage", "s"),
            named_sub("crates_b", "b"),
            named_sub("crates_a", "a"),
        ];
        update_or_create_root_agents_md(&temp_dir, "p", &subs).unwrap();
        let content = fs::read_to_string(temp_dir.join("AGENTS.md")).unwrap();
        let pos = |id: &str| content.find(&format!("- `{id}`")).unwrap();
        assert!(pos("crates_a") < pos("crates_a_storage"));
        assert!(pos("crates_a_storage") < pos("crates_b"));
        assert!(pos("crates_b") < pos("tests"));
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_update_refreshes_generated_summary_but_keeps_manual_one() {
        let temp_dir = std::env::temp_dir().join("test_update_summary_refresh");
        let _ = fs::remove_dir_all(&temp_dir);
        let analysis = ProjectAnalysis {
            name: "p".to_string(),
            root_dir: temp_dir.clone(),
            description: String::new(),
            project_type: crate::storage::ProjectType::Rust,
            rules: Vec::new(),
            root_entrypoints: Vec::new(),
            commands: crate::storage::ProjectCommands::default(),
            detected_modules: Vec::new(),
            workspace_members: Vec::new(),
        };
        let kd = temp_dir.join("kb");
        let old_generated = named_sub(
            "a_cfg",
            "Модуль и компоненты директории `a/cfg.rs` (1 файлов)",
        );
        let manual = named_sub("a_manual", "Hand-written summary");
        generate(
            &analysis,
            &[old_generated, manual],
            &kd,
            &GenerateOptions::default(),
        )
        .unwrap();

        let fresh = [
            named_sub("a_cfg", "Модуль-файл `a/cfg.rs`"),
            named_sub("a_manual", "Модуль-файл `a/manual.rs`"),
        ];
        update(&analysis, &fresh, &kd, false).unwrap();
        let index = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        let summary = |id: &str| {
            index
                .subsystems
                .iter()
                .find(|s| s.id == id)
                .unwrap()
                .summary
                .clone()
        };
        assert_eq!(summary("a_cfg"), "Модуль-файл `a/cfg.rs`");
        assert_eq!(summary("a_manual"), "Hand-written summary");
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_agents_md_mcp_first_navigation() {
        let temp_dir = std::env::temp_dir().join("test_agents_md_mcp");
        let _ = fs::create_dir_all(&temp_dir);

        let sub = SubsystemInfo {
            id: "core".to_string(),
            name: "Core".to_string(),
            summary: "Core module".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            files: Vec::new(),
            entrypoints: Vec::new(),
            key_files: Vec::new(),
            key_symbols: Vec::new(),
            tags: vec!["core".to_string()],
            test_command: None,
            tokens: 100,
            parent: None,
        };

        update_or_create_root_agents_md(&temp_dir, "test-proj", &[sub]).expect("should succeed");

        let content = fs::read_to_string(temp_dir.join("AGENTS.md")).expect("read AGENTS.md");
        assert!(content.contains("MCP-First Protocol"));
        assert!(content.contains("context_get_subsystems"));
        assert!(content.contains("context_get_subsystem"));
        assert!(content.contains("context_pack"));
        assert!(content.contains("context_check"));
        assert!(
            !content.contains("[Core](.agents/knowledge/subsystems/core.md)"),
            "AGENTS.md must not contain direct links to subsystem markdown files"
        );
        assert!(content.contains("- `core` (Core) — Core module"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    fn split_drift_fixture() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "test_split_drift_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let src = root.join("crates").join("foo").join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/foo\"]\n",
        )
        .unwrap();
        fs::write(
            root.join("crates").join("foo").join("Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(src.join("lib.rs"), "pub mod big;\n").unwrap();
        let body = |prefix: &str, n: usize| -> String {
            let mut out = String::new();
            let mut i = 0;
            while crate::analysis::detector::count_tokens(&out) < n {
                out.push_str(&format!("pub fn {prefix}_{i}() {{}}\n"));
                i += 1;
            }
            out
        };
        fs::write(src.join("big.rs"), body("big", 900)).unwrap();
        fs::write(src.join("small.rs"), body("small", 300)).unwrap();
        root
    }

    fn analyze_at(root: &Path, threshold: usize) -> ProjectAnalysis {
        crate::analysis::detector::analyze_project_with(
            root,
            &crate::analysis::detector::AnalyzeOptions {
                deep: false,
                with_rtk: false,
                split_threshold_tokens: threshold,
            },
        )
        .unwrap()
    }

    #[test]
    fn test_crate_split_is_clean_added_and_removed_drift() {
        let root = split_drift_fixture();
        let kb = root.with_extension("kb");
        let whole = analyze_at(&root, 1_000_000);
        let subs = crate::analysis::subsystems::build_deterministic(&whole);
        generate(&whole, &subs, &kb, &GenerateOptions::default()).unwrap();

        // Crate grows past the threshold: the child is new, the crate id survives.
        let split = analyze_at(&root, 1000);
        let report = audit(&split, &kb).unwrap();
        assert!(report.missing_in_fs.is_empty(), "{report:?}");
        assert_eq!(
            report.untracked_in_knowledge,
            vec!["crates_foo_big".to_string()]
        );

        let split_subs = crate::analysis::subsystems::build_deterministic(&split);
        let child = split_subs
            .iter()
            .find(|s| s.id == "crates_foo_big")
            .unwrap();
        assert_eq!(child.parent.as_deref(), Some("crates_foo"));
        let (added, _) = update(&split, &split_subs, &kb, false).unwrap();
        assert_eq!(added, 1);
        let report = audit(&split, &kb).unwrap();
        assert!(report.missing_in_fs.is_empty() && report.untracked_in_knowledge.is_empty());
        let index = L0Index::load_from_file(&kb.join("L0_index.json")).unwrap();
        let crate_sub = index
            .subsystems
            .iter()
            .find(|s| s.id == "crates_foo")
            .unwrap();
        assert!(
            !crate_sub.files.iter().any(|f| f.path.ends_with("big.rs")),
            "the crate root no longer packs the split-off module"
        );

        // Crate shrinks back: the child is reported gone and dropped by update.
        let report = audit(&whole, &kb).unwrap();
        assert_eq!(report.missing_in_fs, vec!["crates_foo_big".to_string()]);
        assert!(report.untracked_in_knowledge.is_empty());
        update(&whole, &subs, &kb, false).unwrap();
        let index = L0Index::load_from_file(&kb.join("L0_index.json")).unwrap();
        assert!(index.subsystems.iter().all(|s| s.id != "crates_foo_big"));
        assert!(!kb.join("subsystems/crates_foo_big.md").exists());
        let report = audit(&whole, &kb).unwrap();
        assert!(report.missing_in_fs.is_empty() && report.untracked_in_knowledge.is_empty());

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&kb);
    }
}
