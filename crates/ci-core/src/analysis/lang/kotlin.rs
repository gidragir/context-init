use super::text::{
    apply_options, brace_spans, mask_code, span_bodies, Flavor, LineRules, SpanRules,
};
use super::{LanguageAdapter, SymbolSpan, ELIDED_BODY};
use crate::analysis::detector::{clean_multiline_signature, sanitize_symbol};
use crate::analysis::pack::CompressOptions;

/// Kotlin language adapter: accurate symbol extraction and AST-level body compression.
pub struct KotlinAdapter;

const KOTLIN_EXTENSIONS: &[&str] = &["kt", "kts"];

impl LanguageAdapter for KotlinAdapter {
    fn language_id(&self) -> &'static str {
        "kotlin"
    }

    fn extensions(&self) -> &'static [&'static str] {
        KOTLIN_EXTENSIONS
    }

    fn extract_symbols(&self, source: &str) -> Vec<String> {
        let lines: Vec<&str> = source.lines().collect();
        let masked = mask_code(source, Flavor::kotlin());
        let mut idx = 0;
        let mut symbols = Vec::new();

        while idx < lines.len() {
            let line = lines[idx];
            // Lines entirely inside raw strings / comments are blank once masked.
            let in_code = masked.get(idx).is_some_and(|m| !m.trim().is_empty());
            if in_code && is_symbol_start(line) {
                let is_fun = is_fun_start(line);
                let mut accumulated = String::new();
                let mut parens: i32 = 0;
                let mut brackets: i32 = 0;
                let mut line_count = 0;

                while idx < lines.len() && line_count < 12 {
                    let current_line = lines[idx].trim();
                    if !accumulated.is_empty() {
                        accumulated.push(' ');
                    }
                    accumulated.push_str(current_line);

                    update_bracket_depths(current_line, &mut parens, &mut brackets);
                    line_count += 1;
                    idx += 1;

                    if parens == 0 && brackets == 0 {
                        // Functions must have parentheses before terminating
                        if is_fun && !accumulated.contains('(') {
                            continue;
                        }

                        // Rule 1: contains unquoted '{'
                        if has_unquoted_brace(current_line) {
                            break;
                        }

                        // Rule 2: terminates if line ends with an identifier, type, or closing paren/bracket
                        // and does not end with continuation operators (=, ,, +, ->)
                        let clean_line = strip_line_comment(current_line).trim_end();
                        if is_signature_terminated(clean_line) {
                            break;
                        }
                    }
                }

                let cleaned = clean_multiline_signature(&accumulated, "kt");
                if !cleaned.is_empty() {
                    if let Some(sanitized) = sanitize_symbol(&cleaned) {
                        if !symbols.contains(&sanitized) {
                            symbols.push(sanitized);
                        }
                    }
                }
            } else {
                idx += 1;
            }
        }

        symbols
    }

    fn compress_source(&self, source: &str) -> String {
        let masked = mask_code(source, Flavor::kotlin());
        let mut out = String::with_capacity(source.len() / 3);
        let mut depth: usize = 0;
        let mut eliding = false;
        let mut elide_target_depth = 0;

        for (idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                if !eliding {
                    out.push('\n');
                }
                continue;
            }

            // Strings, templates and comments are blanked, so braces inside them never count.
            let code = masked.get(idx).map_or("", |m| m.trim());
            let is_container_decl = is_container_declaration(code);
            let has_fun_decl = has_fun_declaration(code);

            let mut line_should_elide = false;
            let mut closed_on_line = false;
            let mut body_start_indent = "";

            for c in code.chars() {
                if c == '{' {
                    depth += 1;
                    if !eliding {
                        let should_elide_here = if depth >= 2 {
                            !is_container_decl
                        } else {
                            has_fun_decl && !is_container_decl
                        };

                        if should_elide_here {
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
                    if eliding && depth == elide_target_depth {
                        eliding = false;
                        closed_on_line = line_should_elide;
                    }
                }
            }

            if line_should_elide && !closed_on_line {
                out.push_str(line);
                out.push('\n');
                out.push_str(body_start_indent);
                out.push_str("    ");
                out.push_str(ELIDED_BODY);
                out.push('\n');
            } else if line_should_elide || !eliding {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }

    fn method_bodies(&self, source: &str) -> Vec<(String, String)> {
        span_bodies(source, &self.extract_symbol_spans(source, "kt"), &["fun"])
    }

    fn compress_with(&self, source: &str, _ext: &str, opts: &CompressOptions) -> String {
        let base = self.compress_source(source);
        if *opts == CompressOptions::legacy() {
            return base;
        }
        apply_options(&base, opts, &LineRules::c_like())
    }

    fn extract_symbol_spans(&self, source: &str, _ext: &str) -> Vec<SymbolSpan> {
        let masked = mask_code(source, Flavor::kotlin());
        brace_spans(
            source,
            &masked,
            &SpanRules {
                detect: &kotlin_decl,
                expr_bodies: true,
                bodyless: true,
                attr_prefixes: &["@"],
            },
        )
    }
}

/// Strips leading `@Annotation` / `@Annotation(args)` prefixes from a masked line.
fn strip_annotations(mut t: &str) -> &str {
    while let Some(rest) = t.strip_prefix('@') {
        let name_len = rest
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.' || c == ':'))
            .unwrap_or(rest.len());
        let mut tail = &rest[name_len..];
        if tail.starts_with('(') {
            let mut depth = 0i32;
            let mut cut = tail.len();
            for (i, c) in tail.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            cut = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            tail = &tail[cut..];
        }
        t = tail.trim_start();
    }
    t
}

const KOTLIN_MODIFIERS: &[&str] = &[
    "public",
    "protected",
    "internal",
    "abstract",
    "final",
    "open",
    "sealed",
    "data",
    "value",
    "enum",
    "annotation",
    "inline",
    "suspend",
    "override",
    "tailrec",
    "operator",
    "infix",
    "external",
    "expect",
    "actual",
    "companion",
    "inner",
];

/// Kotlin declaration kind and visibility of a masked, trimmed line.
fn kotlin_decl(t: &str, parent: Option<&'static str>) -> Option<(&'static str, bool)> {
    if parent == Some("fun") || t.starts_with("package ") || t.starts_with("import ") {
        return None;
    }
    let t = strip_annotations(t);
    let words: Vec<&str> = t.split_whitespace().collect();
    let mut public = true;
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if w == "private" {
            public = false;
        } else if w == "fun" && words.get(i + 1) == Some(&"interface") {
            return Some(("interface", public));
        } else if !KOTLIN_MODIFIERS.contains(&w) {
            break;
        }
        i += 1;
    }
    let kind = match *words.get(i)? {
        "class" => "class",
        "interface" => "interface",
        "object" => "object",
        "typealias" => "typealias",
        "fun" if i + 1 < words.len() => "fun",
        _ => return None,
    };
    Some((kind, public))
}

fn is_symbol_start(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("package ")
        || trimmed.starts_with("import ")
    {
        return false;
    }

    let mut words: Vec<&str> = trimmed.split_whitespace().collect();
    while let Some(w) = words.first() {
        if w.starts_with('@') {
            words.remove(0);
        } else {
            break;
        }
    }

    if words.is_empty() {
        return false;
    }

    const MODIFIERS: &[&str] = &[
        "public",
        "internal",
        "protected",
        "abstract",
        "final",
        "open",
        "sealed",
        "data",
        "value",
        "enum",
        "annotation",
        "inline",
        "suspend",
        "override",
        "tailrec",
        "operator",
        "infix",
        "external",
        "expect",
        "actual",
        "companion",
    ];

    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if w == "private" {
            return false;
        }
        if w.starts_with('@') {
            i += 1;
            continue;
        }
        if w == "fun" && i + 1 < words.len() && words[i + 1] == "interface" {
            i += 1;
            continue;
        }
        if MODIFIERS.contains(&w) {
            i += 1;
            continue;
        }
        break;
    }

    if i >= words.len() {
        return false;
    }

    let target = words[i];
    match target {
        "class" | "interface" | "object" | "typealias" => true,
        "fun" => i + 1 < words.len(),
        _ => false,
    }
}

