use super::text::{apply_options, brace_spans, mask_code, Flavor, LineRules, SpanRules};
use super::{LanguageAdapter, SymbolSpan};
use crate::analysis::detector::extract_rust_symbols;
use crate::analysis::pack::{
    compress_code, compress_rust_ast, compress_rust_ast_with, CompressOptions,
};

/// Rust via `syn`: public items as symbols, bodies stripped from the compressed view.
pub struct RustAdapter;

impl LanguageAdapter for RustAdapter {
    fn language_id(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn extract_symbols(&self, source: &str) -> Vec<String> {
        extract_rust_symbols(source, false)
    }

    fn compress_source(&self, source: &str) -> String {
        compress_rust_ast(source).unwrap_or_else(|| compress_code(source))
    }

    fn compress_with(&self, source: &str, _ext: &str, opts: &CompressOptions) -> String {
        compress_rust_ast_with(source, opts).unwrap_or_else(|| {
            let base = compress_code(source);
            if *opts == CompressOptions::legacy() {
                base
            } else {
                apply_options(&base, opts, &LineRules::c_like())
            }
        })
    }

    fn extract_symbol_spans(&self, source: &str, _ext: &str) -> Vec<SymbolSpan> {
        extract_rust_symbol_spans(source)
    }

    fn method_bodies(&self, source: &str) -> Vec<(String, String)> {
        fn walk(items: &[syn::Item], out: &mut Vec<(String, String)>) {
            for item in items {
                match item {
                    syn::Item::Fn(f) => {
                        out.push((f.sig.ident.to_string(), quote::quote!(#f).to_string()))
                    }
                    syn::Item::Impl(imp) => {
                        for it in &imp.items {
                            if let syn::ImplItem::Fn(f) = it {
                                out.push((f.sig.ident.to_string(), quote::quote!(#f).to_string()));
                            }
                        }
                    }
                    syn::Item::Mod(m) => {
                        if let Some((_, inner)) = &m.content {
                            walk(inner, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        if let Ok(file) = syn::parse_file(source) {
            walk(&file.items, &mut out);
        }
        out
    }
}

/// Skips a balanced `open ... close` group at the start of `s` (which must start with `open`).
fn skip_group(s: &str, open: char, close: char) -> &str {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return s[i + 1..].trim_start();
            }
        }
    }
    ""
}

/// Rust item kind and visibility of a masked, trimmed line. Items are recognized at the top
/// level and directly inside `impl` / `trait` / `mod` blocks (never inside function bodies).
fn rust_decl(t: &str, parent: Option<&'static str>) -> Option<(&'static str, bool)> {
    if parent.is_some_and(|p| !matches!(p, "impl" | "trait" | "mod")) {
        return None;
    }
    let mut s = t;
    while s.starts_with("#[") || s.starts_with("#![") {
        s = skip_group(&s[s.find('[')?..], '[', ']');
    }
    let mut public = false;
    if let Some(r) = s.strip_prefix("pub") {
        if r.starts_with(' ') {
            public = true;
            s = r.trim_start();
        } else if r.starts_with('(') {
            public = true;
            s = skip_group(r, '(', ')');
        }
    }
    loop {
        if let Some(r) = s.strip_prefix("const ") {
            let r = r.trim_start();
            if ["fn ", "unsafe ", "async ", "extern "]
                .iter()
                .any(|p| r.starts_with(p))
            {
                s = r;
                continue;
            }
            break;
        }
        if let Some(r) = ["default ", "async ", "unsafe ", "extern "]
            .iter()
            .find_map(|p| s.strip_prefix(p))
        {
            s = r.trim_start();
            continue;
        }
        break;
    }
    let word: String = s
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '!')
        .collect();
    let after = s[word.len()..].chars().next();
    let kind = match word.as_str() {
        "fn" => "fn",
        "struct" => "struct",
        "enum" => "enum",
        "trait" => "trait",
        "mod" => "mod",
        "type" => "type",
        "union" if after == Some(' ') => "union",
        "const" => "const",
        "static" => "static",
        "macro_rules!" => "macro",
        "impl" if matches!(after, Some(' ') | Some('<')) => "impl",
        _ => return None,
    };
    if kind != "macro" && after != Some(' ') && kind != "impl" {
        return None;
    }
    let public = public || matches!(kind, "impl") || parent == Some("trait");
    Some((kind, public))
}

/// One pass over the masked source: exact `[line_start-line_end]` of every item, found by
/// lexical `{}` / `;` boundary scanning (no `syn` span locations needed).
pub(crate) fn extract_rust_symbol_spans(source: &str) -> Vec<SymbolSpan> {
    let masked = mask_code(source, Flavor::rust());
    brace_spans(
        source,
        &masked,
        &SpanRules {
            detect: &rust_decl,
            expr_bodies: false,
            bodyless: false,
            attr_prefixes: &["#["],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_spans_cover_items_impl_methods_and_skip_locals() {
        let src = "use std::fmt;\n\n#[derive(Debug)]\npub struct S {\n    a: u8,\n}\n\nimpl S {\n    pub fn new() -> Self {\n        fn local() {}\n        let s = \"}\";\n        S { a: b'}' }\n    }\n\n    fn hidden<'a>(&self, x: &'a str)\n    where\n        Self: Sized,\n    {\n    }\n}\n\npub(crate) const N: usize = 3;\n";
        let spans = extract_rust_symbol_spans(src);
        let got: Vec<(&str, usize, usize, bool)> = spans
            .iter()
            .map(|s| (s.kind, s.line_start, s.line_end, s.is_public))
            .collect();
        assert_eq!(
            got,
            vec![
                ("struct", 3, 6, true),
                ("impl", 8, 20, true),
                ("fn", 9, 13, true),
                ("fn", 15, 19, false),
                ("const", 22, 22, true),
            ],
            "{spans:?}"
        );
        assert_eq!(
            spans[3].signature,
            "fn hidden<'a>(&self, x: &'a str) where Self: Sized,"
        );
    }
}
