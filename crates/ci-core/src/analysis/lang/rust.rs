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
