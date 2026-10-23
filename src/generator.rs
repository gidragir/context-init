use crate::types::{DriftReport, L0Index, ProjectAnalysis, SubsystemInfo};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn generate_knowledge_base(
    analysis: &ProjectAnalysis,
    subsystems: &[SubsystemInfo],
    force: bool,
    with_agents_md: bool,
) -> Result<()> {
    let base_dir = analysis.root_dir.join(".agents").join("knowledge");
    let subsystems_dir = base_dir.join("subsystems");
    let cli_dir = base_dir.join("cli");
    let adr_dir = base_dir.join("adr");

    if base_dir.exists() && !force {
        anyhow::bail!(
            "Директория {:?} уже существует. Используйте флаг --force для перезаписи или команду update.",
            base_dir
        );
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
        project: analysis.name.clone(),
        description: analysis.description.clone(),
        root_path: analysis.root_dir.to_string_lossy().to_string(),
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

pub fn audit_knowledge_base(analysis: &ProjectAnalysis) -> Result<DriftReport> {
    let (base_dir, l0_path) =
        match crate::service::ContextService::find_knowledge_dir(&analysis.root_dir) {
            Ok(dir) => {
                let p = dir.join("L0_index.json");
                (dir, p)
            }
            Err(_) => {
                let dir = analysis.root_dir.join(".agents").join("knowledge");
                let p = dir.join("L0_index.json");
                if !p.exists() {
                    anyhow::bail!(
                        "База знаний не найдена по пути {:?}. Сначала запустите `context-init`.",
                        p
                    );
                }
                (dir, p)
            }
        };

    let content = fs::read_to_string(&l0_path)
        .with_context(|| format!("Не удалось прочитать {:?}", l0_path))?;
    let index: L0Index = serde_json::from_str(&content)
        .with_context(|| format!("Ошибка десериализации L0_index.json в {:?}", l0_path))?;

    let existing_sub_ids: Vec<String> = index.subsystems.iter().map(|s| s.id.clone()).collect();
    let fs_module_ids: Vec<String> = analysis
        .detected_modules
        .iter()
        .map(|m| sanitize_id(&m.dir_path))
        .collect();

    let mut missing_in_fs = Vec::new();
    let mut up_to_date_count = 0;

    for sub in &index.subsystems {
        let exists = if !sub.files.is_empty() {
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

pub fn update_knowledge_base(
    analysis: &ProjectAnalysis,
    new_subsystems: &[SubsystemInfo],
    with_agents_md: bool,
) -> Result<(usize, usize)> {
    let base_dir = analysis.root_dir.join(".agents").join("knowledge");
    let l0_path = base_dir.join("L0_index.json");

    if !l0_path.exists() {
        // Если базы не было, создаем с нуля
        generate_knowledge_base(analysis, new_subsystems, false, with_agents_md)?;
        return Ok((new_subsystems.len(), 0));
    }

    let content = fs::read_to_string(&l0_path)?;
    let mut index: L0Index = serde_json::from_str(&content)?;

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
    md.push_str("| Подсистема | Роль / Назначение | Точки входа | Токены (BPE) | L1 Карточка |\n");
    md.push_str("| :--- | :--- | :--- | :--- | :--- |\n");

    for s in &index.subsystems {
        let ep_str = if s.entrypoints.is_empty() {
            "-".to_string()
        } else {
            s.entrypoints.join(", ")
        };
        let tokens_str = if s.tokens > 0 {
            format!("~{}", s.tokens)
        } else {
            "-".to_string()
        };
        md.push_str(&format!(
            "| **{}** | {} | `{}` | {} | [{}]({}) |\n",
            s.name, s.summary, ep_str, tokens_str, s.l1_path, s.l1_path
        ));
    }

    md.push_str("\n## 📋 Инварианты и правила проекта\n");
    for rule in &index.core_rules {
        md.push_str(&format!("- {}\n", rule));
    }

    md
}

fn render_subsystem_card(sub: &SubsystemInfo, analysis: &ProjectAnalysis) -> String {
    let tokens_info = if sub.tokens > 0 {
        format!(" | **Объем:** ~{} токенов BPE", sub.tokens)
    } else {
        String::new()
    };
    let mut md = format!(
        "# Подсистема: {} (L1)\n\n> **Идентификатор:** `{}`\n> **Теги:** {}{}\n\n## 🎯 Назначение и роль\n{}\n\n",
        sub.name,
        sub.id,
        sub.tags.join(", "),
        tokens_info,
        sub.summary
    );

    // Точки входа и ключевые файлы
    md.push_str("## 📁 Точки входа и структура файлов\n\n");
    if !sub.files.is_empty() {
        md.push_str("| Файл | Строк | Статус |\n| :--- | :--- | :--- |\n");
        for file in &sub.files {
            let badge = if file.is_entrypoint {
                "⭐ Точка входа"
            } else {
                "Исходный код"
            };
            md.push_str(&format!(
                "| `{}` | {} | {} |\n",
                file.path, file.lines, badge
            ));
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
        "\n\n{}\n\nПроект `{}` управляется трехуровневой базой знаний OpenViking (L0-L2).\nКатегорически запрещено читать файлы `.agents/knowledge/**` напрямую через инструменты чтения файлов (`view_file`, `cat` и др.). Вся навигация и исследование архитектуры выполняются исключительно через MCP-сервер `context-init` (MCP-First Protocol).\n\n### ⚡ MCP-First Protocol (context-init)\n\n1. **Обзор всех подсистем и структуры проекта (L0):**\n   Вызовите `context_get_subsystems`:\n   ```json\n   {{\"ServerName\": \"context-init\", \"ToolName\": \"context_get_subsystems\", \"Arguments\": {{}}}}\n   ```\n2. **Получение компактной L1-карточки конкретной подсистемы:**\n   Вызовите `context_get_subsystem` (передайте `subsystem_id`):\n   ```json\n   {{\"ServerName\": \"context-init\", \"ToolName\": \"context_get_subsystem\", \"Arguments\": {{\"subsystem_id\": \"<subsystem_id>\"}}}}\n   ```\n3. **AST-сжатый контекст кода подсистемы (без раздувания контекста):**\n   Вызовите `context_pack`:\n   ```json\n   {{\"ServerName\": \"context-init\", \"ToolName\": \"context_pack\", \"Arguments\": {{\"subsystem_id\": \"<subsystem_id>\", \"compress\": true}}}}\n   ```\n4. **Проверка архитектурного дрейфа перед сдачей задачи:**\n   Вызовите `context_check`:\n   ```json\n   {{\"ServerName\": \"context-init\", \"ToolName\": \"context_check\", \"Arguments\": {{}}}}\n   ```\n\n### 📦 Доступные подсистемы:\n",
        section_header, project_name
    );

    for s in subsystems {
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

pub fn sanitize_id(raw: &str) -> String {
    raw.replace(['/', '\\'], "_").to_lowercase()
}

fn chrono_like_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let mut year = 1970;
    let mut remaining_days = days;
    loop {
        let days_in_year = if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days >= days_in_year {
            remaining_days -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let days_in_months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for &d in &days_in_months {
        if remaining_days >= d {
            remaining_days -= d;
            month += 1;
        } else {
            break;
        }
    }
    let day = remaining_days + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
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
    fn test_chrono_like_date() {
        let date = chrono_like_date();
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
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
            project_type: crate::types::ProjectType::Rust,
            rules: Vec::new(),
            root_entrypoints: Vec::new(),
            commands: crate::types::ProjectCommands::default(),
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
        };

        // 1. Test generate_knowledge_base
        generate_knowledge_base(&analysis, std::slice::from_ref(&sub), false, false)
            .expect("generate should succeed");

        let l0_path = temp_dir.join(".agents/knowledge/L0_index.json");
        assert!(l0_path.exists());
        let content = fs::read_to_string(&l0_path).expect("read L0_index.json");
        assert!(
            content.ends_with('\n'),
            "generate_knowledge_base must produce L0_index.json ending with newline"
        );
        assert!(
            !content.ends_with("\n\n"),
            "L0_index.json should not end with multiple newlines"
        );

        // 2. Test update_knowledge_base
        let mut sub2 = sub.clone();
        sub2.tokens = 150;
        update_knowledge_base(&analysis, &[sub2], false).expect("update should succeed");

        let updated_content = fs::read_to_string(&l0_path).expect("read updated L0_index.json");
        assert!(
            updated_content.ends_with('\n'),
            "update_knowledge_base must produce L0_index.json ending with newline"
        );
        assert!(
            !updated_content.ends_with("\n\n"),
            "L0_index.json should not end with multiple newlines"
        );

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
}
