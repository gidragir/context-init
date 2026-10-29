//! Built-in AST context packer (shared by CLI `pack` and MCP `context_pack`).

use std::path::Path;

use anyhow::{bail, Result};

use crate::storage::{safe_join, L0Index, SubsystemInfo};

/// Output format of a context pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackStyle {
    #[default]
    Xml,
    Markdown,
}

impl PackStyle {
    /// Parses `"markdown"` (case-insensitive) as Markdown; anything else is XML.
    pub fn parse(style: &str) -> Self {
        if style.eq_ignore_ascii_case("markdown") {
            Self::Markdown
        } else {
            Self::Xml
        }
    }
}

/// A packed subsystem selection.
#[derive(Debug, Clone)]
pub struct PackedSubsystems {
    /// Packed context text.
    pub content: String,
    /// Relative paths of the packed files.
    pub files: Vec<String>,
    /// Subsystems that matched the requested id.
    pub subsystems: Vec<SubsystemInfo>,
}

/// Selects subsystems by id or name (`all` = every subsystem) and packs their files that
/// exist on disk. A subsystem without file lists (legacy indexes) contributes its L1 card
/// from `knowledge_dir`. Shared by CLI `pack` and MCP `context_pack`.
pub fn pack_subsystem(
    root: &Path,
    knowledge_dir: &Path,
    index: &L0Index,
    subsystem_id: &str,
    compress: bool,
    style: PackStyle,
) -> Result<PackedSubsystems> {
    let wanted = subsystem_id.trim();
    let clean = wanted
        .trim_start_matches("subsystems/")
        .trim_end_matches(".md");
    let matched: Vec<&SubsystemInfo> = if wanted.eq_ignore_ascii_case("all") {
        index.subsystems.iter().collect()
    } else {
        index
            .subsystems
            .iter()
            .filter(|s| {
                [wanted, clean]
                    .iter()
                    .any(|w| s.id.eq_ignore_ascii_case(w) || s.name.eq_ignore_ascii_case(w))
            })
            .collect()
    };
    if matched.is_empty() {
        let available: Vec<&str> = index.subsystems.iter().map(|s| s.id.as_str()).collect();
        bail!(
            "Subsystem '{}' not found in L0 index. Available subsystems: {}",
            subsystem_id,
            available.join(", ")
        );
    }

    let mut files: Vec<String> = Vec::new();
    for sub in &matched {
        if sub.files.is_empty() && sub.key_files.is_empty() {
            let Some(card) = safe_join(knowledge_dir, &sub.l1_path) else {
                continue;
            };
            if card.is_file() {
                // Relative when the card lives in the repo, absolute for central storage.
                let path = card
                    .strip_prefix(root)
                    .unwrap_or(&card)
                    .to_string_lossy()
                    .replace('\\', "/");
                if !files.contains(&path) {
                    files.push(path);
                }
            }
            continue;
        }
        let candidates = sub
            .files
            .iter()
            .map(|f| &f.path)
            .chain(sub.key_files.iter());
        for path in candidates {
            // The index is committed and may come from a clone: never pack secrets or databases.
            let inside = !is_secret_path(path) && safe_join(root, path).is_some_and(|p| p.exists());
            if inside && !files.contains(path) {
                files.push(path.clone());
            }
        }
    }
    if files.is_empty() {
        bail!(
            "No existing source files found on disk for subsystem '{}'",
            subsystem_id
        );
    }

    let content = pack_files(root, &matched, &files, compress, style);
    Ok(PackedSubsystems {
        content,
        files,
        subsystems: matched.into_iter().cloned().collect(),
    })
}

