use crate::analysis::subsystems::GENERATED_SUMMARY_PREFIXES;
use crate::analysis::ProjectAnalysis;
use crate::config::StorageMode;
use crate::git::{GitBackend, SystemGitBackend};
use crate::openviking::{CardFreshness, CardFrontmatter, CardGeneratedBy, CardSource, OkfCard};
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
    /// Create or refresh the navigation section of the repo's `AGENTS.md`, worded for this
    /// storage mode; `None` leaves `AGENTS.md` alone.
    pub agents_md: Option<StorageMode>,
}

/// Writes `content` to `path` only when the bytes differ (creating parent directories).
/// Returns whether the file was written.
fn write_if_changed(path: &Path, content: &str) -> Result<bool> {
    if fs::read(path).ok().as_deref() == Some(content.as_bytes()) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("Не удалось создать {:?}", parent))?;
    }
    fs::write(path, content).with_context(|| format!("Не удалось записать {:?}", path))?;
    Ok(true)
}

/// Directory (or single file) a subsystem's L1 card tracks in Git: the common parent of its
/// files, the file itself for single-file subsystems, else the subsystem id.
fn subsystem_directory(sub: &SubsystemInfo) -> String {
    let paths: Vec<String> = if sub.files.is_empty() {
        sub.key_files.iter().map(|p| p.replace('\\', "/")).collect()
    } else {
        sub.files
            .iter()
            .map(|f| f.path.replace('\\', "/"))
            .collect()
    };
    let Some(first) = paths.first() else {
        return sub.id.clone();
    };
    if paths.len() == 1 {
        return first.clone();
    }
    let mut common: Vec<&str> = first.split('/').collect();
    common.pop();
    for p in &paths[1..] {
        let parts: Vec<&str> = p.split('/').collect();
        let keep = common
            .iter()
            .zip(parts.iter().take(parts.len().saturating_sub(1)))
            .take_while(|(a, b)| a == b)
            .count();
        common.truncate(keep);
    }
    if common.is_empty() {
        ".".to_string()
    } else {
        common.join("/")
    }
}

/// `(blob_sha, commit_sha)` of `directory` in the Git repository at `root`; `None` outside Git.
fn git_source_shas(root: &Path, directory: &str) -> Option<(String, String)> {
    let git = SystemGitBackend::new();
    let commit = git.head_commit(root).ok()?;
    let blob = git.blob_sha(root, Path::new(directory)).ok()??;
    Some((blob, commit))
}