fn is_fun_start(line: &str) -> bool {
    let trimmed = line.trim();
    let before_parens = trimmed.split('(').next().unwrap_or(trimmed);
    let words: Vec<&str> = before_parens.split_whitespace().collect();
    if words.contains(&"interface") {
        return false;
    }
    words.contains(&"fun")
}

fn is_container_declaration(trimmed: &str) -> bool {
    let before_parens = trimmed.split('(').next().unwrap_or(trimmed);
    for word in before_parens.split_whitespace() {
        if matches!(word, "class" | "interface" | "object") {
            return true;
        }
    }
    false
}

fn has_fun_declaration(trimmed: &str) -> bool {
    is_fun_start(trimmed)
}

fn is_signature_terminated(clean_line: &str) -> bool {
    if clean_line.is_empty() {
        return false;
    }

    if clean_line.ends_with('=')
        || clean_line.ends_with(',')
        || clean_line.ends_with('+')
        || clean_line.ends_with("->")
        || clean_line.ends_with(':')
        || clean_line.ends_with('&')
        || clean_line.ends_with('|')
        || clean_line.ends_with(" where")
        || clean_line == "where"
    {
        return false;
    }

    clean_line.ends_with(|c: char| {
        c.is_alphanumeric() || c == '_' || c == ')' || c == ']' || c == '>' || c == '?' || c == ';'
    })
}

