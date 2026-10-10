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
        if analysis.detected_modules.is_empty() {
            // Плоский проект
            subsystems.push(SubsystemInfo {
                id: "core".to_string(),
                name: "Core Logic".to_string(),
                summary: format!("Основная логика и компоненты проекта {}", analysis.name),
                l1_path: "subsystems/core.md".to_string(),
                tags: vec!["core".to_string(), "main".to_string()],
                entrypoints: analysis.root_entrypoints.clone(),
                key_files: analysis.root_entrypoints.clone(),
                files: Vec::new(),
                key_symbols: Vec::new(),
                test_command: analysis.commands.test.clone(),
                tokens: 0,
            });
        } else {
            for m in &analysis.detected_modules {
                let clean_id = generator::sanitize_id(&m.dir_path);
                let capitalized = m
                    .name
                    .replace(['_', '-'], " ")
                    .split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                let summary = if !m.entrypoints.is_empty() {
                    format!(
                        "Подсистема `{}` с ключевыми точками входа: {}",
                        m.dir_path,
                        m.entrypoints.join(", ")
                    )
                } else {
                    format!(
                        "Модуль и компоненты директории `{}` ({} файлов)",
                        m.dir_path,
                        m.files.len()
                    )
                };

                let key_files = m.files.iter().map(|f| f.path.clone()).collect();

                subsystems.push(SubsystemInfo {
                    id: clean_id.clone(),
                    name: capitalized,
                    summary,
                    l1_path: format!("subsystems/{}.md", clean_id),
                    tags: vec![clean_id, analysis.project_type.display_name().to_string()],
                    entrypoints: m.entrypoints.clone(),
                    key_files,
                    files: m.files.clone(),
                    key_symbols: m.symbols.clone(),
                    test_command: m.test_cmd.clone(),
                    tokens: 0,
                });
            }
        }
    }

    for sub in &mut subsystems {
        compute_subsystem_tokens(&analysis.root_dir, sub);
    }

    Ok(subsystems)
}

fn compute_subsystem_tokens(root_dir: &Path, sub: &mut SubsystemInfo) {
    let mut total_tokens = 0;
    if !sub.files.is_empty() {
        for f in &sub.files {
            let path = root_dir.join(&f.path);
            if let Ok(content) = std::fs::read_to_string(&path) {
                total_tokens += detector::count_tokens(&content);
            }
        }
    } else {
        for path_str in &sub.key_files {
            let path = root_dir.join(path_str);
            if let Ok(content) = std::fs::read_to_string(&path) {
                total_tokens += detector::count_tokens(&content);
            }
        }
    }
    sub.tokens = total_tokens;
}

pub fn pack_builtin_to_string(
    root: &Path,
    subs: &[&SubsystemInfo],
    files: &[String],
    compress: bool,
    style: &str,
) -> String {
    let mut buffer = String::new();

    if style.eq_ignore_ascii_case("markdown") {
        buffer.push_str("# 📦 AI Context Pack: ");
        let names: Vec<&str> = subs.iter().map(|s| s.name.as_str()).collect();
        buffer.push_str(&names.join(", "));
        buffer.push_str("\n\n> Сгенерировано context-init (встроенный AST-пакер)\n\n");

        for file_rel in files {
            let full_p = root.join(file_rel);
            if let Ok(content) = std::fs::read_to_string(&full_p) {
                let ext = full_p.extension().and_then(|e| e.to_str()).unwrap_or("");
                let processed = if compress {
                    compress_file_content(&content, ext)
                } else {
                    content
                };
                buffer.push_str(&format!(
                    "## File: `{}`\n```{}\n{}\n```\n\n",
                    file_rel, ext, processed
                ));
            }
        }
    } else {
        buffer.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        buffer.push_str("<context_pack generated_by=\"context-init\">\n");
        for sub in subs {
            buffer.push_str(&format!(
                "  <subsystem id=\"{}\" name=\"{}\">\n    <summary>{}</summary>\n  </subsystem>\n",
                sub.id, sub.name, sub.summary
            ));
        }
        buffer.push_str("  <files>\n");

        for file_rel in files {
            let full_p = root.join(file_rel);
            if let Ok(content) = std::fs::read_to_string(&full_p) {
                let ext = full_p.extension().and_then(|e| e.to_str()).unwrap_or("");
                let processed = if compress {
                    compress_file_content(&content, ext)
                } else {
                    content
                };
                buffer.push_str(&format!(
                    "    <file path=\"{}\">\n<![CDATA[\n{}\n]]>\n    </file>\n",
                    file_rel, processed
                ));
            }
        }
        buffer.push_str("  </files>\n</context_pack>\n");
    }

    buffer
}