fn file_list_sha(sub: &SubsystemInfo) -> String {
    let mut paths: Vec<&str> = sub.files.iter().map(|f| f.path.as_str()).collect();
    paths.extend(sub.key_files.iter().map(String::as_str));
    paths.sort_unstable();
    let mut hasher = Sha256::new();
    for p in paths {
        hasher.update(p.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// L1 card as an OpenViking OKF Sidecar: YAML frontmatter (`directory`, `source`, `generated_by`,
/// `freshness`) followed by the card body. `source_shas` is `(blob_sha, commit_sha)`; without it
/// (outside Git) the blob is a SHA-256 of the file list.
pub fn render_subsystem_card_okf(
    sub: &SubsystemInfo,
    analysis: &ProjectAnalysis,
    source_shas: Option<(String, String)>,
    role: &str,
    model: &str,
) -> String {
    let entries = sub.files.len().max(sub.key_files.len());
    let source = match source_shas {
        Some((blob_sha, commit_sha)) => CardSource {
            kind: "git".to_string(),
            blob_sha,
            commit_sha,
        },
        None => CardSource {
            kind: "fs".to_string(),
            blob_sha: file_list_sha(sub),
            commit_sha: String::new(),
        },
    };
    let frontmatter = CardFrontmatter {
        directory: subsystem_directory(sub),
        source,
        generated_by: CardGeneratedBy {
            component: "context-init".to_string(),
            role: role.to_string(),
            model: model.to_string(),
        },
        freshness: CardFreshness {
            total_entries: entries,
            sampled_entries: entries,
            ..CardFreshness::default()
        },
    };
    OkfCard::new(frontmatter, render_subsystem_card(sub, analysis)).to_markdown()
}

fn render_card_for(sub: &SubsystemInfo, analysis: &ProjectAnalysis, role: &str) -> String {
    let shas = git_source_shas(&analysis.root_dir, &subsystem_directory(sub));
    render_subsystem_card_okf(sub, analysis, shas, role, "none")
}

/// Whether `disk` is the same card as `fresh` apart from provenance noise: body, directory and
/// source blob must match. Keeps `commit_sha`/`generated_by` stable, so an unchanged tree
/// rewrites nothing even after HEAD moved.
fn same_card(disk: &str, fresh: &str) -> bool {
    match (OkfCard::parse(disk), OkfCard::parse(fresh)) {
        (Ok(a), Ok(b)) => {
            a.body == b.body
                && a.frontmatter.directory == b.frontmatter.directory
                && a.frontmatter.source.blob_sha == b.frontmatter.source.blob_sha
                && a.frontmatter.freshness == b.frontmatter.freshness
        }
        _ => false,
    }
}

/// Whether the card on disk is untouched generator output for `expected_body` (legacy cards
/// without frontmatter are compared whole).
fn is_pristine_card(disk: &str, expected_body: &str) -> bool {
    match OkfCard::parse(disk) {
        Ok(card) => card.body == expected_body.trim(),
        Err(_) => disk.trim() == expected_body.trim(),
    }
}

/// Rewrites `source` of an existing OKF card to the current Git state, keeping its body.
/// Returns whether the file changed. Cards without frontmatter are left alone.
pub fn restamp_card_source(card_path: &Path, root: &Path, directory: Option<&str>) -> Result<bool> {
    let disk = fs::read_to_string(card_path)?;
    let Ok(mut card) = OkfCard::parse(&disk) else {
        return Ok(false);
    };
    let dir = directory
        .map(str::to_string)
        .unwrap_or_else(|| card.frontmatter.directory.clone());
    let Some((blob_sha, commit_sha)) = git_source_shas(root, &dir) else {
        return Ok(false);
    };
    card.frontmatter.source = CardSource {
        kind: "git".to_string(),
        blob_sha,
        commit_sha,
    };
    write_if_changed(card_path, &card.to_markdown())
}

/// Gives the index a stable order: subsystems by id; files, entrypoints, key files and
/// tags of each subsystem lexicographically. `key_symbols` keep the detector's order.
fn normalize_index(index: &mut L0Index) {
    index.subsystems.sort_by(|a, b| a.id.cmp(&b.id));
    for s in &mut index.subsystems {
        s.files.sort_by(|a, b| a.path.cmp(&b.path));
        s.entrypoints.sort();
        s.key_files.sort();
        s.tags.sort();
    }
}

fn render_l0_json(index: &L0Index) -> Result<String> {
    Ok(format!("{}\n", serde_json::to_string_pretty(index)?))
}

/// Writes a fresh L0–L2 knowledge base into `knowledge_dir`. Only `AGENTS.md` goes into
/// the repository (`analysis.root_dir`). Files whose bytes are unchanged are not rewritten.
pub fn generate(
    analysis: &ProjectAnalysis,
    subsystems: &[SubsystemInfo],
    knowledge_dir: &Path,
    opts: &GenerateOptions,
) -> Result<()> {
    let GenerateOptions {
        force,
        dry_run,
        agents_md,
    } = *opts;
    let base_dir = knowledge_dir.to_path_buf();
    let subsystems_dir = base_dir.join("subsystems");

    if base_dir.exists() && !force {
        anyhow::bail!(
            "Директория {:?} уже существует. Используйте флаг --force для перезаписи или команду update.",
            base_dir
        );
    }
    if dry_run {
        return Ok(());
    }

    let today = chrono_like_date();

    let mut l0_index = L0Index {
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
    normalize_index(&mut l0_index);

    // A forced rebuild drops cards of subsystems that no longer exist.
    if force && subsystems_dir.exists() {
        let keep: Vec<_> = l0_index
            .subsystems
            .iter()
            .map(|s| base_dir.join(&s.l1_path))
            .collect();
        if let Ok(entries) = fs::read_dir(&subsystems_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file()
                    && p.extension().and_then(|e| e.to_str()) == Some("md")
                    && !keep.contains(&p)
                {
                    let _ = fs::remove_file(p);
                }
            }
        }
    }

    write_if_changed(&base_dir.join("L0_index.json"), &render_l0_json(&l0_index)?)?;
    write_if_changed(
        &base_dir.join("L0_index.md"),
        &render_l0_markdown(&l0_index, &today),
    )?;
    for sub in &l0_index.subsystems {
        write_if_changed(
            &base_dir.join(&sub.l1_path),
            &render_card_for(sub, analysis, "init"),
        )?;
    }
    write_if_changed(
        &base_dir.join("cli").join("commands.md"),
        &render_commands_markdown(analysis),
    )?;
    write_if_changed(
        &base_dir.join("adr").join("0001_initial_architecture.md"),
        &render_adr_markdown(analysis, &l0_index.subsystems, &today),
    )?;

    if let Some(mode) = agents_md {
        update_or_create_root_agents_md(
            &analysis.root_dir,
            &analysis.name,
            &l0_index.subsystems,
            mode,
        )?;
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
///
/// Quiet by design: everything is rendered in memory first, and when no file would
/// change nothing is written (`updated_at` included). Only a real difference bumps
/// `updated_at` and rewrites the files whose bytes changed. Cards that are not
/// generator output (hand- or agent-edited) are left alone.
pub fn update(
    analysis: &ProjectAnalysis,
    new_subsystems: &[SubsystemInfo],
    knowledge_dir: &Path,
    agents_md: Option<StorageMode>,
) -> Result<(usize, usize)> {
    let base_dir = knowledge_dir.to_path_buf();
    let l0_path = base_dir.join("L0_index.json");

    if !l0_path.exists() {
        // Если базы не было, создаем с нуля
        let opts = GenerateOptions {
            agents_md,
            ..GenerateOptions::default()
        };
        generate(analysis, new_subsystems, knowledge_dir, &opts)?;
        return Ok((new_subsystems.len(), 0));
    }

    let mut index = L0Index::load_from_file(&l0_path)?;
    // What the previous run would have rendered: a card still equal to it is untouched
    // generator output and may be refreshed; anything else was edited by hand or by an agent.
    let old_analysis = ProjectAnalysis {
        commands: index.commands.clone(),
        ..analysis.clone()
    };
    let mut pristine_ids: Vec<String> = Vec::new();
    index.commands = analysis.commands.clone();
    index.entrypoints = analysis.root_entrypoints.clone();
    index.core_rules = analysis.rules.clone();

    let mut added_count = 0;
    let mut updated_count = 0;
    let mut new_ids: Vec<String> = Vec::new();

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
            if let Some(card) = safe_join(&base_dir, &existing.l1_path) {
                let pristine = fs::read_to_string(&card).map_or(true, |disk| {
                    is_pristine_card(&disk, &render_subsystem_card(existing, &old_analysis))
                });
                if pristine {
                    pristine_ids.push(existing.id.clone());
                }
            }
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
            new_ids.push(new_sub.id.clone());
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
    let superseded_cards: Vec<_> = superseded
        .iter()
        .filter_map(|l1| safe_join(&base_dir, l1))
        .filter(|card| card.exists())
        .collect();
    normalize_index(&mut index);

    // Render everything in memory; generator-owned cards only.
    let mut rendered: Vec<(std::path::PathBuf, String)> = Vec::new();
    for sub in &index.subsystems {
        let Some(card) = safe_join(&base_dir, &sub.l1_path) else {
            continue;
        };
        let owned = new_ids.contains(&sub.id) || pristine_ids.contains(&sub.id);
        if owned {
            let fresh = render_card_for(sub, analysis, "update");
            let content = match fs::read_to_string(&card) {
                Ok(disk) if same_card(&disk, &fresh) => disk,
                _ => fresh,
            };
            rendered.push((card, content));
        }
    }
    rendered.push((
        base_dir.join("cli/commands.md"),
        render_commands_markdown(analysis),
    ));
    let differs =
        |path: &Path, content: &str| fs::read(path).ok().as_deref() != Some(content.as_bytes());
    let l0_md_path = base_dir.join("L0_index.md");
    let changed = !superseded_cards.is_empty()
        || differs(&l0_path, &render_l0_json(&index)?)
        || differs(&l0_md_path, &render_l0_markdown(&index, &index.updated_at))
        || rendered.iter().any(|(p, c)| differs(p, c));

    if changed {
        index.updated_at = chrono_like_date();
        for (path, content) in &rendered {
            write_if_changed(path, content)?;
        }
        for card in &superseded_cards {
            let _ = fs::remove_file(card);
        }
        write_if_changed(&l0_path, &render_l0_json(&index)?)?;
        write_if_changed(&l0_md_path, &render_l0_markdown(&index, &index.updated_at))?;
    }

    if let Some(mode) = agents_md {
        update_or_create_root_agents_md(
            &analysis.root_dir,
            &analysis.name,
            &index.subsystems,
            mode,
        )?;
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
        "# Подсистема: {} (L1)\n\n{}\n\n> **Идентификатор:** `{}`\n> **Теги:** {}\n\n",
        sub.name,
        sub.summary,
        sub.id,
        sub.tags.join(", ")
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
    mode: StorageMode,
) -> Result<()> {
    let agents_path = root.join("AGENTS.md");
    let section_header = "## 🗺 Архитектурная карта и навигация (OpenViking L0-L2)";

    let storage_note = match mode {
        StorageMode::Central => {
            "которая хранится вне репозитория. Для навигации используйте её вместо чтения исходников целиком."
        }
        StorageMode::Repo => {
            "которая лежит в `.context-init/knowledge/` и коммитится вместе с кодом. Людям можно читать её напрямую; агентам — предпочитать MCP/`pack` ниже: это компактнее и учитывает бюджет токенов. После структурных изменений запустите `context-init update` и закоммитьте изменения базы в том же PR."
        }
    };
    let mut map_content = format!(
        "\n\n{}\n\nПроект `{}` описан трехуровневой базой знаний context-init (L0-L2), {}\n\n\
### ⚡ MCP-First Protocol (context-init)\n\n\
1. `context_get_subsystems` — карта подсистем проекта (L0).\n\
2. `context_get_subsystem(subsystem_id)` — компактная L1-карточка подсистемы.\n\
3. `context_pack(subsystem_id, compress=true)` — AST-сжатый код подсистемы.\n\
4. `context_check` — проверка архитектурного дрейфа перед сдачей задачи.\n\n\
Без доступа к MCP (субагенты, скрипты): `rtk context-init pack <subsystem_id> --compress` и `rtk context-init check --compact`.\n\n\
### 📦 Доступные подсистемы:\n",
        section_header, project_name, storage_note
    );

    // Sorted by id, so split-off modules (`<crate>_<module>`) follow their crate.
    let mut sorted: Vec<&SubsystemInfo> = subsystems.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    for s in sorted {
        map_content.push_str(&format!("- `{}` ({}) — {}\n", s.id, s.name, s.summary));
    }

    let content = if agents_path.exists() {
        let existing = fs::read_to_string(&agents_path).unwrap_or_default();
        if let Some(idx) = existing.find("## 🗺 Архитектурная карта и навигация")
        {
            let before = existing[..idx].trim_end();
            format!("{}\n\n{}", before, map_content.trim_start())
        } else {
            let mut u = existing;
            u.push_str(&map_content);
            u
        }
    } else {
        let mut initial = format!(
            "# Инструкции для ИИ-агентов в проекте {}\n\nДанный проект настроен для автономной работы ассистентов.",
            project_name
        );
        initial.push_str(&map_content);
        initial
    };
    write_if_changed(&agents_path, &content)?;

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
    use crate::config::StorageMode;
    use std::collections::BTreeMap;
    use std::time::SystemTime;

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
        update(&analysis, &[sub2], &kd, None).expect("update should succeed");

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
        update_or_create_root_agents_md(&temp_dir, "p", &subs, StorageMode::Central).unwrap();
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
        update(&analysis, &fresh, &kd, None).unwrap();
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

        update_or_create_root_agents_md(&temp_dir, "test-proj", &[sub], StorageMode::Central)
            .expect("should succeed");

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
                scope: Default::default(),
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
        let (added, _) = update(&split, &split_subs, &kb, None).unwrap();
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
        update(&whole, &subs, &kb, None).unwrap();
        let index = L0Index::load_from_file(&kb.join("L0_index.json")).unwrap();
        assert!(index.subsystems.iter().all(|s| s.id != "crates_foo_big"));
        assert!(!kb.join("subsystems/crates_foo_big.md").exists());
        let report = audit(&whole, &kb).unwrap();
        assert!(report.missing_in_fs.is_empty() && report.untracked_in_knowledge.is_empty());

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&kb);
    }

    fn quiet_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "test_quiet_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src/api")).unwrap();
        fs::create_dir_all(root.join("src/store")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"quiet\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            root.join("src/main.rs"),
            "mod api;\nmod store;\nfn main() {}\n",
        )
        .unwrap();
        fs::write(
            root.join("src/api/mod.rs"),
            "pub fn run_api() -> u32 {\n    1\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("src/store/mod.rs"),
            "pub fn run_store() -> u32 {\n    2\n}\n",
        )
        .unwrap();
        root
    }

    fn deterministic(root: &Path) -> (ProjectAnalysis, Vec<SubsystemInfo>) {
        let analysis = analyze_at(root, 15_000);
        let subs = crate::analysis::subsystems::build_deterministic(&analysis);
        (analysis, subs)
    }

    /// Bytes and mtime of every file under `dir` plus `extra`.
    fn snapshot(dir: &Path, extra: &[&Path]) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
        fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
            for e in fs::read_dir(dir).unwrap().flatten() {
                if e.path().is_dir() {
                    walk(&e.path(), out);
                } else {
                    out.push(e.path());
                }
            }
        }
        let mut files = Vec::new();
        walk(dir, &mut files);
        files.extend(extra.iter().map(|p| p.to_path_buf()));
        files
            .into_iter()
            .map(|p| {
                let meta = fs::metadata(&p).unwrap();
                (
                    p.to_string_lossy().to_string(),
                    (fs::read(&p).unwrap(), meta.modified().unwrap()),
                )
            })
            .collect()
    }

    fn agents_opts() -> GenerateOptions {
        GenerateOptions {
            agents_md: Some(StorageMode::Repo),
            ..GenerateOptions::default()
        }
    }

    #[test]
    fn update_twice_is_noop() {
        let root = quiet_root("twice");
        let kd = root.join(".context-init/knowledge");
        let (analysis, subs) = deterministic(&root);
        generate(&analysis, &subs, &kd, &agents_opts()).unwrap();
        update(&analysis, &subs, &kd, Some(StorageMode::Repo)).unwrap();
        let agents = root.join("AGENTS.md");
        let before = snapshot(&kd, &[&agents]);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        update(&analysis, &subs, &kd, Some(StorageMode::Repo)).unwrap();
        assert!(before == snapshot(&kd, &[&agents]), "update rewrote files");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn body_edit_does_not_touch_kb() {
        let root = quiet_root("body");
        let kd = root.join(".context-init/knowledge");
        let (analysis, subs) = deterministic(&root);
        generate(&analysis, &subs, &kd, &agents_opts()).unwrap();
        update(&analysis, &subs, &kd, Some(StorageMode::Repo)).unwrap();
        let before = snapshot(&kd, &[]);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let api = root.join("src/api/mod.rs");
        fs::write(
            &api,
            "// reworked\npub fn run_api() -> u32 {\n    let x = 40;\n    x + 2\n}\n",
        )
        .unwrap();
        let (analysis, subs) = deterministic(&root);
        update(&analysis, &subs, &kd, Some(StorageMode::Repo)).unwrap();
        assert!(before == snapshot(&kd, &[]), "a body edit changed the KB");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn structural_change_bumps_updated_at() {
        let root = quiet_root("bump");
        let kd = root.join(".context-init/knowledge");
        let (analysis, subs) = deterministic(&root);
        generate(&analysis, &subs, &kd, &GenerateOptions::default()).unwrap();
        let mut index = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        index.updated_at = "2000-01-01".to_string();
        fs::write(
            kd.join("L0_index.json"),
            format!("{}\n", serde_json::to_string_pretty(&index).unwrap()),
        )
        .unwrap();
        fs::write(
            kd.join("L0_index.md"),
            render_l0_markdown(&index, "2000-01-01"),
        )
        .unwrap();

        update(&analysis, &subs, &kd, None).unwrap();
        let after = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        assert_eq!(
            after.updated_at, "2000-01-01",
            "unchanged run keeps the date"
        );

        fs::create_dir_all(root.join("src/extra")).unwrap();
        fs::write(root.join("src/extra/mod.rs"), "pub fn extra() {}\n").unwrap();
        let (analysis, subs) = deterministic(&root);
        update(&analysis, &subs, &kd, None).unwrap();
        let after = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        assert_eq!(after.updated_at, chrono_like_date());
        assert!(
            kd.join("subsystems/src_extra.md").is_file()
                || after.subsystems.iter().any(|s| s.id.contains("extra"))
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn update_keeps_enriched_summary() {
        let root = quiet_root("enriched");
        let kd = root.join(".context-init/knowledge");
        let (analysis, subs) = deterministic(&root);
        generate(&analysis, &subs, &kd, &GenerateOptions::default()).unwrap();
        let mut index = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        index.subsystems[0].summary = "Ручное описание".to_string();
        let id = index.subsystems[0].id.clone();
        fs::write(
            kd.join("L0_index.json"),
            format!("{}\n", serde_json::to_string_pretty(&index).unwrap()),
        )
        .unwrap();
        update(&analysis, &subs, &kd, None).unwrap();
        let after = L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        let sub = after.subsystems.iter().find(|s| s.id == id).unwrap();
        assert_eq!(sub.summary, "Ручное описание");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn agents_md_wording_follows_mode() {
        let dir = quiet_root("wording");
        let subs = [named_sub("core", "c")];
        update_or_create_root_agents_md(&dir, "p", &subs, StorageMode::Repo).unwrap();
        let repo = fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        assert!(repo.contains(".context-init/knowledge/"), "{repo}");
        assert!(repo.contains("закоммитьте"), "{repo}");
        assert!(!repo.contains("вне репозитория"), "{repo}");
        update_or_create_root_agents_md(&dir, "p", &subs, StorageMode::Central).unwrap();
        let central = fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        assert!(central.contains("вне репозитория"), "{central}");
        assert!(!central.contains("закоммитьте"), "{central}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn update_keeps_hand_edited_generated_card() {
        let root = quiet_root("handcard");
        let kd = root.join(".context-init/knowledge");
        let (analysis, subs) = deterministic(&root);
        generate(&analysis, &subs, &kd, &GenerateOptions::default()).unwrap();
        let card = kd.join("subsystems").join(format!("{}.md", subs[0].id));
        let mut text = fs::read_to_string(&card).unwrap();
        text.push_str("\n## Notes\nhand-written\n");
        fs::write(&card, &text).unwrap();
        update(&analysis, &subs, &kd, None).unwrap();
        assert_eq!(fs::read_to_string(&card).unwrap(), text);
        let _ = fs::remove_dir_all(&root);
    }

    fn git(root: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn git_repo(tag: &str) -> std::path::PathBuf {
        let root = quiet_root(tag);
        git(&root, &["init", "-q"]);
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "init"]);
        root
    }

    #[test]
    fn generated_l1_cards_parse_as_valid_okf_cards() {
        for root in [git_repo("okf_git"), quiet_root("okf_plain")] {
            let kd = root.join(".context-init/knowledge");
            let (analysis, subs) = deterministic(&root);
            generate(&analysis, &subs, &kd, &GenerateOptions::default()).unwrap();
            for sub in &subs {
                let text = fs::read_to_string(kd.join(&sub.l1_path)).unwrap();
                let card = crate::openviking::OkfCard::parse(&text)
                    .unwrap_or_else(|e| panic!("{}: {e:#}\n{text}", sub.id));
                assert!(!card.frontmatter.directory.is_empty(), "{}", sub.id);
                assert!(!card.frontmatter.source.blob_sha.is_empty(), "{}", sub.id);
                assert!(card.body.starts_with("# Подсистема:"), "{}", card.body);
                assert!(
                    card.l0_abstract().contains(sub.summary.trim()),
                    "{}",
                    sub.id
                );
            }
            let _ = fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn update_twice_with_okf_cards_remains_noop() {
        let root = git_repo("okf_noop");
        let kd = root.join(".context-init/knowledge");
        let (analysis, subs) = deterministic(&root);
        generate(&analysis, &subs, &kd, &GenerateOptions::default()).unwrap();
        update(&analysis, &subs, &kd, None).unwrap();
        let before = snapshot(&kd, &[]);
        // HEAD moves, but nothing under any subsystem changes.
        fs::write(root.join("NOTES.md"), "unrelated\n").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "docs"]);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let (analysis, subs) = deterministic(&root);
        update(&analysis, &subs, &kd, None).unwrap();
        assert!(before == snapshot(&kd, &[]), "update rewrote cards");
        let _ = fs::remove_dir_all(&root);
    }
}
