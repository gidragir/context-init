use super::{LanguageAdapter, ELIDED_BODY};

/// BSL / OneScript modules (`.bsl`, `.os`): Cyrillic and English keywords, `&Directive`
/// annotations, CRLF line endings.
pub struct BslAdapter;

const METHOD_STARTS: [&str; 8] = [
    "процедура",
    "функция",
    "procedure",
    "function",
    "асинх процедура",
    "асинх функция",
    "async procedure",
    "async function",
];
const METHOD_ENDS: [&str; 4] = [
    "конецпроцедуры",
    "конецфункции",
    "endprocedure",
    "endfunction",
];
const EXPORT: [&str; 2] = ["экспорт", "export"];

/// `line` starts with `keyword` followed by a non-identifier character (case-insensitive).
fn starts_with_keyword(line: &str, keywords: &[&str]) -> bool {
    let lower = line
        .trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .to_lowercase();
    keywords.iter().any(|kw| {
        lower.strip_prefix(kw).is_some_and(|rest| {
            rest.chars()
                .next()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
        })
    })
}

/// `line` without its `//` comment and without the contents of "string literals".
fn code_part(line: &str) -> String {
    let mut out = String::new();
    let mut in_string = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_string = !in_string;
                out.push(c);
            }
            '/' if !in_string && chars.peek() == Some(&'/') => break,
            _ if in_string => {}
            _ => out.push(c),
        }
    }
    out
}

/// Index of the line closing the parameter list that starts at `start`.
fn signature_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0i32;
    for (i, line) in lines.iter().enumerate().skip(start) {
        let code = code_part(line);
        depth += code.matches('(').count() as i32 - code.matches(')').count() as i32;
        if depth <= 0 && code.contains(')') {
            return i;
        }
    }
    start
}

/// Signature (possibly spread over several lines) as one normalized line.
fn joined_signature(lines: &[&str], start: usize, end: usize) -> String {
    lines[start..=end]
        .iter()
        .map(|l| {
            l.trim_start_matches('\u{feff}')
                .split("//")
                .next()
                .unwrap_or("")
                .trim()
        })
        .collect::<Vec<_>>()
        .join(" ")
        .replace("( ", "(")
}

fn has_export(signature: &str) -> bool {
    let code = code_part(signature);
    let tail = code
        .rsplit_once(')')
        .map_or("", |(_, tail)| tail)
        .to_lowercase();
    EXPORT
        .iter()
        .any(|e| tail.split_whitespace().any(|w| w == *e))
}

impl LanguageAdapter for BslAdapter {
    fn language_id(&self) -> &'static str {
        "bsl"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bsl", "os"]
    }

    fn extract_symbols(&self, source: &str) -> Vec<String> {
        let lines: Vec<&str> = source.lines().collect();
        let mut symbols = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if starts_with_keyword(lines[i], &METHOD_STARTS) {
                let end = signature_end(&lines, i);
                let signature = joined_signature(&lines, i, end);
                if has_export(&signature) {
                    symbols.push(signature);
                }
                i = end;
            }
            i += 1;
        }
        symbols
    }

    fn method_bodies(&self, source: &str) -> Vec<(String, String)> {
        let lines: Vec<&str> = source.lines().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if starts_with_keyword(lines[i], &METHOD_STARTS) {
                let end = signature_end(&lines, i);
                let signature = joined_signature(&lines, i, end);
                let name = signature
                    .split_whitespace()
                    .find(|w| {
                        let l = w.to_lowercase();
                        !(METHOD_STARTS.contains(&l.as_str()) || l == "асинх" || l == "async")
                    })
                    .unwrap_or("")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .to_string();
                let mut j = end + 1;
                while j < lines.len()
                    && !starts_with_keyword(lines[j], &METHOD_ENDS)
                    && !starts_with_keyword(lines[j], &METHOD_STARTS)
                {
                    j += 1;
                }
                let last = if j < lines.len() && starts_with_keyword(lines[j], &METHOD_ENDS) {
                    j
                } else {
                    j.saturating_sub(1)
                };
                out.push((name, lines[i..=last.max(i)].join("\n")));
                i = last + 1;
                continue;
            }
            i += 1;
        }
        out
    }

    fn compress_source(&self, source: &str) -> String {
        let lines: Vec<&str> = source.lines().collect();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim_end();
            if starts_with_keyword(line, &METHOD_STARTS) {
                let end = signature_end(&lines, i);
                out.extend(lines[i..=end].iter().map(|l| l.trim_end().to_string()));
                // Skip to the matching end keyword; the body becomes one marker.
                let mut j = end + 1;
                // An unterminated method ends where the next one starts.
                while j < lines.len()
                    && !starts_with_keyword(lines[j], &METHOD_ENDS)
                    && !starts_with_keyword(lines[j], &METHOD_STARTS)
                {
                    j += 1;
                }
                let terminated = j < lines.len() && starts_with_keyword(lines[j], &METHOD_ENDS);
                if j > end + 1 {
                    out.push(format!("    {ELIDED_BODY}"));
                }
                if terminated {
                    out.push(lines[j].trim_end().to_string());
                    i = j + 1;
                } else {
                    i = j;
                }
                continue;
            }
            if !line.trim().is_empty() || out.last().is_some_and(|l| !l.is_empty()) {
                out.push(line.to_string());
            }
            i += 1;
        }
        out.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_bom_async_and_unterminated_methods() {
        let a = BslAdapter;
        let src = "\u{feff}Асинх Процедура Первая() Экспорт\n    X = 1;\nКонецПроцедуры\nAsync Function Second(Val S = \"a)\") Export\n    Return 1;\nEndFunction\n";
        assert_eq!(
            a.extract_symbols(src),
            vec![
                "Асинх Процедура Первая() Экспорт".to_string(),
                "Async Function Second(Val S = \"a)\") Export".to_string()
            ]
        );
        let broken =
            "Procedure Broken()\n    X = 1;\nProcedure Next() Export\n    Y = 2;\nEndProcedure\n";
        let c = a.compress_source(broken);
        assert!(
            c.contains("Procedure Next() Export") && c.contains("EndProcedure"),
            "{c}"
        );
        assert!(!c.contains("Y = 2"), "{c}");
    }
}
