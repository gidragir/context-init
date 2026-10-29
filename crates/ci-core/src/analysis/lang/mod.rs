//! Language adapters (D7): symbol extraction and signature-level compression per language.

mod bsl;
pub mod mdo;
mod python;
mod rust;

use std::path::Path;

pub use bsl::BslAdapter;
pub use python::PythonAdapter;
pub use rust::RustAdapter;

/// Marker that replaces an elided body in compressed sources.
pub const ELIDED_BODY: &str = "⋮----";

/// Per-language knowledge: which files it owns, their public symbols and a compressed view.
pub trait LanguageAdapter: Send + Sync {
    fn language_id(&self) -> &'static str;
    /// Lower-case file extensions without the dot.
    fn extensions(&self) -> &'static [&'static str];
    /// Exported / public signatures of `source`.
    fn extract_symbols(&self, source: &str) -> Vec<String>;
    /// `source` with bodies elided, keeping signatures, directives and comments.
    fn compress_source(&self, source: &str) -> String;
}

static RUST: RustAdapter = RustAdapter;
static PYTHON: PythonAdapter = PythonAdapter;
static BSL: BslAdapter = BslAdapter;

fn all() -> [&'static dyn LanguageAdapter; 3] {
    [&RUST, &PYTHON, &BSL]
}

/// Adapter for a file extension (with or without the dot, any case).
pub fn adapter_for_ext(ext: &str) -> Option<&'static dyn LanguageAdapter> {
    let ext = ext.trim_start_matches('.').to_lowercase();
    all()
        .into_iter()
        .find(|a| a.extensions().contains(&ext.as_str()))
}

/// Adapter for the file at `path`, chosen by its extension.
pub fn adapter_for_path(path: &Path) -> Option<&'static dyn LanguageAdapter> {
    adapter_for_ext(path.extension()?.to_str()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::detector::count_tokens;

    fn bsl_module() -> String {
        let mut src = String::from(
            "// Module of data handling\r\n#Область ПрограммныйИнтерфейс\r\n&НаСервере\r\nПроцедура ОбработатьДанные(Парам1, Парам2 = Неопределено) Экспорт\r\n",
        );
        for i in 0..40 {
            src.push_str(&format!(
                "    Результат{i} = Парам1 + {i}; // шаг вычисления номер {i}\r\n    Если Результат{i} > 100 Тогда\r\n        ЗаписатьВЖурнал(Результат{i});\r\n    КонецЕсли;\r\n"
            ));
        }
        src.push_str("КонецПроцедуры\r\n\r\nФункция Внутренняя()\r\n");
        for i in 0..20 {
            src.push_str(&format!("    Значение{i} = {i} * 2;\r\n"));
        }
        src.push_str("    Возврат 1;\r\nКонецФункции\r\n#КонецОбласти\r\n\r\n");
        src.push_str("Procedure RunTask(Val X) Export\r\n");
        for i in 0..30 {
            src.push_str(&format!(
                "    Total{i} = X + {i};\r\n    Log(Total{i});\r\n"
            ));
        }
        src.push_str("EndProcedure\r\n");
        src
    }

    #[test]
    fn bsl_adapter_extracts_exported_procedures_and_compresses_bodies() {
        let src = bsl_module();
        let adapter = adapter_for_ext("bsl").expect("bsl adapter");
        assert_eq!(adapter.language_id(), "bsl");

        let symbols = adapter.extract_symbols(&src);
        assert_eq!(
            symbols,
            vec![
                "Процедура ОбработатьДанные(Парам1, Парам2 = Неопределено) Экспорт".to_string(),
                "Procedure RunTask(Val X) Export".to_string(),
            ],
            "only exported methods, CRLF tolerated"
        );

        let compressed = adapter.compress_source(&src);
        assert!(compressed.contains("&НаСервере"), "{compressed}");
        assert!(
            compressed.contains("// Module of data handling"),
            "{compressed}"
        );
        assert!(compressed.contains("Функция Внутренняя()"), "{compressed}");
        assert!(compressed.contains("КонецПроцедуры") && compressed.contains("EndProcedure"));
        assert!(compressed.contains(ELIDED_BODY), "{compressed}");
        assert!(!compressed.contains("ЗаписатьВЖурнал"), "{compressed}");
        let (before, after) = (count_tokens(&src), count_tokens(&compressed));
        let saved = (before - after) as f64 * 100.0 / before as f64;
        assert!(
            saved >= 60.0,
            "saved only {saved:.1}% ({before} -> {after})"
        );
    }

    #[test]
    fn os_files_use_the_bsl_adapter_and_multiline_signatures_are_joined() {
        let adapter = adapter_for_path(Path::new("scripts/run.os")).unwrap();
        assert_eq!(adapter.language_id(), "bsl");
        let symbols = adapter.extract_symbols(
            "Функция Сумма(А,\n    Б) Экспорт\n    Возврат А + Б;\nКонецФункции\n",
        );
        assert_eq!(symbols, vec!["Функция Сумма(А, Б) Экспорт".to_string()]);
    }

    #[test]
    fn rust_and_python_adapters_preserve_existing_pack_compression() {
        use crate::analysis::pack::{compress_code, compress_rust_ast, compress_source};
        for (name, id) in [
            ("a.rs", "rust"),
            ("a.py", "python"),
            ("a.bsl", "bsl"),
            ("a.os", "bsl"),
        ] {
            assert_eq!(
                adapter_for_path(Path::new(name)).unwrap().language_id(),
                id,
                "{name}"
            );
        }
        assert!(adapter_for_path(Path::new("a.txt")).is_none());

        let rs = "pub fn f(x: u32) -> u32 {\n    let y = x + 1;\n    y\n}\n";
        let expected = compress_rust_ast(rs).unwrap();
        assert_eq!(adapter_for_ext("rs").unwrap().compress_source(rs), expected);
        assert_eq!(compress_source(rs, "rs"), expected);
        let py = "def f(x):\n\n\n    return x   \n";
        assert_eq!(
            adapter_for_ext("py").unwrap().compress_source(py),
            compress_code(py)
        );

        // The packer dispatches through the adapters, so BSL bodies are elided in a pack.
        let root = std::env::temp_dir().join(format!("lang_pack_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("m.bsl"), bsl_module()).unwrap();
        let packed = crate::analysis::pack::pack_files(
            &root,
            &[],
            &["m.bsl".to_string()],
            true,
            crate::analysis::pack::PackStyle::Markdown,
        );
        assert!(
            packed.contains(ELIDED_BODY) && !packed.contains("ЗаписатьВЖурнал"),
            "{packed}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
