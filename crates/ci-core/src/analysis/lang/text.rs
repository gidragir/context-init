//! Shared lexical helpers for adapters: literal/comment masking, line-wise application of
//! [`CompressOptions`], brace-delimited declaration spans and symbol names.

use super::{SymbolSpan, ELIDED_BODY, INLINE_ELIDED};
use crate::analysis::pack::CompressOptions;

// ---------------------------------------------------------------------------------------
// Masking
// ---------------------------------------------------------------------------------------

/// How backtick literals behave in a language.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Backtick {
    /// Single-line quoted identifier / string (Kotlin, Java, C#).
    Str,
    /// Multi-line raw string without escapes or templates (Go).
    Raw,
    /// Multi-line template literal with `${...}` (TypeScript / JavaScript).
    Template,
}

/// Lexical flavor of a language for [`mask_code`].
#[derive(Clone, Copy)]
pub(crate) struct Flavor {
    /// `"""..."""` (and `'''...'''` with `hash_comments`) multi-line strings.
    pub triple_quotes: bool,
    /// `"${expr}"` templates inside double-quoted and triple-quoted strings (Kotlin).
    pub dollar_templates: bool,
    pub backtick: Backtick,
    /// `#` line comments instead of `//` and `/* */` (Python).
    pub hash_comments: bool,
    /// Rust: raw strings `r#"..."#`, lifetimes vs char literals, nested block comments.
    pub rust: bool,
}

impl Flavor {
    pub(crate) fn kotlin() -> Self {
        Self {
            triple_quotes: true,
            dollar_templates: true,
            backtick: Backtick::Str,
            hash_comments: false,
            rust: false,
        }
    }

    pub(crate) fn python() -> Self {
        Self {
            triple_quotes: true,
            dollar_templates: false,
            backtick: Backtick::Str,
            hash_comments: true,
            rust: false,
        }
    }

    pub(crate) fn rust() -> Self {
        Self {
            triple_quotes: false,
            dollar_templates: false,
            backtick: Backtick::Str,
            hash_comments: false,
            rust: true,
        }
    }

