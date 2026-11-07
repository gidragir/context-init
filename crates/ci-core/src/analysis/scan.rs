//! Phase one of the two-phase `init` (D9): a cheap, LLM-free survey of the tree.

use crate::analysis::detector::count_tokens;
use crate::analysis::file_kind::canonical_file_ext;
use crate::analysis::scope::ScopeFilter;
use crate::config::ScopeConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;

pub const KNOWN_BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "msi", "cab", "zip", "tar", "gz", "7z", "rar", "png", "jpg",
    "jpeg", "gif", "webp", "ico", "icns", "pdf", "woff", "woff2", "ttf", "otf", "eot", "mp3",
    "mp4", "wav", "ogg", "wasm", "pyc", "class", "jar", "db", "sqlite", "sqlite3", "cf", "epf",
    "erf", "bin", "pack", "idx",
];

/// Returns `true` if a file is binary by extension, contains NUL bytes, or has non-UTF-8 content.
pub fn is_likely_binary(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_ascii_lowercase();
        if KNOWN_BINARY_EXTENSIONS.contains(&ext_lower.as_str()) {
            return true;
        }
    }

    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 8192];
    let Ok(n) = file.read(&mut buf) else {
        return false;
    };
    if n == 0 {
        return false;
    }
    let slice = &buf[..n];
    if slice.contains(&0x00) {
        return true;
    }
    if let Err(err) = std::str::from_utf8(slice) {
        if err.error_len().is_some() {
            return true;
        }
    }
    false
}

/// Options for [`scan_project_with`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanOptions {
    /// Directory aggregation depth (1..=5, default: 2).
    pub max_depth: usize,
    /// Diagnostic mode: include hidden dot-directories (except runtime caches and .git).
    pub include_hidden: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            max_depth: 2,
            include_hidden: false,
        }
    }
}

/// A context bomb outlier file consuming an anomalous token share.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanOutlier {
    pub path: String,
    pub lines: usize,
    pub tokens: usize,
    pub share_percent: f64,
}

/// Size of one directory or one file extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanStat {
    pub name: String,
    pub files: usize,
    pub lines: usize,
    pub tokens: usize,
    /// Share of the project's tokens, in percent.
    pub share_percent: f64,
}

/// Result of [`scan_project`] and [`scan_project_with`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanReport {
    pub total_files: usize,
    pub total_lines: usize,
    pub total_tokens: usize,
    pub directories: Vec<ScanStat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subdirectories: Vec<ScanStat>,
    pub extensions: Vec<ScanStat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outliers: Vec<ScanOutlier>,
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

/// Surveys `root` honoring `.gitignore` and `scope` with default options.
pub fn scan_project(root: &Path, scope: &ScopeConfig) -> Result<ScanReport> {
    scan_project_with(root, scope, &ScanOptions::default())
}