pub fn compress_file_content(content: &str, ext: &str) -> String {
    if ext.eq_ignore_ascii_case("rs") {
        if let Some(compressed) = compress_rust_ast(content) {
            return compressed;
        }
    }
    compress_code(content)
}

pub fn compress_rust_ast(content: &str) -> Option<String> {
    let parsed = syn::parse_file(content).ok()?;
    let mut file_attrs = Vec::new();
    for attr in &parsed.attrs {
        file_attrs.push(normalize_rust_tokens(&quote::quote!(#attr).to_string()));
    }
    let body = format_rust_items(&parsed.items);
    if file_attrs.is_empty() {
        Some(body)
    } else {
        Some(format!("{}\n\n{}", file_attrs.join("\n"), body))
    }
}

pub fn format_rust_items(items: &[syn::Item]) -> String {
    let mut formatted_items = Vec::new();
    for item in items {
        match item {
            syn::Item::Fn(f) => {
                let attrs = &f.attrs;
                let vis = &f.vis;
                let sig = &f.sig;
                let sig_str = quote::quote!(#vis #sig).to_string();
                let clean_sig = normalize_rust_tokens(&sig_str);
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                if attr_strs.is_empty() {
                    formatted_items.push(format!("{} {{\n    ⋮----\n}}", clean_sig));
                } else {
                    formatted_items.push(format!(
                        "{}\n{} {{\n    ⋮----\n}}",
                        attr_strs.join("\n"),
                        clean_sig
                    ));
                }
            }
            syn::Item::Impl(imp) => {
                let attrs = &imp.attrs;
                let defaultness = &imp.defaultness;
                let unsafety = &imp.unsafety;
                let generics = &imp.generics;
                let (impl_generics, _, where_clause) = generics.split_for_impl();
                let self_ty = &imp.self_ty;
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let header_raw = if let Some((bang, path, for_tok)) = &imp.trait_ {
                    quote::quote!(#defaultness #unsafety impl #impl_generics #bang #path #for_tok #self_ty #where_clause).to_string()
                } else {
                    quote::quote!(#defaultness #unsafety impl #impl_generics #self_ty #where_clause)
                        .to_string()
                };
                let clean_header_core = normalize_rust_tokens(&header_raw);
                let clean_header = if attr_strs.is_empty() {
                    clean_header_core
                } else {
                    format!("{}\n{}", attr_strs.join("\n"), clean_header_core)
                };
                let mut item_strs = Vec::new();
                for it in &imp.items {
                    if let syn::ImplItem::Fn(m) = it {
                        let m_attrs: Vec<String> = m
                            .attrs
                            .iter()
                            .map(|a| {
                                format!(
                                    "    {}",
                                    normalize_rust_tokens(&quote::quote!(#a).to_string())
                                )
                            })
                            .collect();
                        let vis = &m.vis;
                        let defaultness = &m.defaultness;
                        let sig = &m.sig;
                        let sig_str = quote::quote!(#vis #defaultness #sig).to_string();
                        let clean_sig = normalize_rust_tokens(&sig_str);
                        if m_attrs.is_empty() {
                            item_strs.push(format!("    {} {{\n        ⋮----\n    }}", clean_sig));
                        } else {
                            item_strs.push(format!(
                                "{}\n    {} {{\n        ⋮----\n    }}",
                                m_attrs.join("\n"),
                                clean_sig
                            ));
                        }
                    } else {
                        let other_str = quote::quote!(#it).to_string();
                        item_strs.push(format!("    {}", normalize_rust_tokens(&other_str)));
                    }
                }
                if item_strs.is_empty() {
                    formatted_items.push(format!("{} {{}}", clean_header));
                } else {
                    formatted_items.push(format!(
                        "{} {{\n{}\n}}",
                        clean_header,
                        item_strs.join("\n\n")
                    ));
                }
            }
            syn::Item::Trait(t) => {
                let attrs = &t.attrs;
                let vis = &t.vis;
                let ident = &t.ident;
                let generics = &t.generics;
                let (impl_generics, _, where_clause) = generics.split_for_impl();
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let header_raw =
                    quote::quote!(#vis trait #ident #impl_generics #where_clause).to_string();
                let clean_header_core = normalize_rust_tokens(&header_raw);
                let clean_header = if attr_strs.is_empty() {
                    clean_header_core
                } else {
                    format!("{}\n{}", attr_strs.join("\n"), clean_header_core)
                };
                let mut trait_items = Vec::new();
                for it in &t.items {
                    if let syn::TraitItem::Fn(tf) = it {
                        let tf_attrs: Vec<String> = tf
                            .attrs
                            .iter()
                            .map(|a| {
                                format!(
                                    "    {}",
                                    normalize_rust_tokens(&quote::quote!(#a).to_string())
                                )
                            })
                            .collect();
                        let sig = &tf.sig;
                        let sig_str = quote::quote!(#sig).to_string();
                        let clean_sig = normalize_rust_tokens(&sig_str);
                        let body = if tf.default.is_some() {
                            format!("    {} {{\n        ⋮----\n    }}", clean_sig)
                        } else {
                            format!("    {};", clean_sig)
                        };
                        if tf_attrs.is_empty() {
                            trait_items.push(body);
                        } else {
                            trait_items.push(format!("{}\n{}", tf_attrs.join("\n"), body));
                        }
                    } else {
                        let other_str = quote::quote!(#it).to_string();
                        trait_items.push(format!("    {}", normalize_rust_tokens(&other_str)));
                    }
                }
                if trait_items.is_empty() {
                    formatted_items.push(format!("{} {{}}", clean_header));
                } else {
                    formatted_items.push(format!(
                        "{} {{\n{}\n}}",
                        clean_header,
                        trait_items.join("\n\n")
                    ));
                }
            }
            syn::Item::Mod(m) => {
                let attrs = &m.attrs;
                let vis = &m.vis;
                let ident = &m.ident;
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let mod_header_raw = quote::quote!(#vis mod #ident).to_string();
                let clean_header_core = normalize_rust_tokens(&mod_header_raw);
                let clean_header = if attr_strs.is_empty() {
                    clean_header_core
                } else {
                    format!("{}\n{}", attr_strs.join("\n"), clean_header_core)
                };
                if let Some((_, inner_items)) = &m.content {
                    let inner = format_rust_items(inner_items);
                    let indented: Vec<String> = inner
                        .lines()
                        .map(|l| {
                            if l.is_empty() {
                                String::new()
                            } else {
                                format!("    {}", l)
                            }
                        })
                        .collect();
                    formatted_items.push(format!(
                        "{} {{\n{}\n}}",
                        clean_header,
                        indented.join("\n")
                    ));
                } else {
                    formatted_items.push(format!("{};", clean_header));
                }
            }
            other => {
                let s = quote::quote!(#other).to_string();
                formatted_items.push(normalize_rust_tokens(&s));
            }
        }
    }
    formatted_items.join("\n\n")
}

pub fn normalize_rust_tokens(s: &str) -> String {
    let s = s
        .replace("# ! [ ", "#![")
        .replace("# ! [", "#![")
        .replace("#![ ", "#![")
        .replace("# ! ", "#!")
        .replace("# !", "#!")
        .replace("# [ ", "#[")
        .replace("# [", "#[")
        .replace("#[ ", "#[")
        .replace(" < ", "<")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace(" > ", "> ")
        .replace(" ( ", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" : ", ": ")
        .replace(" , ", ", ")
        .replace(" ;", ";")
        .replace(" :: ", "::")
        .replace("( &", "(&")
        .replace("(& ", "(&")
        .replace("& mut ", "&mut ")
        .replace("& '", "&'")
        .replace("& ", "&");

    let words: Vec<&str> = s.split_whitespace().collect();
    let joined = words.join(" ");
    joined
        .replace(" (", "(")
        .replace(":: ", "::")
        .replace(" ::", "::")
        .replace(":(", ": (")
        .replace("->(", "-> (")
        .replace("+(", "+ (")
}

pub fn compress_code(content: &str) -> String {
    let mut result = Vec::new();
    let mut prev_was_empty = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_was_empty {
                result.push("");
                prev_was_empty = true;
            }
        } else {
            prev_was_empty = false;
            result.push(line.trim_end());
        }
    }

    result.join("\n")
}
