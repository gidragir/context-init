use super::LanguageAdapter;
use crate::analysis::detector::extract_multiline_symbols;
use crate::analysis::pack::compress_code;

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
        compress_code(source)
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
