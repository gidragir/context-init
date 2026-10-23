//! Built-in AST context packer (shared by CLI `pack` and MCP `context_pack`).

use std::path::Path;

use crate::storage::SubsystemInfo;

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

/// Compresses source text: Rust via AST (bodies stripped), others via whitespace normalization.
pub fn compress_source(content: &str, ext: &str) -> String {
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