    /// Flavor of a brace-language extension (`go`, `ts`, `java`, ...).
    pub(crate) fn brace(ext: &str) -> Self {
        let backtick = match ext {
            "go" => Backtick::Raw,
            "java" | "cs" => Backtick::Str,
            _ => Backtick::Template,
        };
        Self {
            triple_quotes: false,
            dollar_templates: false,
            backtick,
            hash_comments: false,
            rust: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Ctx {
    Str(char),
    Triple(char),
    Template(usize),
    Tick(bool),
    RawRust(usize),
}

fn push_masked(out: &mut String, chars: &[char]) {
    for &c in chars {
        out.push(match c {
            '\n' => '\n',
            '\r' => '\r',
            _ => ' ',
        });
    }
}

/// Lines of `source` where string/char literals (incl. template expressions) and comments
/// are replaced by spaces, char-for-char, so braces and keywords can be scanned safely and
/// char columns map 1:1 onto the original lines.
pub(crate) fn mask_code(source: &str, f: Flavor) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let at = |k: usize| chars.get(k).copied();
    let mut out = String::with_capacity(source.len());
    let mut stack: Vec<Ctx> = Vec::new();
    let mut block = 0usize;
    let mut line_comment = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let n = at(i + 1);
        if c == '\n' {
            line_comment = false;
            while matches!(stack.last(), Some(Ctx::Str(_))) {
                stack.pop();
            }
            out.push('\n');
            i += 1;
            continue;
        }
        if c == '\r' {
            out.push('\r');
            i += 1;
            continue;
        }
        if line_comment {
            out.push(' ');
            i += 1;
            continue;
        }
        if block > 0 {
            if c == '*' && n == Some('/') {
                block -= 1;
                out.push_str("  ");
                i += 2;
            } else if f.rust && c == '/' && n == Some('*') {
                block += 1;
                out.push_str("  ");
                i += 2;
            } else {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        let top = stack.last().copied();
        if matches!(top, None | Some(Ctx::Template(_))) {
            if f.hash_comments {
                if c == '#' {
                    line_comment = true;
                    out.push(' ');
                    i += 1;
                    continue;
                }
            } else if c == '/' && n == Some('/') {
                line_comment = true;
                out.push(' ');
                i += 1;
                continue;
            } else if c == '/' && n == Some('*') {
                block = 1;
                out.push_str("  ");
                i += 2;
                continue;
            }
            let mut taken = 0usize;
            let triple_q = c == '"' || (f.hash_comments && c == '\'');
            if f.triple_quotes && triple_q && n == Some(c) && at(i + 2) == Some(c) {
                stack.push(Ctx::Triple(c));
                taken = 3;
            } else if f.rust
                && (c == 'r' || c == 'b')
                && !(i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_'))
            {
                let mut j = i + 1;
                let raw = if c == 'b' {
                    if at(j) == Some('r') {
                        j += 1;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                };
                if raw {
                    let mut hashes = 0;
                    while at(j) == Some('#') {
                        hashes += 1;
                        j += 1;
                    }
                    if at(j) == Some('"') {
                        stack.push(Ctx::RawRust(hashes));
                        taken = j + 1 - i;
                    }
                }
            }
            if taken == 0 {
                if c == '"' {
                    stack.push(Ctx::Str('"'));
                    taken = 1;
                } else if c == '\'' {
                    if f.rust {
                        if n == Some('\\') {
                            let mut j = i + 3;
                            while j < chars.len() && j < i + 12 && chars[j] != '\'' {
                                j += 1;
                            }
                            taken = (j + 1).min(chars.len()) - i;
                        } else if at(i + 2) == Some('\'') && n != Some('\n') {
                            taken = 3;
                        } else {
                            // Lifetime / label.
                            out.push(c);
                            i += 1;
                            continue;
                        }
                    } else {
                        stack.push(Ctx::Str('\''));
                        taken = 1;
                    }
                } else if c == '`' {
                    stack.push(match f.backtick {
                        Backtick::Str => Ctx::Str('`'),
                        Backtick::Raw => Ctx::Tick(false),
                        Backtick::Template => Ctx::Tick(true),
                    });
                    taken = 1;
                }
            }
            if taken > 0 {
                push_masked(&mut out, &chars[i..i + taken]);
                i += taken;
                continue;
            }
            if let Some(Ctx::Template(d)) = stack.last_mut() {
                if c == '{' {
                    *d += 1;
                } else if c == '}' {
                    if *d == 0 {
                        stack.pop();
                    } else {
                        *d -= 1;
                    }
                }
                out.push(' ');
                i += 1;
                continue;
            }
            out.push(c);
            i += 1;
            continue;
        }
        let mut taken = 1usize;
        match top {
            Some(Ctx::Str(q)) => {
                if c == '\\' && q != '`' && n.is_some_and(|x| x != '\n') {
                    taken = 2;
                } else if c == q {
                    stack.pop();
                } else if f.dollar_templates && q == '"' && c == '$' && n == Some('{') {
                    stack.push(Ctx::Template(0));
                    taken = 2;
                }
            }
            Some(Ctx::Triple(q)) => {
                if c == q && n == Some(q) && at(i + 2) == Some(q) {
                    let mut j = i + 3;
                    while at(j) == Some(q) {
                        j += 1;
                    }
                    stack.pop();
                    taken = j - i;
                } else if f.dollar_templates && c == '$' && n == Some('{') {
                    stack.push(Ctx::Template(0));
                    taken = 2;
                } else if !f.dollar_templates && c == '\\' && n.is_some() {
                    taken = 2;
                }
            }
            Some(Ctx::Tick(tpl)) => {
                if tpl && c == '\\' && n.is_some() {
                    taken = 2;
                } else if c == '`' {
                    stack.pop();
                } else if tpl && c == '$' && n == Some('{') {
                    stack.push(Ctx::Template(0));
                    taken = 2;
                }
            }
            Some(Ctx::RawRust(h)) => {
                if c == '"' && (1..=h).all(|k| at(i + k) == Some('#')) {
                    stack.pop();
                    taken = h + 1;
                }
            }
            Some(Ctx::Template(_)) | None => {}
        }
        let end = (i + taken).min(chars.len());
        push_masked(&mut out, &chars[i..end]);
        i = end;
    }
    out.lines().map(str::to_string).collect()
}

// ---------------------------------------------------------------------------------------
// Line-wise option application
// ---------------------------------------------------------------------------------------

/// Line predicates of a language for [`apply_options`]. All take a trimmed line.
pub(crate) struct LineRules {
    pub comment: &'static str,
    pub block_comments: bool,
    pub is_import: fn(&str) -> bool,
    pub is_private: fn(&str) -> bool,
    pub is_test: fn(&str) -> bool,
}

fn never(_: &str) -> bool {
    false
}

fn c_import(t: &str) -> bool {
    t.starts_with("import ")
        || t.starts_with("package ")
        || (t.starts_with("using ") && t.ends_with(';'))
}

fn py_import(t: &str) -> bool {
    t.starts_with("import ") || (t.starts_with("from ") && t.contains(" import "))
}

/// `private` modifier before the parameter list (Kotlin, Java, C#, TypeScript), or `#name(`.
fn c_private(t: &str) -> bool {
    let head = t.split('(').next().unwrap_or(t);
    head.split_whitespace().any(|w| w == "private") || t.starts_with('#')
}

/// Go: functions, methods and types with a lower-case name are package-private.
pub(crate) fn go_private(t: &str) -> bool {
    let rest = if let Some(r) = t.strip_prefix("func ") {
        if r.starts_with('(') {
            r.split_once(')').map_or("", |(_, tail)| tail.trim_start())
        } else {
            r
        }
    } else if let Some(r) = t.strip_prefix("type ") {
        r
    } else {
        return false;
    };
    rest.chars().next().is_some_and(|c| c.is_lowercase())
}

fn py_name(t: &str) -> Option<&str> {
    t.strip_prefix("async def ")
        .or_else(|| t.strip_prefix("def "))
        .or_else(|| t.strip_prefix("class "))
}

fn py_private(t: &str) -> bool {
    py_name(t).is_some_and(|n| n.starts_with('_') && !n.starts_with("__"))
}

fn py_test(t: &str) -> bool {
    py_name(t).is_some_and(|n| n.starts_with("test_") || n.starts_with("Test"))
}

impl LineRules {
    /// `//` comments, `import`/`package`/`using` preambles, `private` declarations.
    pub(crate) fn c_like() -> Self {
        Self {
            comment: "//",
            block_comments: true,
            is_import: c_import,
            is_private: c_private,
            is_test: never,
        }
    }

    pub(crate) fn go() -> Self {
        Self {
            is_private: go_private,
            ..Self::c_like()
        }
    }

    pub(crate) fn python() -> Self {
        Self {
            comment: "#",
            block_comments: false,
            is_import: py_import,
            is_private: py_private,
            is_test: py_test,
        }
    }

    pub(crate) fn bsl(is_private: fn(&str) -> bool) -> Self {
        Self {
            comment: "//",
            block_comments: false,
            is_import: never,
            is_private,
            is_test: never,
        }
    }
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// BSL method terminators (`КонецПроцедуры`, `EndFunction`, ...).
fn is_end_keyword(t: &str) -> bool {
    let l = t.to_lowercase();
    [
        "конецпроцедуры",
        "конецфункции",
        "endprocedure",
        "endfunction",
    ]
    .iter()
    .any(|k| l.starts_with(k))
}

/// Applies `strip_comments`, `public_only` / `exclude_tests`, `strip_imports` and
/// `inline_elide` to an already compressed text, line by line.
pub(crate) fn apply_options(text: &str, opts: &CompressOptions, rules: &LineRules) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if opts.strip_comments {
        lines = strip_comment_lines(lines, rules);
    }
    if opts.public_only || opts.exclude_tests {
        lines = drop_decls(lines, |t| {
            (opts.public_only && (rules.is_private)(t))
                || (opts.exclude_tests && (rules.is_test)(t))
        });
    }
    if opts.strip_imports {
        lines = collapse_imports(lines, rules);
    }
    if opts.inline_elide {
        lines = inline_elide(lines);
    }
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for l in lines {
        if l.trim().is_empty() && out.last().is_none_or(|p: &String| p.trim().is_empty()) {
            continue;
        }
        out.push(l);
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') && !joined.is_empty() {
        joined.push('\n');
    }
    joined
}

fn strip_comment_lines(lines: Vec<String>, rules: &LineRules) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut in_block = false;
    for l in lines {
        let t = l.trim();
        if in_block {
            if t.contains("*/") {
                in_block = false;
            }
            continue;
        }
        if rules.block_comments && t.starts_with("/*") {
            match t[2..].find("*/") {
                None => in_block = true,
                Some(p) if !t[2 + p + 2..].trim().is_empty() => out.push(l),
                Some(_) => {}
            }
            continue;
        }
        if t.starts_with(rules.comment) && !(rules.comment == "#" && t.starts_with("#!")) {
            continue;
        }
        out.push(l);
    }
    out
}

fn drop_decls(lines: Vec<String>, pred: impl Fn(&str) -> bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        if t.is_empty() || !pred(t) {
            out.push(lines[i].clone());
            i += 1;
            continue;
        }
        let ind = indent_of(&lines[i]);
        // Annotations / directives of the dropped declaration.
        while out.last().is_some_and(|p| {
            let pt = p.trim();
            indent_of(p) == ind
                && (pt.starts_with('@') || pt.starts_with("#[") || pt.starts_with('&'))
        }) {
            out.pop();
        }
        let opens = t.matches('{').count() > t.matches('}').count();
        i += 1;
        while i < lines.len() && (lines[i].trim().is_empty() || indent_of(&lines[i]) > ind) {
            i += 1;
        }
        if i < lines.len() && indent_of(&lines[i]) == ind {
            let nt = lines[i].trim();
            if (opens && nt.starts_with('}')) || is_end_keyword(nt) {
                i += 1;
            }
        }
    }
    out
}

fn collapse_imports(lines: Vec<String>, rules: &LineRules) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut count = 0usize;
    let mut marker: Option<usize> = None;
    // Open multi-line import: Go `import (` (one entry per line) or TS/JS `import {`
    // (one statement; its lines, incl. a legacy `⋮----` marker, end at the line with `}`).
    enum Group {
        Paren,
        Brace,
    }
    let mut group: Option<Group> = None;
    for l in lines {
        let t = l.trim();
        match group {
            Some(Group::Paren) => {
                if t == ")" {
                    group = None;
                } else if !t.is_empty() {
                    count += 1;
                }
                continue;
            }
            Some(Group::Brace) => {
                if t.starts_with('}') {
                    group = None;
                }
                continue;
            }
            None => {}
        }
        if indent_of(&l) == 0 && (rules.is_import)(t) {
            if marker.is_none() {
                marker = Some(out.len());
                out.push(String::new());
            }
            if t.ends_with('(') {
                group = Some(Group::Paren);
            } else {
                count += 1;
                if t.matches('{').count() > t.matches('}').count() {
                    group = Some(Group::Brace);
                }
            }
            continue;
        }
        out.push(l);
    }
    if let Some(p) = marker {
        out[p] = format!("{} [{count} imports omitted]", rules.comment);
    }
    out
}

fn inline_elide(lines: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == ELIDED_BODY {
            while out.last().is_some_and(|l| l.trim().is_empty()) {
                out.pop();
            }
            if let Some(prev) = out.last_mut() {
                let head = prev.trim_end().to_string();
                let next = lines.get(i + 1).map(|l| l.trim());
                match next {
                    Some(nt)
                        if (head.ends_with('{') && nt.starts_with('}')) || is_end_keyword(nt) =>
                    {
                        *prev = format!("{head} {INLINE_ELIDED} {nt}");
                        i += 2;
                    }
                    _ => {
                        *prev = format!("{head} {INLINE_ELIDED}");
                        i += 1;
                    }
                }
                continue;
            }
        }
        out.push(lines[i].clone());
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------------------
// Brace-delimited spans
// ---------------------------------------------------------------------------------------

/// Declaration detector: `(masked trimmed line, kind of the innermost enclosing span)` ->
/// `(kind, is_public)`.
pub(crate) type DetectFn<'a> =
    &'a dyn Fn(&str, Option<&'static str>) -> Option<(&'static str, bool)>;

pub(crate) struct SpanRules<'a> {
    pub detect: DetectFn<'a>,
    /// `=` at nesting depth 0 starts an expression body (Kotlin `fun f() = ...`).
    pub expr_bodies: bool,
    /// Declarations may end at the end of a line without `{` / `;` (Kotlin, Go, TS, Java).
    pub bodyless: bool,
    /// Original-line prefixes (attributes / annotations) pulled into the span start.
    pub attr_prefixes: &'a [&'a str],
}

struct Scan {
    sig_line: usize,
    sig_col: usize,
    end: usize,
}

fn match_brace(masked: &[String], line: usize, col: usize) -> usize {
    let mut depth = 0i64;
    for (j, l) in masked.iter().enumerate().skip(line) {
        let skip = if j == line { col } else { 0 };
        for c in l.chars().skip(skip) {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    return j;
                }
            }
        }
    }
    masked.len().saturating_sub(1)
}

