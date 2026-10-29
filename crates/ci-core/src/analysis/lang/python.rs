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
}
