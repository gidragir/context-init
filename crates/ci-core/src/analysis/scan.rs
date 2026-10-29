//! Phase one of the two-phase `init` (D9): a cheap, LLM-free survey of the tree.

use crate::analysis::detector::count_tokens;
use crate::analysis::scope::ScopeFilter;
use crate::config::ScopeConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Size of one top-level directory or one file extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanStat {
    pub name: String,
    pub files: usize,
    pub lines: usize,
    pub tokens: usize,
    /// Share of the project's tokens, in percent.
    pub share_percent: f64,
}

/// Result of [`scan_project`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanReport {
    pub total_files: usize,
    pub total_lines: usize,
    pub total_tokens: usize,
    pub directories: Vec<ScanStat>,
    pub extensions: Vec<ScanStat>,
    pub suggested_scope: ScopeConfig,
}

/// Files above this size are estimated at 4 bytes per token instead of being tokenized.
const TOKENIZE_MAX_BYTES: usize = 512 * 1024;

#[derive(Default)]
struct Tally {
    files: usize,
    lines: usize,
    tokens: usize,
}

fn stats(map: BTreeMap<String, Tally>, total_tokens: usize) -> Vec<ScanStat> {
    let mut out: Vec<ScanStat> = map
        .into_iter()
        .map(|(name, t)| ScanStat {
            name,
            files: t.files,
            lines: t.lines,
            tokens: t.tokens,
            share_percent: if total_tokens == 0 {
                0.0
            } else {
                t.tokens as f64 * 100.0 / total_tokens as f64
            },
        })
        .collect();
    out.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.name.cmp(&b.name)));
    out
}

/// Surveys `root` honoring `.gitignore` and `scope`: files, lines and BPE tokens per top-level
/// directory and per extension, plus a suggested `[scope]`.
pub fn scan_project(root: &Path, scope: &ScopeConfig) -> Result<ScanReport> {
    let filter = ScopeFilter::build(root, scope)?;
    let mut dirs: BTreeMap<String, Tally> = BTreeMap::new();
    let mut exts: BTreeMap<String, Tally> = BTreeMap::new();
    let (mut total_files, mut total_lines, mut total_tokens) = (0, 0, 0);

    for rel in filter.files() {
        let bytes = fs::read(root.join(rel)).unwrap_or_default();
        let (lines, tokens) = match std::str::from_utf8(&bytes) {
            Ok(text) if bytes.len() <= TOKENIZE_MAX_BYTES => {
                (text.lines().count(), count_tokens(text))
            }
            Ok(text) => (text.lines().count(), bytes.len() / 4),
            Err(_) => (0, 0),
        };
        let dir = rel.split_once('/').map_or(".", |(d, _)| d).to_string();
        let ext = Path::new(rel)
            .extension()
            .and_then(|e| e.to_str())
            .map_or_else(|| "none".to_string(), str::to_ascii_lowercase);
        for (map, key) in [(&mut dirs, dir), (&mut exts, ext)] {
            let t = map.entry(key).or_default();
            t.files += 1;
            t.lines += lines;
            t.tokens += tokens;
        }
        total_files += 1;
        total_lines += lines;
        total_tokens += tokens;
    }

    let extensions = stats(exts, total_tokens);
    let mut suggested: Vec<String> = extensions
        .iter()
        .filter(|e| e.tokens > 0 && e.share_percent >= 1.0)
        .map(|e| e.name.clone())
        .collect();
    suggested.sort();
    Ok(ScanReport {
        total_files,
        total_lines,
        total_tokens,
        directories: stats(dirs, total_tokens),
        extensions,
        suggested_scope: ScopeConfig {
            extensions: suggested,
            ..scope.clone()
        },
    })
}

