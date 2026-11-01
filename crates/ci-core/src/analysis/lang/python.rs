use super::LanguageAdapter;
use crate::analysis::detector::extract_multiline_symbols;

/// Python: top-level `def`/`class` signatures; compression normalizes whitespace.
pub struct PythonAdapter;

impl LanguageAdapter for PythonAdapter {
    fn language_id(&self) -> &'static str {
        "python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn extract_symbols(&self, source: &str) -> Vec<String> {
        extract_multiline_symbols(source, "py", false)
    }

    fn compress_source(&self, source: &str) -> String {
        let mut out = String::with_capacity(source.len() / 2);
        let mut eliding_indent: Option<usize> = None;
        let mut awaiting_docstring = false;
        let mut in_docstring: Option<&'static str> = None;

        for line in source.lines() {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();

            if trimmed.is_empty() {
                if eliding_indent.is_none() && in_docstring.is_none() {
                    out.push('\n');
                }
                continue;
            }

            if let Some(quote) = in_docstring {
                out.push_str(line);
                out.push('\n');
                if trimmed.contains(quote) {
                    in_docstring = None;
                    if let Some(def_ind) = eliding_indent {
                        let body_indent = " ".repeat(def_ind + 4);
                        out.push_str(&body_indent);
                        out.push_str(super::ELIDED_BODY);
                        out.push('\n');
                    }
                }
                continue;
            }

            if let Some(def_ind) = eliding_indent {
                if indent > def_ind {
                    if awaiting_docstring {
                        awaiting_docstring = false;
                        if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
                            let quote = if trimmed.starts_with("\"\"\"") {
                                "\"\"\""
                            } else {
                                "'''"
                            };
                            out.push_str(line);
                            out.push('\n');
                            if trimmed.len() > 3 && trimmed[3..].contains(quote) {
                                let body_indent = " ".repeat(def_ind + 4);
                                out.push_str(&body_indent);
                                out.push_str(super::ELIDED_BODY);
                                out.push('\n');
                            } else {
                                in_docstring = Some(quote);
                            }
                            continue;
                        } else {
                            let body_indent = " ".repeat(def_ind + 4);
                            out.push_str(&body_indent);
                            out.push_str(super::ELIDED_BODY);
                            out.push('\n');
                            continue;
                        }
                    }
                    continue;
                } else {
                    eliding_indent = None;
                    awaiting_docstring = false;
                }
            }

            out.push_str(line);
            out.push('\n');

            let is_def = trimmed.starts_with("def ") || trimmed.starts_with("async def ");
            if is_def {
                eliding_indent = Some(indent);
                awaiting_docstring = true;
            }
        }
        out
    }

    fn method_bodies(&self, source: &str) -> Vec<(String, String)> {
        let mut out: Vec<(String, String, usize)> = Vec::new(); // name, text, indent
        let mut current: Option<usize> = None;
        for line in source.lines() {
            let indent = line.len() - line.trim_start().len();
            let t = line.trim_start();
            let def = t
                .strip_prefix("async def ")
                .or_else(|| t.strip_prefix("def "));
            if let Some(rest) = def {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                out.push((name, format!("{line}\n"), indent));
                current = Some(out.len() - 1);
            } else if let Some(idx) = current {
                if !t.is_empty() && indent <= out[idx].2 {
                    current = None;
                } else {
                    out[idx].1.push_str(line);
                    out[idx].1.push('\n');
                }
            }
        }
        out.into_iter().map(|(n, t, _)| (n, t)).collect()
    }
}
