use super::{LanguageAdapter, ELIDED_BODY};

/// BSL / OneScript modules (`.bsl`, `.os`): Cyrillic and English keywords, `&Directive`
/// annotations, CRLF line endings.
pub struct BslAdapter;

const METHOD_STARTS: [&str; 4] = ["процедура", "функция", "procedure", "function"];
const METHOD_ENDS: [&str; 4] = [
    "конецпроцедуры",
    "конецфункции",
    "endprocedure",
    "endfunction",
];
const EXPORT: [&str; 2] = ["экспорт", "export"];

/// `line` starts with `keyword` followed by a non-identifier character (case-insensitive).
fn starts_with_keyword(line: &str, keywords: &[&str]) -> bool {
    let lower = line.trim_start().to_lowercase();
    keywords.iter().any(|kw| {
        lower.strip_prefix(kw).is_some_and(|rest| {
            rest.chars()
                .next()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
        })
    })
}

/// Index of the line closing the parameter list that starts at `start`.
fn signature_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0i32;
    for (i, line) in lines.iter().enumerate().skip(start) {
        let code = line.split("//").next().unwrap_or("");
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
        .map(|l| l.split("//").next().unwrap_or("").trim())
        .collect::<Vec<_>>()
        .join(" ")
        .replace("( ", "(")
}

fn has_export(signature: &str) -> bool {
    let tail = signature
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
                while j < lines.len() && !starts_with_keyword(lines[j], &METHOD_ENDS) {
                    j += 1;
                }
                if j > end + 1 {
                    out.push(format!("    {ELIDED_BODY}"));
                }
                if j < lines.len() {
                    out.push(lines[j].trim_end().to_string());
                }
                i = j + 1;
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