/// Files that must never reach a pack: env files, keys, tokens, local secrets, databases.
fn is_secret_path(path: &str) -> bool {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || name == "mise.local.toml"
        || [".key", ".token", ".pem", ".db", ".sqlite", ".sqlite3"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

/// Packs `files` (relative to `root`) of `subs` into one AI-ready context string,
/// optionally AST-compressed.
pub fn pack_files(
    root: &Path,
    subs: &[&SubsystemInfo],
    files: &[String],
    compress: bool,
    style: PackStyle,
) -> String {
    let mut buffer = String::new();

    if style == PackStyle::Markdown {
        buffer.push_str("# 📦 AI Context Pack: ");
        let names: Vec<&str> = subs.iter().map(|s| s.name.as_str()).collect();
        buffer.push_str(&names.join(", "));
        buffer.push_str("\n\n> Сгенерировано context-init (встроенный AST-пакер)\n\n");

        for file_rel in files {
            let full_p = root.join(file_rel);
            if let Ok(content) = std::fs::read_to_string(&full_p) {
                let ext = full_p.extension().and_then(|e| e.to_str()).unwrap_or("");
                let processed = if compress {
                    compress_source(&content, ext)
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
                    compress_source(&content, ext)
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

/// Compresses source text through the language adapter of `ext` (D7: Rust via AST, BSL by
/// signatures, ...); unknown languages get whitespace normalization.
pub fn compress_source(content: &str, ext: &str) -> String {
    match crate::analysis::lang::adapter_for_ext(ext) {
        Some(adapter) => adapter.compress_source(content),
        None => compress_code(content),
    }
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

#[cfg(test)]
mod packer_tests {
    use super::*;

    #[test]
    fn test_compress_code() {
        let raw = "fn hello() {\n\n\n    println!(\"hi\");   \n\n\n}\n";
        let compressed = compress_code(raw);
        assert_eq!(compressed, "fn hello() {\n\n    println!(\"hi\");\n\n}");
    }

    #[test]
    fn test_compress_rust_ast() {
        let code = r#"
use std::collections::HashMap;

pub struct Service {
    pub name: String,
}

impl Service {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

pub fn run() {
    println!("running");
}
"#;
        let compressed = compress_source(code, "rs");
        assert!(compressed.contains("pub fn run() {\n    ⋮----\n}"));
        assert!(compressed.contains("pub fn new(name: String) -> Self {\n        ⋮----\n    }"));
        assert!(compressed.contains("pub struct Service { pub name: String, }"));
    }

    #[test]
    fn test_compress_rust_ast_with_attributes_and_references() {
        let code = r#"
#![allow(unused)]

#[inline]
pub fn process(text: &str) -> &str {
    text
}

impl Service {
    #[inline]
    pub fn handle(&self, query: &str) -> bool {
        query.is_empty()
    }
}

#[cfg(test)]
mod tests {
    pub fn internal() {
        println!("test");
    }
}
"#;
        let compressed = compress_source(code, "rs");
        assert!(compressed.contains("#![allow(unused)]"));
        assert!(
            compressed.contains("#[inline]\npub fn process(text: &str) -> &str {\n    ⋮----\n}")
        );
        assert!(compressed.contains(
            "#[inline]\n    pub fn handle(&self, query: &str) -> bool {\n        ⋮----\n    }"
        ));
        assert!(compressed.contains(
            "#[cfg(test)]\nmod tests {\n    pub fn internal() {\n        ⋮----\n    }\n}"
        ));
    }

    #[test]
    fn test_pack_fallback_xml_and_md() {
        let temp_dir = std::env::temp_dir().join(format!("test_pack_sub_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("lib.rs");
        std::fs::write(&test_file, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        let sub = SubsystemInfo {
            id: "core".to_string(),
            name: "Core".to_string(),
            summary: "Core logic".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec!["core".to_string()],
            entrypoints: vec!["lib.rs".to_string()],
            key_files: vec!["lib.rs".to_string()],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 10,
            parent: None,
        };

        let files = vec!["lib.rs".to_string()];
        let out_xml = temp_dir.join("pack.xml");
        let out_md = temp_dir.join("pack.md");

        let xml_str = pack_files(&temp_dir, &[&sub], &files, true, PackStyle::parse("xml"));
        std::fs::write(&out_xml, &xml_str).unwrap();
        assert!(out_xml.exists());
        let xml_content = std::fs::read_to_string(&out_xml).unwrap();
        assert!(xml_content.contains("<context_pack"));
        assert!(xml_content.contains("lib.rs"));
        assert!(xml_content.contains("⋮----"));

        let md_str = pack_files(
            &temp_dir,
            &[&sub],
            &files,
            true,
            PackStyle::parse("markdown"),
        );
        std::fs::write(&out_md, &md_str).unwrap();
        assert!(out_md.exists());
        let md_content = std::fs::read_to_string(&out_md).unwrap();
        assert!(md_content.contains("# 📦 AI Context Pack:"));
        assert!(md_content.contains("## File: `lib.rs`"));
        assert!(md_content.contains("⋮----"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pack_builtin_no_compress() {
        let temp_dir = std::env::temp_dir().join(format!("test_pack_nc_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("lib.rs");
        std::fs::write(&test_file, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        let sub = SubsystemInfo {
            id: "core".to_string(),
            name: "Core".to_string(),
            summary: "Core logic".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec!["core".to_string()],
            entrypoints: vec!["lib.rs".to_string()],
            key_files: vec!["lib.rs".to_string()],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 10,
            parent: None,
        };

        let files = vec!["lib.rs".to_string()];
        let out_xml = temp_dir.join("pack_nc.xml");
        let out_md = temp_dir.join("pack_nc.md");

        let xml_str = pack_files(&temp_dir, &[&sub], &files, false, PackStyle::parse("xml"));
        std::fs::write(&out_xml, &xml_str).unwrap();
        let xml_content = std::fs::read_to_string(&out_xml).unwrap();
        assert!(xml_content.contains("pub fn add(a: i32, b: i32) -> i32 { a + b }"));
        assert!(!xml_content.contains("⋮----"));

        let md_str = pack_files(
            &temp_dir,
            &[&sub],
            &files,
            false,
            PackStyle::parse("markdown"),
        );
        std::fs::write(&out_md, &md_str).unwrap();
        let md_content = std::fs::read_to_string(&out_md).unwrap();
        assert!(md_content.contains("pub fn add(a: i32, b: i32) -> i32 { a + b }"));
        assert!(!md_content.contains("⋮----"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pack_builtin_non_rust_files() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir =
            std::env::temp_dir().join(format!("test_pack_poly_{}_{}", std::process::id(), nanos));
        let _ = std::fs::create_dir_all(&temp_dir);

        let py_code = "def calculate(a, b):\n    # Indented block\n    result = a + b    \n\n\n    return result\n";
        let py_file = temp_dir.join("main.py");
        std::fs::write(&py_file, py_code).unwrap();

        let sub = SubsystemInfo {
            id: "scripts".to_string(),
            name: "Scripts".to_string(),
            summary: "Python script module".to_string(),
            l1_path: "subsystems/scripts.md".to_string(),
            tags: vec!["scripts".to_string()],
            entrypoints: vec!["main.py".to_string()],
            key_files: vec!["main.py".to_string()],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 15,
            parent: None,
        };

        let files = vec!["main.py".to_string()];
        let out_nc = temp_dir.join("pack_nc.xml");
        let out_comp = temp_dir.join("pack_comp.xml");

        // With no-compress: content is 100% identical including extra blank lines and trailing whitespace
        let nc_str = pack_files(&temp_dir, &[&sub], &files, false, PackStyle::parse("xml"));
        std::fs::write(&out_nc, &nc_str).unwrap();
        assert!(nc_str.contains("    result = a + b    \n\n\n    return result"));

        // With compress: trailing whitespace and excessive empty lines collapsed, but python indentation strictly kept
        let comp_str = pack_files(&temp_dir, &[&sub], &files, true, PackStyle::parse("xml"));
        std::fs::write(&out_comp, &comp_str).unwrap();
        assert!(comp_str.contains("    result = a + b\n\n    return result"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[cfg(test)]
mod pack_subsystem_tests {
    use super::*;
    use crate::storage::SubsystemFile;
    use std::fs;

    #[test]
    fn secrets_and_databases_are_never_packed() {
        for p in [
            ".env",
            "cfg/.env.local",
            "mise.local.toml",
            "a\\b\\id.KEY",
            "x/api.token",
            "data/app.sqlite",
            "d.db",
        ] {
            assert!(is_secret_path(p), "{p}");
        }
        for p in ["src/lib.rs", "environment.rs", "keys.rs", "README.md"] {
            assert!(!is_secret_path(p), "{p}");
        }
    }

    #[test]
    fn pack_subsystem_falls_back_to_l1_card_without_files() {
        let root = std::env::temp_dir().join(format!(
            "ci_pack_card_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let kd = root.join("kb");
        fs::create_dir_all(kd.join("subsystems")).unwrap();
        fs::write(kd.join("subsystems/wm.md"), "# Window manager card\n").unwrap();
        let index = L0Index {
            subsystems: vec![SubsystemInfo {
                id: "wm".to_string(),
                name: "WM".to_string(),
                summary: "s".to_string(),
                l1_path: "subsystems/wm.md".to_string(),
                tags: vec![],
                entrypoints: vec![],
                key_files: vec![],
                files: vec![],
                key_symbols: vec![],
                test_command: None,
                tokens: 0,
                parent: None,
            }],
            ..L0Index::default()
        };
        let packed = pack_subsystem(&root, &kd, &index, "wm", true, PackStyle::Xml).unwrap();
        assert!(
            packed.content.contains("Window manager card"),
            "{}",
            packed.content
        );
        assert_eq!(packed.files, vec!["kb/subsystems/wm.md".to_string()]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pack_subsystem_ignores_paths_outside_root() {
        let base = std::env::temp_dir().join(format!(
            "ci_pack_escape_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = base.join("repo");
        let kd = root.join("kb");
        fs::create_dir_all(kd.join("subsystems")).unwrap();
        let secret = base.join("secret.txt");
        fs::write(&secret, "TOP-SECRET").unwrap();
        let sub = |id: &str, files: Vec<SubsystemFile>, key_files: Vec<String>, l1: String| {
            SubsystemInfo {
                id: id.to_string(),
                name: id.to_string(),
                summary: "s".to_string(),
                l1_path: l1,
                tags: vec![],
                entrypoints: vec![],
                key_files,
                files,
                key_symbols: vec![],
                test_command: None,
                tokens: 0,
                parent: None,
            }
        };
        let index = L0Index {
            subsystems: vec![
                sub(
                    "a",
                    vec![SubsystemFile {
                        path: "../secret.txt".to_string(),
                        is_entrypoint: false,
                        lines: 1,
                    }],
                    vec![secret.to_string_lossy().into_owned()],
                    "subsystems/a.md".to_string(),
                ),
                sub("b", vec![], vec![], secret.to_string_lossy().into_owned()),
                sub("c", vec![], vec![], "../../secret.txt".to_string()),
            ],
            ..L0Index::default()
        };
        for id in ["a", "b", "c", "all"] {
            match pack_subsystem(&root, &kd, &index, id, false, PackStyle::Xml) {
                Ok(p) => assert!(!p.content.contains("TOP-SECRET"), "{id}: {}", p.content),
                Err(e) => assert!(!format!("{e:#}").contains("TOP-SECRET")),
            }
        }
        let _ = fs::remove_dir_all(&base);
    }
}
