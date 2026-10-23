use anyhow::{Context, Result};
use std::path::Path;

use crate::detector;
use crate::generator;
use crate::ollama;
use crate::types::{L0Index, McpCheckResult, ProjectAnalysis, SubsystemInfo};

/// Опции инициализации базы знаний
#[derive(Debug, Clone)]
pub struct ServiceInitOptions<'a> {
    pub path: &'a Path,
    pub ai: bool,
    pub ollama_url: &'a str,
    pub model: &'a str,
    pub force: bool,
    pub dry_run: bool,
    pub deep: bool,
    pub with_agents_md: bool,
    pub with_rtk: bool,
    pub no_rtk: bool,
    pub compact: bool,
}

/// Опции обновления базы знаний
#[derive(Debug, Clone)]
pub struct ServiceUpdateOptions<'a> {
    pub path: &'a Path,
    pub ai: bool,
    pub ollama_url: &'a str,
    pub model: &'a str,
    pub deep: bool,
    pub with_agents_md: bool,
    pub with_rtk: bool,
    pub no_rtk: bool,
    pub compact: bool,
}

/// Сервисный фасад для операций с базой знаний
pub struct ContextService;

impl ContextService {
    /// Выполнить аудит архитектурного дрейфа и вернуть структурированный отчет
    pub fn audit_drift(path: &Path) -> Result<McpCheckResult> {
        let analysis = detector::analyze_project(path, false, false)?;
        let report = generator::audit_knowledge_base(&analysis)?;
        let has_drift =
            !report.missing_in_fs.is_empty() || !report.untracked_in_knowledge.is_empty();

        let message = if !has_drift {
            format!("OK: {} subsystems up-to-date", report.up_to_date_count)
        } else {
            format!(
                "DRIFT: up_to_date={}, missing=[{}], untracked=[{}]",
                report.up_to_date_count,
                report.missing_in_fs.join(", "),
                report.untracked_in_knowledge.join(", ")
            )
        };

        Ok(McpCheckResult {
            has_drift,
            missing_in_fs: report.missing_in_fs,
            untracked_in_knowledge: report.untracked_in_knowledge,
            up_to_date_count: report.up_to_date_count,
            message,
        })
    }

    /// Найти директорию с базой знаний OpenViking (поддерживает плоскую структуру и вложенные dotfiles/<project>)
    pub fn find_knowledge_dir(path: &Path) -> Result<std::path::PathBuf> {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => path.to_path_buf(),
        };

        // 1. Прямая директория .agents/knowledge/L0_index.json
        let direct = canonical.join(".agents").join("knowledge");
        if direct.join("L0_index.json").exists() {
            return Ok(direct);
        }

