use super::text::{
    apply_options, brace_spans, go_private, mask_code, span_bodies, DetectFn, Flavor, LineRules,
    SpanRules,
};
use super::{LanguageAdapter, SymbolSpan, ELIDED_BODY};
use crate::analysis::detector::extract_multiline_symbols;
use crate::analysis::pack::CompressOptions;

/// Lexical brace-matching adapter for TypeScript, JavaScript, Kotlin, Java, Go, C#, etc.
pub struct BraceLangAdapter;

const BRACE_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "astro", "vue", "svelte", "java", "cs",
    "go",
];

impl LanguageAdapter for BraceLangAdapter {
    fn language_id(&self) -> &'static str {
        "brace"
    }

    fn extensions(&self) -> &'static [&'static str] {
        BRACE_EXTENSIONS
    }

    fn extract_symbols(&self, source: &str) -> Vec<String> {
        // No extension here: try the TS/JS rules, then Go, then the language-neutral rules.
        for ext in ["ts", "go", ""] {
            let symbols = extract_multiline_symbols(source, ext, false);
            if !symbols.is_empty() {
                return symbols;
            }
        }
        Vec::new()
    }

    fn compress_with(&self, source: &str, ext: &str, opts: &CompressOptions) -> String {
        let base = self.compress_source(source);
        if *opts == CompressOptions::legacy() {
            return base;
        }
        let rules = if ext.eq_ignore_ascii_case("go") {
            LineRules::go()
        } else {
            LineRules::c_like()
        };
        apply_options(&base, opts, &rules)
    }

    fn extract_symbol_spans(&self, source: &str, ext: &str) -> Vec<SymbolSpan> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        let masked = mask_code(source, Flavor::brace(&ext));
        let detect: DetectFn<'_> = match ext.as_str() {
            "go" => &go_decl,
            "java" | "cs" => &jvm_decl,
            _ => &ts_decl,
        };
        let attrs: &[&str] = if ext == "cs" { &["["] } else { &["@"] };
        brace_spans(
            source,
            &masked,
            &SpanRules {
                detect,
                expr_bodies: false,
                bodyless: true,
                attr_prefixes: attrs,
            },
        )
    }

    fn compress_source(&self, source: &str) -> String {
        let mut out = String::with_capacity(source.len() / 3);
        let mut depth: usize = 0;
        let mut eliding = false;
        let mut elide_target_depth = 0;
        let mut in_block_comment = false;
        let mut in_template = false;
        let mut interface_depth: Option<usize> = None;

        for line in source.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                if !eliding {
                    out.push('\n');
                }
                continue;
            }

            if !eliding
                && (trimmed.contains("interface ")
                    || trimmed.starts_with("type ")
                    || trimmed.contains(" type "))
                && interface_depth.is_none()
            {
                interface_depth = Some(depth);
            }

            let mut prev_char = '\0';
            let mut in_line_comment = false;
            let mut in_str: Option<char> = None;
            let chars = trimmed.chars();

            let mut line_should_elide = false;
            let mut closed_on_line = false;
            let mut body_start_indent = "";

            for c in chars {
                if in_line_comment {
                    break;
                }
                if in_block_comment {
                    if prev_char == '*' && c == '/' {
                        in_block_comment = false;
                    }
                    prev_char = c;
                    continue;
                }
                if let Some(q) = in_str {
                    if c == q && prev_char != '\\' {
                        in_str = None;
                    }
                    prev_char = c;
                    continue;
                }
                if in_template {
                    if c == '`' && prev_char != '\\' {
                        in_template = false;
                    }
                    prev_char = c;
                    continue;
                }

                if prev_char == '/' && c == '/' {
                    in_line_comment = true;
                    prev_char = c;
                    continue;
                }
                if prev_char == '/' && c == '*' {
                    in_block_comment = true;
                    prev_char = c;
                    continue;
                }

                if c == '"' || c == '\'' {
                    in_str = Some(c);
                } else if c == '`' {
                    in_template = true;
                } else if c == '{' {
                    depth += 1;
                    if !eliding && interface_depth.is_none() {
                        // Function/method/constructor body at depth 1 (top-level) or depth 2 (class method)
                        if depth == 1 || depth == 2 {
                            eliding = true;
                            elide_target_depth = depth - 1;
                            line_should_elide = true;
                            closed_on_line = false;
                            let indent_len = line.len() - line.trim_start().len();
                            body_start_indent = &line[..indent_len];
                        }
                    }
                } else if c == '}' {
                    depth = depth.saturating_sub(1);
                    if let Some(id) = interface_depth {
                        if depth <= id {
                            interface_depth = None;
                        }
                    }
                    if eliding && depth == elide_target_depth {
                        eliding = false;
                        closed_on_line = line_should_elide;
                    }
                }
                prev_char = c;
            }

            if line_should_elide && closed_on_line {
                // `func f() {}` / `fn() { x }`: nothing hidden on another line.
                out.push_str(line);
                out.push('\n');
            } else if line_should_elide {
                out.push_str(line);
                out.push('\n');
                out.push_str(body_start_indent);
                out.push_str("    ");
                out.push_str(ELIDED_BODY);
                out.push('\n');
            } else if !eliding {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }

    fn method_bodies(&self, source: &str) -> Vec<(String, String)> {
        let ext = guess_ext(source);
        span_bodies(
            source,
            &self.extract_symbol_spans(source, ext),
            &["func", "function", "method"],
        )
    }
}

