use super::{LanguageAdapter, ELIDED_BODY};
use crate::analysis::detector::{clean_multiline_signature, sanitize_symbol};

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
        let mut idx = 0;
        let mut symbols = Vec::new();

        while idx < lines.len() {
            let line = lines[idx];
            if is_symbol_start(line) {
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
        let mut out = String::with_capacity(source.len() / 3);
        let mut depth: usize = 0;
        let mut eliding = false;
        let mut elide_target_depth = 0;
        let mut in_block_comment = false;
        let mut in_raw_str = false;

        for line in source.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                if !eliding {
                    out.push('\n');
                }
                continue;
            }

            let is_container_decl = is_container_declaration(trimmed);
            let has_fun_decl = has_fun_declaration(trimmed);

            let mut prev_char = '\0';
            let mut in_line_comment = false;
            let mut in_str: Option<char> = None;
            let mut line_should_elide = false;
            let mut body_start_indent = "";

            let chars: Vec<char> = trimmed.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let c = chars[i];

                if in_line_comment {
                    break;
                }

                if in_block_comment {
                    if prev_char == '*' && c == '/' {
                        in_block_comment = false;
                    }
                    prev_char = c;
                    i += 1;
                    continue;
                }

                if in_raw_str {
                    if prev_char == '"' && c == '"' && i + 1 < chars.len() && chars[i + 1] == '"' {
                        in_raw_str = false;
                        i += 2;
                        prev_char = '"';
                        continue;
                    }
                    prev_char = c;
                    i += 1;
                    continue;
                }

                if let Some(q) = in_str {
                    if c == q && prev_char != '\\' {
                        in_str = None;
                    }
                    prev_char = c;
                    i += 1;
                    continue;
                }

                if c == '"' && i + 2 < chars.len() && chars[i + 1] == '"' && chars[i + 2] == '"' {
                    in_raw_str = true;
                    i += 3;
                    prev_char = '"';
                    continue;
                }

                if prev_char == '/' && c == '/' {
                    in_line_comment = true;
                    prev_char = c;
                    i += 1;
                    continue;
                }
                if prev_char == '/' && c == '*' {
                    in_block_comment = true;
                    prev_char = c;
                    i += 1;
                    continue;
                }

                if c == '"' || c == '\'' || c == '`' {
                    in_str = Some(c);
                } else if c == '{' {
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
                            let indent_len = line.len() - line.trim_start().len();
                            body_start_indent = &line[..indent_len];
                        }
                    }
                } else if c == '}' {
                    depth = depth.saturating_sub(1);
                    if eliding && depth == elide_target_depth {
                        eliding = false;
                    }
                }

                prev_char = c;
                i += 1;
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
}