fn next_code_line(masked: &[String], from: usize) -> Option<&str> {
    masked
        .iter()
        .skip(from)
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
}

fn expr_end(masked: &[String], line: usize, col: usize) -> usize {
    let mut depth = 0i64;
    let mut j = line;
    let limit = (line + 200).min(masked.len());
    while j < limit {
        let skip = if j == line { col } else { 0 };
        for c in masked[j].chars().skip(skip) {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth < 0 {
                        return j;
                    }
                }
                _ => {}
            }
        }
        let t = masked[j].trim_end();
        let next = next_code_line(masked, j + 1);
        let continues = depth > 0
            || t.ends_with(['=', '.', '+', '-', '*', '/', ',', '(', '?', ':'])
            || t.ends_with("&&")
            || t.ends_with("||")
            || next.is_some_and(|n| {
                n.starts_with('.')
                    || n.starts_with("?.")
                    || n.starts_with("?:")
                    || n.starts_with("&&")
                    || n.starts_with("||")
            });
        if !continues {
            return j;
        }
        j += 1;
    }
    limit.saturating_sub(1).max(line)
}

fn is_assign(chars: &[char], k: usize) -> bool {
    let next = chars.get(k + 1).copied();
    let prev = if k > 0 { Some(chars[k - 1]) } else { None };
    next != Some('=') && next != Some('>') && !prev.is_some_and(|p| "=!<>+-*/%&|^:".contains(p))
}

