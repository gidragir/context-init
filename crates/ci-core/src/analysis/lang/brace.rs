use super::{LanguageAdapter, ELIDED_BODY};
use crate::analysis::detector::extract_multiline_symbols;

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
        extract_multiline_symbols(source, "ts", false)
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
                    }
                }
                prev_char = c;
            }

            if line_should_elide {
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

    fn method_bodies(&self, _source: &str) -> Vec<(String, String)> {
        Vec::new()
    }
}