/// Surveys `root` honoring `.gitignore` and `scope` with explicit options.
pub fn scan_project_with(
    root: &Path,
    scope: &ScopeConfig,
    opts: &ScanOptions,
) -> Result<ScanReport> {
    let max_depth = opts.max_depth.clamp(1, 5);
    let filter = ScopeFilter::build_diagnostic(root, scope, opts.include_hidden)?;
    let mut dirs_depth1: BTreeMap<String, Tally> = BTreeMap::new();
    let mut subdirs_depth_n: BTreeMap<String, Tally> = BTreeMap::new();
    let mut exts: BTreeMap<String, Tally> = BTreeMap::new();
    let (mut total_files, mut total_lines, mut total_tokens) = (0, 0, 0);

    struct ScannedFile {
        rel: String,
        lines: usize,
        tokens: usize,
        ext: String,
    }

    let mut scanned_files: Vec<ScannedFile> = Vec::new();

    for rel in filter.files() {
        let path = root.join(rel);
        let size = fs::metadata(&path).map_or(0, |m| m.len() as usize);
        let is_bin = is_likely_binary(&path);
        let (lines, tokens) = if is_bin {
            (0, 0)
        } else if size > TOKENIZE_MAX_BYTES {
            (0, size / 4)
        } else {
            match fs::read_to_string(&path) {
                Ok(text) => (text.lines().count(), count_tokens(&text)),
                Err(_) => (0, 0),
            }
        };

        let ext = canonical_file_ext(rel).unwrap_or_else(|| "none".to_string());
        let top_dir = rel.split_once('/').map_or(".", |(d, _)| d).to_string();

        let t_top = dirs_depth1.entry(top_dir).or_default();
        t_top.files += 1;
        t_top.lines += lines;
        t_top.tokens += tokens;

        // Subdirectories depth 2..=max_depth
        let segments: Vec<&str> = rel.split('/').collect();
        if segments.len() > 1 {
            for d in 2..=max_depth {
                if segments.len() >= d {
                    let sub_path = segments[..d].join("/");
                    let t_sub = subdirs_depth_n.entry(sub_path).or_default();
                    t_sub.files += 1;
                    t_sub.lines += lines;
                    t_sub.tokens += tokens;
                }
            }
        }

        let t_ext = exts.entry(ext.clone()).or_default();
        t_ext.files += 1;
        t_ext.lines += lines;
        t_ext.tokens += tokens;

        total_files += 1;
        total_lines += lines;
        total_tokens += tokens;

        scanned_files.push(ScannedFile {
            rel: rel.clone(),
            lines,
            tokens,
            ext,
        });
    }

    // Identify outliers (Context Bombs)
    // Criteria: tokens >= 10_000 && (tokens >= 25_000 || (total_files >= 8 && share_percent >= 15.0))
    let mut outliers: Vec<ScanOutlier> = Vec::new();
    for f in &scanned_files {
        let share_percent = if total_tokens == 0 {
            0.0
        } else {
            f.tokens as f64 * 100.0 / total_tokens as f64
        };
        if f.tokens >= 10_000 && (f.tokens >= 25_000 || (total_files >= 8 && share_percent >= 15.0))
        {
            outliers.push(ScanOutlier {
                path: f.rel.clone(),
                lines: f.lines,
                tokens: f.tokens,
                share_percent,
            });
        }
    }
    outliers.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.path.cmp(&b.path)));
    outliers.truncate(10);

    let outlier_paths: HashSet<&str> = outliers.iter().map(|o| o.path.as_str()).collect();

    // Suggested scope calculation:
    // Strictly computed on files passing standard ScopeFilter (without include_hidden)
    let standard_filter = ScopeFilter::build(root, scope)?;
    let standard_set: HashSet<&str> = standard_filter.files().iter().map(|s| s.as_str()).collect();

    let mut clean_exts: BTreeMap<String, Tally> = BTreeMap::new();
    let mut clean_total_tokens = 0;

    for f in &scanned_files {
        if standard_set.contains(f.rel.as_str()) && !outlier_paths.contains(f.rel.as_str()) {
            clean_total_tokens += f.tokens;
            let t = clean_exts.entry(f.ext.clone()).or_default();
            t.files += 1;
            t.lines += f.lines;
            t.tokens += f.tokens;
        }
    }

    let mut suggested_exts: Vec<String> = Vec::new();
    if clean_total_tokens > 0 {
        for (ext_name, t) in clean_exts {
            if ext_name == "none" {
                continue;
            }
            let share = t.tokens as f64 * 100.0 / clean_total_tokens as f64;
            if share >= 1.0 || (t.files >= 3 && t.tokens > 0) {
                suggested_exts.push(ext_name);
            }
        }
        suggested_exts.sort();
    }

    // Suggested excludes for outliers:
    // Subdirectories of depth 1..=3 where >= 80% tokens are outliers and no root build manifests
    let mut suggested_exclude = scope.exclude.clone();
    let mut handled_outliers: HashSet<String> = HashSet::new();

    const ROOT_MANIFESTS: &[&str] = &[
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "pyproject.toml",
    ];

    for o in &outliers {
        let segs: Vec<&str> = o.path.split('/').collect();
        let mut grouped = false;
        if segs.len() > 1 {
            for d in (1..=3.min(segs.len() - 1)).rev() {
                let dir_candidate = segs[..d].join("/");
                let dir_total_tokens: usize = scanned_files
                    .iter()
                    .filter(|f| f.rel.starts_with(&format!("{dir_candidate}/")))
                    .map(|f| f.tokens)
                    .sum();
                let dir_outlier_tokens: usize = outliers
                    .iter()
                    .filter(|f| f.path.starts_with(&format!("{dir_candidate}/")))
                    .map(|f| f.tokens)
                    .sum();
                let has_manifest = ROOT_MANIFESTS
                    .iter()
                    .any(|m| root.join(&dir_candidate).join(m).is_file());
                if dir_total_tokens > 0 && !has_manifest {
                    let outlier_share = dir_outlier_tokens as f64 / dir_total_tokens as f64;
                    if outlier_share >= 0.8 {
                        let excl = format!("{dir_candidate}/**");
                        if !suggested_exclude.contains(&excl) {
                            suggested_exclude.push(excl);
                        }
                        for other in &outliers {
                            if other.path.starts_with(&format!("{dir_candidate}/")) {
                                handled_outliers.insert(other.path.clone());
                            }
                        }
                        grouped = true;
                        break;
                    }
                }
            }
        }
        if !grouped && !handled_outliers.contains(&o.path) {
            if !suggested_exclude.contains(&o.path) {
                suggested_exclude.push(o.path.clone());
            }
            handled_outliers.insert(o.path.clone());
        }
    }
    suggested_exclude.sort();
    suggested_exclude.dedup();

    let extensions = stats(exts, total_tokens);
    let directories = stats(dirs_depth1, total_tokens);

    let mut subdirectories = stats(subdirs_depth_n, total_tokens);
    subdirectories.retain(|s| s.share_percent >= 2.0);
    subdirectories.truncate(20);

    Ok(ScanReport {
        total_files,
        total_lines,
        total_tokens,
        directories,
        subdirectories,
        extensions,
        outliers,
        suggested_scope: ScopeConfig {
            extensions: suggested_exts,
            exclude: suggested_exclude,
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
    if !report.subdirectories.is_empty() {
        md.push_str("\n### Significant Subdirectories\n");
        for sd in &report.subdirectories {
            md.push_str(&format!(
                "- `{}/`: {} files, {} lines, ~{} tokens ({:.1}%)\n",
                sd.name, sd.files, sd.lines, sd.tokens, sd.share_percent
            ));
        }
    }
    if !report.outliers.is_empty() {
        md.push_str("\n## Outliers (Context Bombs)\n");
        for o in &report.outliers {
            md.push_str(&format!(
                "- `{}`: {} lines, ~{} tokens ({:.1}%)\n",
                o.path, o.lines, o.tokens, o.share_percent
            ));
        }
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

    #[test]
    fn scan_project_assigns_zero_tokens_to_large_binaries_and_preserves_cyrillic_utf8() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("installer")).unwrap();
        // 600 KB binary file (> TOKENIZE_MAX_BYTES = 512 KB) with NUL bytes
        let mut bin = vec![0u8; 600 * 1024];
        bin[0..4].copy_from_slice(b"MZ\x90\x00");
        std::fs::write(root.join("installer/setup.exe"), &bin).unwrap();

        // Cyrillic UTF-8 file crossing 8192-byte boundary on a 2-byte sequence
        std::fs::create_dir_all(root.join("src")).unwrap();
        let cyrillic = "Процедура Тест()\n".repeat(600); // > 8192 bytes
        std::fs::write(root.join("src/Module.bsl"), &cyrillic).unwrap();

        let report = scan_project(root, &ScopeConfig::default()).unwrap();
        let inst = report
            .directories
            .iter()
            .find(|d| d.name == "installer")
            .unwrap();
        assert_eq!(inst.tokens, 0);
        let src = report.directories.iter().find(|d| d.name == "src").unwrap();
        assert!(src.tokens > 500);
    }

    #[test]
    fn scan_detects_outliers_excludes_context_bombs_and_keeps_code_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("vendor_dump")).unwrap();
        // 2 files in vendor_dump with ~30_000 tokens each (total 60_000)
        let large_cpp = "int compute_heavy() { return 42; }\n".repeat(3500);
        std::fs::write(root.join("vendor_dump/a.cpp"), &large_cpp).unwrap();
        std::fs::write(root.join("vendor_dump/b.cpp"), &large_cpp).unwrap();

        // 10 small shell scripts of ~80 tokens each in scripts/
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        for i in 0..10 {
            let sh_code = format!("#!/bin/bash\necho 'step {}'\n", i).repeat(10);
            std::fs::write(root.join(format!("scripts/s{i}.sh")), sh_code).unwrap();
        }

        // Dockerfile.v1.2 with ~200 tokens
        std::fs::write(
            root.join("Dockerfile.v1.2"),
            "FROM ubuntu:24.04\nRUN apt update\n".repeat(15),
        )
        .unwrap();

        // templates/app.toml.tmpl (3 files with ~300 tokens)
        std::fs::create_dir_all(root.join("templates")).unwrap();
        for i in 0..3 {
            std::fs::write(
                root.join(format!("templates/app{i}.toml.tmpl")),
                "key = \"val\"\n".repeat(30),
            )
            .unwrap();
        }

        let opts = ScanOptions {
            max_depth: 2,
            include_hidden: false,
        };
        let report = scan_project_with(root, &ScopeConfig::default(), &opts).unwrap();

        assert_eq!(report.outliers.len(), 2, "Outliers: {:?}", report.outliers);
        assert!(
            report
                .suggested_scope
                .exclude
                .iter()
                .any(|e| e == "vendor_dump/**"),
            "vendor_dump/** must be excluded: {:?}",
            report.suggested_scope.exclude
        );
        assert!(
            report
                .suggested_scope
                .extensions
                .contains(&"sh".to_string()),
            "extensions should contain 'sh': {:?}",
            report.suggested_scope.extensions
        );
        assert!(
            report
                .suggested_scope
                .extensions
                .contains(&"toml.tmpl".to_string()),
            "extensions should contain 'toml.tmpl': {:?}",
            report.suggested_scope.extensions
        );
        assert!(
            !report.suggested_scope.extensions.contains(&"2".to_string()),
            "extensions should not have '2' from Dockerfile.v1.2: {:?}",
            report.suggested_scope.extensions
        );
        assert!(
            !report
                .suggested_scope
                .extensions
                .contains(&"cpp".to_string()),
            "extensions should not be polluted by outlier 'cpp': {:?}",
            report.suggested_scope.extensions
        );

        // Verifying that suggested_scope cuts vendor_dump and retains code
        let filtered = ScopeFilter::build(root, &report.suggested_scope).unwrap();
        assert!(!filtered
            .files()
            .iter()
            .any(|f| f.starts_with("vendor_dump")));
        assert!(filtered
            .files()
            .iter()
            .any(|f| f.starts_with("scripts/s0.sh")));
        assert!(filtered
            .files()
            .iter()
            .any(|f| f.starts_with("templates/app0.toml.tmpl")));
    }

    #[test]
    fn scan_include_hidden_shows_dot_dirs_without_polluting_suggested_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".agents")).unwrap();
        std::fs::write(root.join(".agents/guide.md"), "# Agent guide\n".repeat(50)).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n".repeat(50)).unwrap();

        let opts_hidden = ScanOptions {
            max_depth: 2,
            include_hidden: true,
        };
        let report = scan_project_with(root, &ScopeConfig::default(), &opts_hidden).unwrap();
        assert!(
            report.directories.iter().any(|d| d.name == ".agents"),
            "Hidden .agents directory should be present in diagnostic scan: {:?}",
            report.directories
        );
        assert!(
            !report
                .suggested_scope
                .extensions
                .contains(&"md".to_string()),
            "Diagnostic hidden files must not pollute suggested_scope: {:?}",
            report.suggested_scope.extensions
        );
    }

    #[test]
    fn scan_all_outliers_does_not_produce_nan_shares() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("dump")).unwrap();
        let huge = "int main() {}\n".repeat(8000);
        std::fs::write(root.join("dump/massive.cpp"), &huge).unwrap();

        let report =
            scan_project_with(root, &ScopeConfig::default(), &ScanOptions::default()).unwrap();
        assert_eq!(report.outliers.len(), 1);
        assert!(report.suggested_scope.extensions.is_empty());
        let json = serde_json::to_string(&report)
            .expect("report must serialize to valid JSON without NaN/Inf");
        assert!(!json.contains("NaN") && !json.contains("null"));
    }
}