fn scan_decl(masked: &[String], i: usize, rules: &SpanRules<'_>) -> Option<Scan> {
    let mut parens = 0i64;
    let limit = (i + 40).min(masked.len());
    for j in i..limit {
        let chars: Vec<char> = masked[j].chars().collect();
        for (k, &c) in chars.iter().enumerate() {
            match c {
                '(' | '[' => parens += 1,
                ')' | ']' => parens = (parens - 1).max(0),
                '{' if parens == 0 => {
                    return Some(Scan {
                        sig_line: j,
                        sig_col: k,
                        end: match_brace(masked, j, k),
                    })
                }
                ';' if parens == 0 => {
                    return Some(Scan {
                        sig_line: j,
                        sig_col: k,
                        end: j,
                    })
                }
                '=' if parens == 0 && rules.expr_bodies && is_assign(&chars, k) => {
                    return Some(Scan {
                        sig_line: j,
                        sig_col: k,
                        end: expr_end(masked, j, k + 1),
                    })
                }
                _ => {}
            }
        }
        if rules.bodyless && parens == 0 {
            let t = masked[j].trim_end();
            let next = next_code_line(masked, j + 1);
            let continues = t.ends_with([',', ':', '(', '='])
                || t.ends_with("->")
                || next.is_some_and(|n| {
                    n.starts_with('{')
                        || n.starts_with(':')
                        || n.starts_with("where")
                        || n.starts_with(')')
                        || n.starts_with('=')
                        || n.starts_with(',')
                });
            if !continues {
                return Some(Scan {
                    sig_line: j,
                    sig_col: chars.len(),
                    end: j,
                });
            }
        }
    }
    None
}

