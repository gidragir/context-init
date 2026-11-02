use std::fs;
use std::path::Path;

#[derive(Debug, PartialEq, Clone)]
enum Token {
    Word(String),
    StringLit(String),
    ParenOpen,
    ParenClose,
    Comma,
    Semicolon,
    Newline,
}

fn strip_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_quote: Option<char> = None;

    while let Some(c) = chars.next() {
        if let Some(quote) = in_quote {
            result.push(c);
            if c == '\\' {
                if let Some(escaped) = chars.next() {
                    result.push(escaped);
                }
            } else if c == quote {
                in_quote = None;
            }
        } else if c == '"' || c == '\'' {
            in_quote = Some(c);
            result.push(c);
        } else if c == '/' && chars.peek() == Some(&'/') {
            chars.next(); // consume '/'
            for next in chars.by_ref() {
                if next == '\n' {
                    result.push('\n');
                    break;
                }
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next(); // consume '*'
            while let Some(next) = chars.next() {
                if next == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn extract_first_string(input: &str) -> Option<String> {
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '"' || c == '\'' {
            let quote = c;
            let mut s = String::new();
            while let Some(ch) = chars.next() {
                if ch == '\\' {
                    if let Some(esc) = chars.next() {
                        s.push(esc);
                    }
                } else if ch == quote {
                    return Some(s);
                } else {
                    s.push(ch);
                }
            }
            return Some(s);
        }
    }
    None
}

fn extract_root_project_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("rootProject.name") {
            if let Some((_, right)) = trimmed.split_once('=') {
                if let Some(val) = extract_first_string(right) {
                    let s = val.trim();
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
                let clean = right
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'' || c == ';' || c == ' ');
                if !clean.is_empty() {
                    return Some(clean.to_string());
                }
            } else if let Some(val) = extract_first_string(trimmed) {
                let s = val.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        } else if trimmed.starts_with("rootProject.setName") {
            if let Some(val) = extract_first_string(trimmed) {
                let s = val.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            '\n' => {
                chars.next();
                tokens.push(Token::Newline);
            }
            '(' => {
                chars.next();
                tokens.push(Token::ParenOpen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::ParenClose);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            ';' => {
                chars.next();
                tokens.push(Token::Semicolon);
            }
            '"' | '\'' => {
                let quote = chars.next().unwrap();
                let mut s = String::new();
                while let Some(ch) = chars.next() {
                    if ch == '\\' {
                        if let Some(esc) = chars.next() {
                            s.push(esc);
                        }
                    } else if ch == quote {
                        break;
                    } else {
                        s.push(ch);
                    }
                }
                tokens.push(Token::StringLit(s));
            }
            c if c.is_alphanumeric() || c == '_' || c == '.' => {
                let mut word = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' || ch == '.' {
                        word.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Word(word));
            }
            _ => {
                chars.next();
            }
        }
    }

    tokens
}

fn normalize_and_validate_member(root: &Path, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let p = Path::new(trimmed);
    let path_str = if p.is_absolute() {
        if let Ok(rel) = p.strip_prefix(root) {
            rel.to_string_lossy().to_string()
        } else {
            return None;
        }
    } else {
        trimmed.to_string()
    };

    let segments: Vec<&str> = path_str
        .split([':', '/'])
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();

    if segments.is_empty() {
        return None;
    }

    if segments.contains(&"..") {
        return None;
    }

    let normalized = segments.join("/");
    let target = root.join(&normalized);

    if let Ok(canonical) = target.canonicalize() {
        if let Ok(root_canonical) = root.canonicalize() {
            if !canonical.starts_with(&root_canonical) {
                return None;
            }
        }
    }

    if target.is_dir() {
        Some(normalized)
    } else {
        None
    }
}

fn parse_includes_and_builds(root: &Path, content: &str) -> Vec<String> {
    let tokens = tokenize(content);
    let mut members = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Word(w) if w == "includeBuild" => {
                i += 1;
                while i < tokens.len() && tokens[i] == Token::Newline {
                    i += 1;
                }
                if i < tokens.len() && tokens[i] == Token::ParenOpen {
                    i += 1;
                    let mut found_lit = None;
                    while i < tokens.len() && tokens[i] != Token::ParenClose {
                        if let Token::StringLit(raw) = &tokens[i] {
                            if found_lit.is_none() {
                                found_lit = Some(raw.clone());
                            }
                        }
                        i += 1;
                    }
                    if i < tokens.len() && tokens[i] == Token::ParenClose {
                        i += 1;
                    }
                    if let Some(raw) = found_lit {
                        if let Some(norm) = normalize_and_validate_member(root, &raw) {
                            if !members.contains(&norm) {
                                members.push(norm);
                            }
                        }
                    }
                } else if i < tokens.len() {
                    if let Token::StringLit(raw) = &tokens[i] {
                        if let Some(norm) = normalize_and_validate_member(root, raw) {
                            if !members.contains(&norm) {
                                members.push(norm);
                            }
                        }
                        i += 1;
                    }
                }
            }
            Token::Word(w) if w == "include" => {
                i += 1;
                if i < tokens.len() && tokens[i] == Token::ParenOpen {
                    i += 1;
                    while i < tokens.len() && tokens[i] != Token::ParenClose {
                        if let Token::StringLit(raw) = &tokens[i] {
                            if let Some(norm) = normalize_and_validate_member(root, raw) {
                                if !members.contains(&norm) {
                                    members.push(norm);
                                }
                            }
                        }
                        i += 1;
                    }
                    if i < tokens.len() && tokens[i] == Token::ParenClose {
                        i += 1;
                    }
                } else {
                    let mut saw_comma = true;
                    while i < tokens.len() {
                        if tokens[i] == Token::Newline {
                            if saw_comma {
                                i += 1;
                                continue;
                            } else {
                                break;
                            }
                        }
                        if let Token::StringLit(raw) = &tokens[i] {
                            if let Some(norm) = normalize_and_validate_member(root, raw) {
                                if !members.contains(&norm) {
                                    members.push(norm);
                                }
                            }
                            saw_comma = false;
                            i += 1;
                        } else if tokens[i] == Token::Comma {
                            saw_comma = true;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    members
}

/// Парсит Gradle workspace: извлекает имя проекта из rootProject.name и
/// список подпроектов из include(...) и includeBuild(...).
pub fn parse_gradle_workspace(root: &Path) -> (Option<String>, Vec<String>) {
    let settings_kts = root.join("settings.gradle.kts");
    let settings_groovy = root.join("settings.gradle");

    let mut name = None;
    let mut members = Vec::new();

    for path in [&settings_kts, &settings_groovy] {
        if let Ok(content) = fs::read_to_string(path) {
            let clean = strip_comments(&content);
            if name.is_none() {
                name = extract_root_project_name(&clean);
            }
            let parsed_members = parse_includes_and_builds(root, &clean);
            for m in parsed_members {
                if !members.contains(&m) {
                    members.push(m);
                }
            }
        }
    }

    (name, members)
}