/// Markdown report of [`scan_project`].
pub fn render_scan_markdown(project_id: &str, root: &Path, report: &ScanReport) -> String {
    let mut md = format!(
        "# Project Scan: {project_id}\n\n- **Root:** `{}`\n- **Total Files:** {}\n- **Total Lines:** {}\n- **Total Tokens:** {}\n",
        root.display(),
        report.total_files,
        report.total_lines,
        report.total_tokens
    );
    md.push_str("\n## Directories\n");
    for d in &report.directories {
        md.push_str(&format!(
            "- `{}/`: {} files, {} lines, ~{} tokens ({:.1}%)\n",
            d.name, d.files, d.lines, d.tokens, d.share_percent
        ));
    }
    md.push_str("\n## Extensions Breakdown\n");
    for e in &report.extensions {
        md.push_str(&format!(
            "- `.{}`: {} files, {} lines, ~{} tokens ({:.1}%)\n",
            e.name, e.files, e.lines, e.tokens, e.share_percent
        ));
    }
    let s = &report.suggested_scope;
    md.push_str("\n## Suggested scope\n```toml\n[scope]\n");
    for (key, list) in [
        ("include", &s.include),
        ("exclude", &s.exclude),
        ("extensions", &s.extensions),
    ] {
        let items: Vec<String> = list.iter().map(|v| format!("\"{v}\"")).collect();
        md.push_str(&format!("{key} = [{}]\n", items.join(", ")));
    }
    md.push_str("```\n");
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("scan_{tag}_{}_{nanos}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    #[test]
    fn scan_respects_gitignore_and_computes_token_shares() {
        let root = temp_root("shares");
        write(&root, ".gitignore", "build/\nvendor/\n");
        write(
            &root,
            "src/main.rs",
            "fn main() {\n    println!(\"hi\");\n}\n",
        );
        write(&root, "src/util.rs", "pub fn util() -> u32 {\n    1\n}\n");
        write(&root, "docs/guide.md", "# Guide\n\nSome text here.\n");
        write(&root, "build/out.rs", "pub fn generated() {}\n");
        write(&root, "vendor/dep.rs", "pub fn dep() {}\n");

        let report = scan_project(&root, &ScopeConfig::default()).unwrap();
        let dir_names: Vec<&str> = report.directories.iter().map(|d| d.name.as_str()).collect();
        assert!(
            dir_names.contains(&"src") && dir_names.contains(&"docs"),
            "{dir_names:?}"
        );
        assert!(
            !dir_names.contains(&"build") && !dir_names.contains(&"vendor"),
            "{dir_names:?}"
        );
        assert!(report.total_files >= 3 && report.total_lines > 0 && report.total_tokens > 0);
        let rs = report.extensions.iter().find(|e| e.name == "rs").unwrap();
        assert_eq!(rs.files, 2, "{report:?}");
        for list in [&report.directories, &report.extensions] {
            let sum: f64 = list.iter().map(|s| s.share_percent).sum();
            assert!((sum - 100.0).abs() < 0.5, "shares sum to {sum}");
        }
        assert!(report
            .suggested_scope
            .extensions
            .contains(&"rs".to_string()));
        let md = render_scan_markdown("p", &root, &report);
        assert!(md.contains("Total Files") && md.contains("`.rs`"), "{md}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_applies_scope_and_skips_symlinks_out_of_root() {
        let root = temp_root("scope");
        let outside = temp_root("outside");
        write(&outside, "secret.rs", "pub fn secret() {}\n");
        write(&root, "crates/a/lib.rs", "pub fn a() {}\n");
        write(&root, "crates/b/lib.rs", "pub fn b() {}\n");
        write(&root, "crates/a/tests/t.rs", "fn t() {}\n");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        let scope = ScopeConfig {
            include: vec!["crates/a/**".into()],
            exclude: vec!["**/tests/**".into()],
            extensions: vec!["rs".into()],
        };
        let report = scan_project(&root, &scope).unwrap();
        assert_eq!(report.total_files, 1, "{report:?}");
        let all = scan_project(&root, &ScopeConfig::default()).unwrap();
        assert_eq!(
            all.total_files, 3,
            "symlink target must not be walked: {all:?}"
        );
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }
}