/// Whitespace-collapsed text of lines `start..=sig_line`, the last one cut at `sig_col` chars.
pub(crate) fn signature_text(
    lines: &[&str],
    start: usize,
    sig_line: usize,
    sig_col: usize,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (j, l) in lines.iter().enumerate().take(sig_line + 1).skip(start) {
        parts.push(if j == sig_line {
            l.chars().take(sig_col).collect()
        } else {
            (*l).to_string()
        });
    }
    let joined = parts.join(" ");
    let collapsed = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_end_matches('{').trim().to_string()
}

/// Spans of brace-delimited declarations found by `rules.detect` on masked lines.
pub(crate) fn brace_spans(
    source: &str,
    masked: &[String],
    rules: &SpanRules<'_>,
) -> Vec<SymbolSpan> {
    let lines: Vec<&str> = source.lines().collect();
    let mut spans = Vec::new();
    let mut open: Vec<(usize, &'static str)> = Vec::new();
    for i in 0..masked.len().min(lines.len()) {
        while open.last().is_some_and(|(end, _)| *end < i) {
            open.pop();
        }
        let t = masked[i].trim();
        if t.is_empty() {
            continue;
        }
        let parent = open.last().map(|(_, k)| *k);
        let Some((kind, is_public)) = (rules.detect)(t, parent) else {
            continue;
        };
        let Some(scan) = scan_decl(masked, i, rules) else {
            continue;
        };
        let signature = signature_text(&lines, i, scan.sig_line, scan.sig_col);
        if signature.is_empty() {
            continue;
        }
        let mut start = i;
        while start > 0 {
            let p = lines[start - 1].trim();
            if !p.is_empty() && rules.attr_prefixes.iter().any(|a| p.starts_with(a)) {
                start -= 1;
            } else {
                break;
            }
        }
        spans.push(SymbolSpan {
            kind,
            signature,
            is_public,
            line_start: start + 1,
            line_end: scan.end.max(i) + 1,
        });
        open.push((scan.end.max(i), kind));
    }
    spans
}

/// `(name, exact text)` of spans whose kind is in `kinds`.
pub(crate) fn span_bodies(
    source: &str,
    spans: &[SymbolSpan],
    kinds: &[&str],
) -> Vec<(String, String)> {
    let lines: Vec<&str> = source.lines().collect();
    spans
        .iter()
        .filter(|s| kinds.contains(&s.kind))
        .map(|s| {
            let end = s.line_end.min(lines.len());
            let start = (s.line_start - 1).min(end);
            (symbol_name(s), lines[start..end].join("\n"))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn strip_generics(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    let mut prev = ' ';
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' if depth > 0 && prev != '-' && prev != '=' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
        prev = c;
    }
    out
}

const CONTAINER_KINDS: &[&str] = &[
    "class",
    "interface",
    "object",
    "struct",
    "enum",
    "trait",
    "record",
    "type",
    "typealias",
    "mod",
    "union",
    "const",
    "static",
];

pub(crate) fn symbol_name(span: &SymbolSpan) -> String {
    let sig = strip_generics(&span.signature);
    let ident = |w: &str| -> String { w.chars().take_while(|c| is_ident_char(*c)).collect() };
    if span.kind == "impl" {
        let tail = sig
            .rsplit_once(" for ")
            .map(|(_, r)| r.to_string())
            .unwrap_or_else(|| {
                sig.split_once("impl")
                    .map_or(String::new(), |(_, r)| r.to_string())
            });
        let first = tail.split_whitespace().next().unwrap_or("");
        return ident(first.rsplit("::").next().unwrap_or(first));
    }
    if CONTAINER_KINDS.contains(&span.kind) {
        let words: Vec<&str> = sig.split_whitespace().collect();
        if let Some(p) = words.iter().position(|w| *w == span.kind) {
            let name = words.get(p + 1).map(|w| ident(w)).unwrap_or_default();
            if !name.is_empty() {
                return name;
            }
            if span.kind == "object" {
                return "Companion".to_string();
            }
        }
    }
    let mut s = sig.as_str();
    if let Some(r) = s.strip_prefix("func (") {
        s = r.split_once(')').map_or(r, |(_, tail)| tail);
    }
    let before = s.split('(').next().unwrap_or(s);
    let before = before.trim_end_matches(|c: char| !is_ident_char(c));
    let rev: String = before
        .chars()
        .rev()
        .take_while(|c| is_ident_char(*c))
        .collect();
    let name: String = rev.chars().rev().collect();
    if name.is_empty() {
        sig.split_whitespace().last().map(ident).unwrap_or_default()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_handles_kotlin_raw_strings_and_templates() {
        let src = "val a = \"\"\"x { y\"\"\"\nval b = \"${f(\"}\")} {\"\nfun g() {}\n";
        let m = mask_code(src, Flavor::kotlin());
        assert_eq!(m.len(), 3);
        assert!(!m[0].contains('{') && !m[1].contains('{'), "{m:?}");
        assert!(m[2].contains("fun g() {}"));
    }

    #[test]
    fn mask_rust_lifetimes_chars_and_raw_strings() {
        let src = "fn f<'a>(x: &'a str) -> char { let r = r#\"{\"#; '{' }\n";
        let m = mask_code(src, Flavor::rust());
        assert_eq!(m[0].matches('{').count(), 1, "{m:?}");
        assert!(m[0].contains("<'a>"));
    }
}
