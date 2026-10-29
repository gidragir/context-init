use super::LanguageAdapter;
use crate::analysis::detector::extract_rust_symbols;
use crate::analysis::pack::{compress_code, compress_rust_ast};

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
}