/// Best-effort language of a brace source when no extension is known.
fn guess_ext(source: &str) -> &'static str {
    let has_line = |p: &str| source.lines().any(|l| l.starts_with(p));
    if has_line("package ") && has_line("func ") {
        "go"
    } else if has_line("using System") || has_line("namespace ") {
        "cs"
    } else if has_line("package ") || has_line("import java") {
        "java"
    } else {
        "ts"
    }
}

const CONTROL_WORDS: &[&str] = &[
    "if",
    "for",
    "while",
    "switch",
    "catch",
    "return",
    "new",
    "function",
    "super",
    "this",
    "else",
    "do",
    "try",
    "throw",
    "await",
    "yield",
    "typeof",
    "delete",
    "case",
    "lock",
    "using",
    "foreach",
    "synchronized",
];

fn ident_prefix(s: &str) -> &str {
    let end = s
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .unwrap_or(s.len());
    &s[..end]
}

fn go_decl(t: &str, parent: Option<&'static str>) -> Option<(&'static str, bool)> {
    if parent.is_some() {
        return None;
    }
    let kind = if t.starts_with("func ") {
        "func"
    } else if t.starts_with("type ") {
        "type"
    } else {
        return None;
    };
    Some((kind, !go_private(t)))
}

fn ts_decl(t: &str, parent: Option<&'static str>) -> Option<(&'static str, bool)> {
    match parent {
        None => {
            let mut s = t;
            let mut exported = false;
            loop {
                if let Some(r) = s.strip_prefix("export ") {
                    exported = true;
                    s = r.trim_start();
                } else if let Some(r) = ["default ", "declare ", "abstract ", "async "]
                    .iter()
                    .find_map(|p| s.strip_prefix(p))
                {
                    s = r.trim_start();
                } else {
                    break;
                }
            }
            let kind = if s.starts_with("function") {
                "function"
            } else if s.starts_with("class ") {
                "class"
            } else if s.starts_with("interface ") {
                "interface"
            } else if s.starts_with("enum ") {
                "enum"
            } else if s.starts_with("type ") {
                "type"
            } else if (s.starts_with("const ") || s.starts_with("let "))
                && (s.contains("=>") || s.contains("function"))
            {
                "function"
            } else {
                return None;
            };
            Some((kind, exported))
        }
        Some("class") => {
            let mut s = t;
            let mut private = false;
            loop {
                let word = ident_prefix(s);
                let is_mod = matches!(
                    word,
                    "public"
                        | "private"
                        | "protected"
                        | "static"
                        | "async"
                        | "readonly"
                        | "override"
                        | "abstract"
                        | "get"
                        | "set"
                        | "declare"
                ) && s[word.len()..].starts_with(' ');
                if !is_mod {
                    break;
                }
                private |= word == "private";
                s = s[word.len()..].trim_start();
            }
            if let Some(r) = s.strip_prefix('#') {
                private = true;
                s = r;
            }
            let s = s.strip_prefix('*').unwrap_or(s);
            let name = ident_prefix(s);
            if name.is_empty() || CONTROL_WORDS.contains(&name) {
                return None;
            }
            let rest = s[name.len()..].trim_start_matches(['?', '!']).trim_start();
            if rest.starts_with('(') || rest.starts_with('<') {
                Some(("method", !private))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn jvm_decl(t: &str, parent: Option<&'static str>) -> Option<(&'static str, bool)> {
    let class_like = |p: &str| matches!(p, "class" | "interface" | "enum" | "record" | "struct");
    if parent.is_some_and(|p| !class_like(p)) {
        return None;
    }
    let mut s = t;
    while s.starts_with('@') || s.starts_with('[') {
        let close = if s.starts_with('[') { ']' } else { ' ' };
        s = s.split_once(close).map_or("", |(_, r)| r).trim_start();
    }
    const MODS: &[&str] = &[
        "public",
        "private",
        "protected",
        "internal",
        "static",
        "final",
        "abstract",
        "sealed",
        "override",
        "virtual",
        "async",
        "synchronized",
        "partial",
        "readonly",
        "extern",
        "unsafe",
        "new",
        "default",
        "native",
        "strictfp",
        "non-sealed",
    ];
    let head = s.split('(').next().unwrap_or(s);
    let words: Vec<&str> = head.split_whitespace().collect();
    let mut i = 0;
    let mut public = false;
    while i < words.len() && MODS.contains(&words[i]) {
        public |= words[i] == "public";
        i += 1;
    }
    let first = *words.get(i)?;
    let kind = match first {
        "class" => "class",
        "interface" => "interface",
        "enum" => "enum",
        "record" => "record",
        "struct" => "struct",
        _ => {
            let is_method = parent.is_some()
                && s.contains('(')
                && !CONTROL_WORDS.contains(&first)
                && !head.contains('=')
                && !head.contains('.')
                && words.len() > i;
            if !is_method {
                return None;
            }
            "method"
        }
    };
    Some((kind, public))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::pack::CompressOptions;

    const GO: &str = "package main\n\nimport (\n\t\"fmt\"\n\t\"net/http\"\n)\n\n// Serve starts the server.\nfunc Serve(addr string, h http.Handler) error {\n\tfmt.Println(\"serving {\")\n\treturn http.ListenAndServe(addr, h)\n}\n\nfunc helper() {}\n";

    const TS: &str = "import { x } from './x';\nexport interface User {\n  id: string;\n}\n\nexport function load(u: User): string {\n  return `${u.id}}`;\n}\n";

    #[test]
    fn brace_extract_symbols_and_compress_respect_go_and_ts_extensions() {
        let a = BraceLangAdapter;
        let go_syms = a.extract_symbols(GO);
        assert!(
            go_syms.iter().any(|s| s.starts_with("func Serve(")),
            "{go_syms:?}"
        );
        let ts_syms = a.extract_symbols(TS);
        assert!(
            ts_syms.iter().any(|s| s.contains("export interface User")),
            "{ts_syms:?}"
        );

        let spans = a.extract_symbol_spans(GO, "go");
        let serve = spans
            .iter()
            .find(|s| s.signature.starts_with("func Serve("))
            .unwrap_or_else(|| panic!("{spans:?}"));
        assert_eq!((serve.line_start, serve.line_end), (9, 12), "{spans:?}");
        assert!(serve.is_public);
        let helper = spans
            .iter()
            .find(|s| s.signature.starts_with("func helper("))
            .unwrap_or_else(|| panic!("{spans:?}"));
        assert_eq!((helper.line_start, helper.line_end), (14, 14));
        assert!(!helper.is_public);

        let ts_spans = a.extract_symbol_spans(TS, "ts");
        let load = ts_spans
            .iter()
            .find(|s| s.signature.contains("function load("))
            .unwrap_or_else(|| panic!("{ts_spans:?}"));
        assert_eq!((load.line_start, load.line_end), (6, 8), "{ts_spans:?}");
        assert!(load.is_public);

        let go_c = a.compress_with(GO, "go", &CompressOptions::compact());
        assert!(!go_c.contains("net/http"), "{go_c}");
        assert!(go_c.contains("imports omitted]"), "{go_c}");
        assert!(!go_c.contains("Serve starts"), "{go_c}");
        assert!(
            go_c.contains("func Serve(addr string, h http.Handler) error { ⋮ }"),
            "{go_c}"
        );
        assert!(!go_c.contains("ListenAndServe"), "{go_c}");

        let ts_c = a.compress_with(TS, "ts", &CompressOptions::compact());
        assert!(!ts_c.contains("import {"), "{ts_c}");
        assert!(ts_c.contains("id: string;"), "{ts_c}");
        assert!(
            ts_c.contains("export function load(u: User): string { ⋮ }"),
            "{ts_c}"
        );
        assert_eq!(
            a.compress_with(GO, "go", &CompressOptions::legacy()),
            a.compress_source(GO)
        );

        let bodies = a.method_bodies(GO);
        assert!(
            bodies
                .iter()
                .any(|(n, b)| n == "Serve" && b.contains("ListenAndServe")),
            "{bodies:?}"
        );
    }
}