fn strip_line_comment(line: &str) -> &str {
    if let Some(pos) = line.find("//") {
        &line[..pos]
    } else {
        line
    }
}

fn has_unquoted_brace(line: &str) -> bool {
    let mut in_str: Option<char> = None;
    let mut prev_char = '\0';
    for c in line.chars() {
        if let Some(q) = in_str {
            if c == q && prev_char != '\\' {
                in_str = None;
            }
            prev_char = c;
            continue;
        }
        if prev_char == '/' && c == '/' {
            break;
        }
        if c == '"' || c == '\'' || c == '`' {
            in_str = Some(c);
        } else if c == '{' {
            return true;
        }
        prev_char = c;
    }
    false
}

fn update_bracket_depths(line: &str, parens: &mut i32, brackets: &mut i32) {
    let mut in_str: Option<char> = None;
    let mut prev_char = '\0';

    for c in line.chars() {
        if let Some(q) = in_str {
            if c == q && prev_char != '\\' {
                in_str = None;
            }
            prev_char = c;
            continue;
        }

        if prev_char == '/' && c == '/' {
            break;
        }

        if c == '"' || c == '\'' || c == '`' {
            in_str = Some(c);
        } else {
            match c {
                '(' => *parens += 1,
                ')' => *parens = (*parens - 1).max(0),
                '[' => *brackets += 1,
                ']' => *brackets = (*brackets - 1).max(0),
                _ => {}
            }
        }
        prev_char = c;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kotlin_metadata() {
        let adapter = KotlinAdapter;
        assert_eq!(adapter.language_id(), "kotlin");
        assert_eq!(adapter.extensions(), &["kt", "kts"]);
    }

    #[test]
    fn test_kotlin_compress_retains_classes_and_elides_methods() {
        let adapter = KotlinAdapter;
        let source = r#"
package dev.mimir.service

import dev.mimir.core.LocalVault

class SyncService(val vault: LocalVault) {
    val status: String = "IDLE"

    fun performSync(): Boolean {
        println("starting")
        val result = true
        return result
    }

    companion object {
        fun create(): SyncService {
            return SyncService()
        }
    }
}

fun topLevelHelper() {
    println("helper")
}
"#;
        let compressed = adapter.compress_source(source);
        assert!(compressed.contains("class SyncService(val vault: LocalVault) {"));
        assert!(compressed.contains("val status: String = \"IDLE\""));
        assert!(compressed.contains("fun performSync(): Boolean {"));
        assert!(compressed.contains("companion object {"));
        assert!(compressed.contains("fun topLevelHelper() {"));
        assert!(compressed.contains(ELIDED_BODY));
        assert!(!compressed.contains("println(\"starting\")"));
        assert!(!compressed.contains("println(\"helper\")"));
    }

    #[test]
    fn test_kotlin_extract_symbols_comprehensive() {
        let adapter = KotlinAdapter;
        let source = r#"
package dev.mimir.test

object SingletonConfig

enum class EngineState { IDLE, RUNNING }

value class FileId(val id: String)

fun interface EventListener {
    fun onEvent(event: String)
}

typealias ActionHandler = (String) -> Unit

suspend fun fetchRemote(): List<String>

inline fun <reified T> parseJson(raw: String): T = TODO()

public class ServiceWithPrivateField(private val secretKey: String)

private class InternalHelper

private fun secretFunction() {}
"#;
        let symbols = adapter.extract_symbols(source);
        assert!(symbols.iter().any(|s| s.contains("object SingletonConfig")));
        assert!(symbols.iter().any(|s| s.contains("enum class EngineState")));
        assert!(symbols.iter().any(|s| s.contains("value class FileId")));
        assert!(symbols
            .iter()
            .any(|s| s.contains("fun interface EventListener")));
        assert!(symbols
            .iter()
            .any(|s| s.contains("fun onEvent(event: String)")));
        assert!(symbols
            .iter()
            .any(|s| s.contains("typealias ActionHandler")));
        assert!(symbols
            .iter()
            .any(|s| s.contains("suspend fun fetchRemote")));
        assert!(symbols
            .iter()
            .any(|s| s.contains("inline fun <reified T> parseJson")));
        assert!(symbols
            .iter()
            .any(|s| s.contains("public class ServiceWithPrivateField")));
        assert!(!symbols.iter().any(|s| s.contains("InternalHelper")));
        assert!(!symbols.iter().any(|s| s.contains("secretFunction")));
    }

    const KT: &str = r##"package demo

import a.B
import a.C

val RAW = """raw { string"""
val T = "${nested()}"

fun calc(x: Int) = x * 2

private fun helper() {}

class Repo {
    fun load() {
        val s = """
          { unbalanced
        """
        println("${s} }")
    }
}
"##;

    #[test]
    fn test_kotlin_raw_strings_expression_bodies_and_non_code_toc_fallback() {
        use crate::analysis::pack::{pack_files_with, CompressOptions, PackMode, PackRequest};
        let a = KotlinAdapter;
        let spans = a.extract_symbol_spans(KT, "kt");
        let find = |p: &str| {
            spans
                .iter()
                .find(|s| s.signature.starts_with(p))
                .unwrap_or_else(|| panic!("{p}: {spans:?}"))
        };
        let calc = find("fun calc(x: Int)");
        assert_eq!(
            (calc.line_start, calc.line_end, calc.is_public),
            (9, 9, true)
        );
        let helper = find("private fun helper()");
        assert_eq!((helper.line_start, helper.line_end), (11, 11));
        assert!(!helper.is_public);
        let repo = find("class Repo");
        assert_eq!((repo.line_start, repo.line_end), (13, 20), "{spans:?}");
        let load = find("fun load()");
        assert_eq!((load.line_start, load.line_end), (14, 19), "{spans:?}");

        let c = a.compress_with(KT, "kt", &CompressOptions::compact());
        assert!(!c.contains("import a.B"), "{c}");
        assert!(c.contains("imports omitted]"), "{c}");
        assert!(c.contains("fun load() { ⋮ }"), "{c}");
        assert!(c.contains("fun calc(x: Int) = x * 2"), "{c}");
        assert!(!c.contains("unbalanced"), "{c}");
        assert!(c.trim_end().ends_with('}'), "{c}");

        let bodies = a.method_bodies(KT);
        assert!(
            bodies
                .iter()
                .any(|(n, b)| n == "load" && b.contains("unbalanced")),
            "{bodies:?}"
        );

        let root = std::env::temp_dir().join(format!(
            "kt_toc_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Repo.kt"), KT).unwrap();
        std::fs::write(
            root.join("README.md"),
            "# Title\n\ntext\n\n## Usage\n```\n# not heading\n```\n",
        )
        .unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();
        let files: Vec<String> = ["Repo.kt", "README.md", "Cargo.toml"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut req = PackRequest::new(&root, "demo");
        req.mode = PackMode::Toc;
        let toc = pack_files_with(&root, &[], &files, &req);
        assert!(toc.starts_with("[TOC] demo"), "{toc}");
        assert!(toc.contains("Repo.kt (20L)"), "{toc}");
        assert!(toc.contains("[L9-L9] fun calc(x: Int)"), "{toc}");
        assert!(toc.contains("[L14-L19] fun load()"), "{toc}");
        assert!(!toc.contains("println"), "{toc}");
        assert!(toc.contains("README.md (8L)"), "{toc}");
        assert!(toc.contains("[L1] # Title"), "{toc}");
        assert!(toc.contains("[L5] ## Usage"), "{toc}");
        assert!(!toc.contains("not heading"), "{toc}");
        assert!(toc.contains("[L1] [package]"), "{toc}");
        assert!(toc.contains("[L4] [dependencies]"), "{toc}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