        // 2. Вложенные директории внутри .agents/knowledge/ (например, .agents/knowledge/dotfiles)
        if direct.exists() && direct.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&direct) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && p.join("L0_index.json").exists() {
                        return Ok(p);
                    }
                }
            }
        }

        // 3. Директория knowledge/ в корне
        let alt = canonical.join("knowledge");
        if alt.join("L0_index.json").exists() {
            return Ok(alt);
        }

        // 4. L0_index.json прямо в корне
        if canonical.join("L0_index.json").exists() {
            return Ok(canonical);
        }

        // 5. Поиск вверх по дереву каталогов
        let mut curr = canonical.parent();
        while let Some(parent) = curr {
            let direct = parent.join(".agents").join("knowledge");
            if direct.join("L0_index.json").exists() {
                return Ok(direct);
            }
            if parent.join("L0_index.json").exists() {
                return Ok(parent.to_path_buf());
            }
            curr = parent.parent();
        }

        anyhow::bail!(
            "База знаний не найдена по пути {:?}. Сначала выполните инициализацию.",
            direct.join("L0_index.json")
        );
    }

    /// Прочитать L0_index.json проекта
    pub fn get_l0_index(path: &Path) -> Result<L0Index> {
        let kdir = Self::find_knowledge_dir(path)?;
        let l0_json_path = kdir.join("L0_index.json");

        let content = std::fs::read_to_string(&l0_json_path)
            .with_context(|| format!("Не удалось прочитать {:?}", l0_json_path))?;
        let index: L0Index = serde_json::from_str(&content)
            .with_context(|| format!("Ошибка десериализации L0_index.json в {:?}", l0_json_path))?;
        Ok(index)
    }

    /// Получить список всех подсистем проекта
    pub fn get_subsystems(path: &Path) -> Result<Vec<SubsystemInfo>> {
        let index = Self::get_l0_index(path)?;
        Ok(index.subsystems)
    }

    /// Получить компактную L1-карточку конкретной подсистемы
    pub fn get_subsystem(path: &Path, subsystem_id: &str) -> Result<String> {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => path.to_path_buf(),
        };
        let kdir = Self::find_knowledge_dir(&canonical)?;
        let index = Self::get_l0_index(&canonical)?;

        let clean_input = subsystem_id
            .trim()
            .trim_start_matches("subsystems/")
            .trim_start_matches("subsystems\\")
            .trim_end_matches(".md");

        let matched = index.subsystems.iter().find(|s| {
            s.id.eq_ignore_ascii_case(subsystem_id)
                || s.name.eq_ignore_ascii_case(subsystem_id)
                || s.id
                    .eq_ignore_ascii_case(&generator::sanitize_id(subsystem_id))
                || s.id.eq_ignore_ascii_case(clean_input)
                || s.name.eq_ignore_ascii_case(clean_input)
                || s.id
                    .eq_ignore_ascii_case(&generator::sanitize_id(clean_input))
        });

        if let Some(sub) = matched {
            // 1. Попробовать прочитать L1 файл по l1_path
            let l1_file = kdir.join(&sub.l1_path);
            if l1_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&l1_file) {
                    return Ok(content);
                }
            }

            // 2. Попробовать прочитать subsystems/<id>.md
            let alt_l1 = kdir.join("subsystems").join(format!("{}.md", sub.id));
            if alt_l1.exists() {
                if let Ok(content) = std::fs::read_to_string(&alt_l1) {
                    return Ok(content);
                }
            }

            // 3. Синтезировать компактную карточку из L0_index
            let mut card = format!(
                "# Подсистема: {} (L1)\n\n> **Идентификатор:** `{}`\n> **Теги:** {} | **Объем:** ~{} токенов BPE\n\n## 🎯 Назначение и роль\n{}\n\n",
                sub.name,
                sub.id,
                sub.tags.join(", "),
                sub.tokens,
                sub.summary
            );

            if !sub.entrypoints.is_empty() {
                card.push_str("## 🚪 Точки входа\n");
                for ep in &sub.entrypoints {
                    card.push_str(&format!("- `{}`\n", ep));
                }
                card.push('\n');
            }

            if !sub.key_files.is_empty() {
                card.push_str("## 📁 Ключевые файлы\n");
                for kf in &sub.key_files {
                    card.push_str(&format!("- `{}`\n", kf));
                }
                card.push('\n');
            }

            if let Some(cmd) = &sub.test_command {
                card.push_str(&format!(
                    "## 🧪 Запуск и тестирование\n```bash\n{}\n```\n",
                    cmd
                ));
            }

            Ok(card)
        } else {
            // Проверить прямое наличие subsystems/<clean_id>.md
            let clean_id = generator::sanitize_id(clean_input);
            let direct_l1 = kdir.join("subsystems").join(format!("{}.md", clean_id));
            if direct_l1.exists() {
                if let Ok(content) = std::fs::read_to_string(&direct_l1) {
                    return Ok(content);
                }
            }

            let available: Vec<String> = index.subsystems.iter().map(|s| s.id.clone()).collect();
            anyhow::bail!(
                "Подсистема '{}' не найдена в L0_index.json. Доступные подсистемы: {}",
                subsystem_id,
                available.join(", ")
            )
        }
    }

    /// Упаковать файлы подсистемы в сжатый AI-контекст и вернуть его в виде строки
    pub fn pack_subsystem_to_string(
        path: &Path,
        subsystem_id: &str,
        compress: bool,
        style: &str,
    ) -> Result<(String, usize, usize)> {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => path.to_path_buf(),
        };
        let kdir = Self::find_knowledge_dir(&canonical)?;
        let index = Self::get_l0_index(&canonical)?;

        let matched_subs: Vec<&SubsystemInfo> = if subsystem_id.eq_ignore_ascii_case("all") {
            index.subsystems.iter().collect()
        } else {
            index
                .subsystems
                .iter()
                .filter(|s| {
                    s.id.eq_ignore_ascii_case(subsystem_id)
                        || s.name.eq_ignore_ascii_case(subsystem_id)
                        || s.id
                            .eq_ignore_ascii_case(&generator::sanitize_id(subsystem_id))
                })
                .collect()
        };

        if matched_subs.is_empty() {
            let available: Vec<String> = index.subsystems.iter().map(|s| s.id.clone()).collect();
            anyhow::bail!(
                "Подсистема '{}' не найдена в L0_index.json. Доступные подсистемы: {}",
                subsystem_id,
                available.join(", ")
            );
        }

        let mut files_to_pack = Vec::new();
        for sub in &matched_subs {
            if !sub.files.is_empty() {
                for f in &sub.files {
                    if !files_to_pack.contains(&f.path) && canonical.join(&f.path).exists() {
                        files_to_pack.push(f.path.clone());
                    }
                }
            } else if !sub.key_files.is_empty() {
                for kf in &sub.key_files {
                    if !files_to_pack.contains(kf) && canonical.join(kf).exists() {
                        files_to_pack.push(kf.clone());
                    }
                }
            } else {
                let l1_file = kdir.join(&sub.l1_path);
                if l1_file.exists() {
                    let rel = l1_file
                        .strip_prefix(&canonical)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| sub.l1_path.clone());
                    if !files_to_pack.contains(&rel) {
                        files_to_pack.push(rel);
                    }
                }
            }
        }

        if files_to_pack.is_empty() {
            anyhow::bail!(
                "Для подсистемы '{}' не найдено существующих файлов на диске.",
                subsystem_id
            );
        }

        let buffer =
            pack_builtin_to_string(&canonical, &matched_subs, &files_to_pack, compress, style);
        let tokens = detector::count_tokens(&buffer);
        let files_count = files_to_pack.len();

        Ok((buffer, tokens, files_count))
    }

    /// Инициализировать базу знаний
    pub fn init_knowledge_base(
        opts: ServiceInitOptions,
    ) -> Result<(ProjectAnalysis, Vec<SubsystemInfo>)> {
        let use_rtk = if opts.no_rtk {
            false
        } else {
            opts.with_rtk || detector::is_rtk_available()
        };

        let analysis = detector::analyze_project(opts.path, opts.deep, use_rtk)?;
        let subsystems = build_subsystems(
            &analysis,
            opts.ai,
            opts.ollama_url,
            opts.model,
            opts.compact,
        )?;

        if !opts.dry_run {
            generator::generate_knowledge_base(
                &analysis,
                &subsystems,
                opts.force,
                opts.with_agents_md,
            )?;
        }

        Ok((analysis, subsystems))
    }

    /// Инкрементально обновить базу знаний
    pub fn update_knowledge_base(opts: ServiceUpdateOptions) -> Result<(usize, usize)> {
        let use_rtk = if opts.no_rtk {
            false
        } else {
            opts.with_rtk || detector::is_rtk_available()
        };

        let analysis = detector::analyze_project(opts.path, opts.deep, use_rtk)?;
        let subsystems = build_subsystems(
            &analysis,
            opts.ai,
            opts.ollama_url,
            opts.model,
            opts.compact,
        )?;

        let (added, updated) =
            generator::update_knowledge_base(&analysis, &subsystems, opts.with_agents_md)?;

        Ok((added, updated))
    }
}

pub fn build_subsystems(
    analysis: &ProjectAnalysis,
    ai: bool,
    ollama_url: &str,
    model: &str,
    _compact: bool,
) -> Result<Vec<SubsystemInfo>> {
    let mut subsystems: Vec<SubsystemInfo> = Vec::new();

    if ai && ollama::is_ollama_online(ollama_url) {
        if let Ok(ai_subs) = ollama::enrich_subsystems_with_ai(analysis, ollama_url, model) {
            if !ai_subs.is_empty() {
                for sub in ai_subs {
                    let matching_mod = analysis.detected_modules.iter().find(|m| {
                        let clean = generator::sanitize_id(&m.dir_path);
                        clean == sub.id
                            || clean == format!("src_{}", sub.id)
                            || m.name == sub.name
                            || m.name == sub.id
                            || (sub.id == "core" && (m.name == "main" || m.name == "core"))
                    });

                    let entrypoints = matching_mod
                        .map(|m| m.entrypoints.clone())
                        .unwrap_or_default();
                    let key_files = matching_mod
                        .map(|m| m.files.iter().map(|f| f.path.clone()).collect())
                        .unwrap_or_default();
                    let files = matching_mod.map(|m| m.files.clone()).unwrap_or_default();
                    let key_symbols = matching_mod.map(|m| m.symbols.clone()).unwrap_or_default();
                    let test_cmd = matching_mod.and_then(|m| m.test_cmd.clone());

                    subsystems.push(SubsystemInfo {
                        l1_path: format!("subsystems/{}.md", sub.id),
                        id: sub.id,
                        name: sub.name,
                        summary: sub.summary,
                        tags: sub.tags,
                        entrypoints,
                        key_files,
                        files,
                        key_symbols,
                        test_command: test_cmd,
                        tokens: 0,
                    });
                }
            }
        }
    }

    if subsystems.is_empty() {
        subsystems = ci_core::analysis::subsystems::build_deterministic(analysis);
    }
    Ok(subsystems)
}

pub use ci_core::analysis::pack::{
    compress_code, compress_rust_ast, format_rust_items, normalize_rust_tokens,
};

/// Transitional wrapper; removed in Task 9.
pub fn pack_builtin_to_string(
    root: &Path,
    subs: &[&SubsystemInfo],
    files: &[String],
    compress: bool,
    style: &str,
) -> String {
    ci_core::analysis::pack::pack_files(
        root,
        subs,
        files,
        compress,
        ci_core::analysis::pack::PackStyle::parse(style),
    )
}

/// Transitional wrapper; removed in Task 9.
pub fn compress_file_content(content: &str, ext: &str) -> String {
    ci_core::analysis::pack::compress_source(content, ext)
}
