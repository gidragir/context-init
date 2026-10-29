//! Task context digest (D4, D5): what changed since a base ref, in two levels.
//! L1 is an overview (commits, documents, semantic `.mdo` diffs, changed methods);
//! L2 (`target`) is the diff hunk of one method or metadata object.

use crate::analysis::detector::count_tokens;
use crate::analysis::lang::{self, mdo, mdo::MdoObject};
use crate::git::GitBackend;
use crate::storage::L0Index;
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// Input of [`build_task_digest`].
pub struct TaskDigestRequest<'a> {
    pub paths: &'a [String],
    pub base_ref: Option<&'a str>,
    /// `None` = L1 overview digest, `Some(method_or_object)` = L2 hunk.
    pub target: Option<&'a str>,
    pub max_tokens: usize,
}

/// Cap on commits listed in an L1 digest.
const MAX_COMMITS: usize = 20;

fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn subsystem_tag(index: Option<&L0Index>, path: &str) -> String {
    index
        .and_then(|idx| {
            idx.subsystems.iter().find(|s| {
                s.files.iter().any(|sf| sf.path == path) || s.key_files.iter().any(|kf| kf == path)
            })
        })
        .map(|s| format!(" (subsystem: `{}`)", s.id))
        .unwrap_or_default()
}

/// Methods keyed by `(name, occurrence)`, so same-named methods of different impls stay apart.
fn method_map(ext: &str, source: &str) -> BTreeMap<(String, usize), String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for (name, body) in lang::adapter_for_ext(ext)
        .map(|a| a.method_bodies(source))
        .unwrap_or_default()
    {
        let n = seen.entry(name.clone()).or_insert(0);
        out.insert((name, *n), body);
        *n += 1;
    }
    out
}

/// Names of methods added, changed (`name`) or removed (`name (removed)`) between two versions.
fn changed_methods(ext: &str, old: Option<&str>, new: Option<&str>) -> Vec<String> {
    let old = method_map(ext, old.unwrap_or(""));
    let new = method_map(ext, new.unwrap_or(""));
    let mut out: Vec<String> = Vec::new();
    for (name, _) in new
        .iter()
        .filter(|(k, body)| old.get(*k) != Some(body))
        .map(|(k, v)| (&k.0, v))
    {
        if !out.contains(name) {
            out.push(name.clone());
        }
    }
    for name in old.keys().filter(|k| !new.contains_key(*k)).map(|k| &k.0) {
        let label = format!("{name} (removed)");
        if !out.contains(&label) && !out.contains(name) {
            out.push(label);
        }
    }
    out
}

/// Raw source text of one method for display: BSL / Python from their adapters, Rust by
/// matching braces from the `fn <name>` line.
fn raw_method(ext: &str, source: &str, name: &str) -> Option<String> {
    if ext == "rs" {
        let lines: Vec<&str> = source.lines().collect();
        let needle = format!("fn {name}");
        let start = lines.iter().position(|l| {
            l.find(&needle).is_some_and(|i| {
                !l[i + needle.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
            })
        })?;
        let (mut depth, mut seen) = (0i32, false);
        for (i, line) in lines.iter().enumerate().skip(start) {
            for c in line.chars() {
                match c {
                    '{' => {
                        depth += 1;
                        seen = true;
                    }
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if seen && depth <= 0 {
                return Some(lines[start..=i].join("\n"));
            }
        }
        return Some(lines[start..].join("\n"));
    }
    method_map(ext, source).remove(&(name.to_string(), 0))
}

/// `-`/`+` view of two method versions with up to two context lines around the change.
fn method_hunk(old: &str, new: &str) -> String {
    let (o, n): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let prefix = o.iter().zip(&n).take_while(|(a, b)| a == b).count();
    let max_suffix = o.len().min(n.len()) - prefix;
    let suffix = o
        .iter()
        .rev()
        .zip(n.iter().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    let mut out = String::new();
    for l in &o[prefix.saturating_sub(2)..prefix] {
        out.push_str(&format!(" {l}\n"));
    }
    for l in &o[prefix..o.len() - suffix] {
        out.push_str(&format!("-{l}\n"));
    }
    for l in &n[prefix..n.len() - suffix] {
        out.push_str(&format!("+{l}\n"));
    }
    for l in &n[n.len() - suffix..(n.len() - suffix + 2).min(n.len())] {
        out.push_str(&format!(" {l}\n"));
    }
    out
}

fn parse_mdo(text: Option<&str>) -> Option<MdoObject> {
    text.and_then(|t| mdo::parse_mdo_xml(t).ok())
}

fn fit_to_budget(text: String, max_tokens: usize) -> String {
    if max_tokens == 0 || count_tokens(&text) <= max_tokens {
        return text;
    }
    let note = "\n… (truncated; narrow down with `target`)\n";
    let reserve = count_tokens(note);
    if max_tokens <= reserve {
        return "…".chars().take(max_tokens).collect();
    }
    // Per-line counts are summed (an upper bound), so the result stays within the budget
    // without re-tokenizing the accumulated text for every line.
    let mut kept = String::new();
    let mut used = 0;
    for line in text.lines() {
        used += count_tokens(line) + 1;
        if used + reserve > max_tokens {
            break;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    format!("{kept}{note}")
}

/// Rejects refs that git would parse as options or that carry control characters.
fn validate_ref(kind: &str, value: &str) -> Result<()> {
    let v = value.trim();
    if v.is_empty() || v.starts_with('-') || v.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        bail!("Invalid {kind} '{value}'");
    }
    Ok(())
}

/// Builds the digest of the changes between `base_ref` (default `HEAD~1`, resolved through
/// `merge-base` with `HEAD`) and `HEAD`, within `req.max_tokens`.
pub fn build_task_digest(
    root: &Path,
    git: &dyn GitBackend,
    index: Option<&L0Index>,
    req: &TaskDigestRequest,
) -> Result<String> {
    let base_ref = req.base_ref.unwrap_or("HEAD~1");
    validate_ref("base ref", base_ref)?;
    if req.target.is_some_and(|t| t.trim().is_empty()) {
        bail!("target must not be empty");
    }
    let head = git.head_commit(root).unwrap_or_default();
    let base = git
        .merge_base(root, base_ref, "HEAD")
        .unwrap_or_else(|_| base_ref.to_string());
    let changed = git.diff_name_only(root, &base, &head).unwrap_or_default();
    let show = |rev: &str, path: &str| git.file_at(root, rev, path).ok().flatten();

    if let Some(target) = req.target {
        let wanted = target.trim().to_ascii_lowercase();
        let mut out = String::new();
        for path in &changed {
            let ext = ext_of(path);
            let (old, new) = (show(&base, path), show(&head, path));
            if ext == "mdo" {
                let (o, n) = (parse_mdo(old.as_deref()), parse_mdo(new.as_deref()));
                if let Some(obj) = n.as_ref().or(o.as_ref()) {
                    if obj.name.to_ascii_lowercase() == wanted || obj.id() == wanted {
                        out.push_str(&format!("## {}.{} (`{path}`)\n", obj.kind, obj.name));
                        for l in mdo::semantic_diff(o.as_ref(), obj) {
                            out.push_str(&format!("{l}\n"));
                        }
                    }
                }
                continue;
            }
            if lang::adapter_for_ext(&ext).is_none() {
                continue;
            }
            let (old_src, new_src) = (old.unwrap_or_default(), new.unwrap_or_default());
            let (o, n) = (
                raw_method(&ext, &old_src, target.trim()),
                raw_method(&ext, &new_src, target.trim()),
            );
            if o == n {
                continue;
            }
            out.push_str(&format!(
                "## {} (`{path}`)\n```diff\n{}```\n",
                target.trim(),
                method_hunk(o.as_deref().unwrap_or(""), n.as_deref().unwrap_or(""))
            ));
        }
        if out.is_empty() {
            out = format!(
                "No changed method or object matching `{}` since `{base_ref}`.\n",
                target.trim()
            );
        }
        return Ok(fit_to_budget(out, req.max_tokens));
    }

    let mut out = String::new();
    if head.is_empty() {
        out.push_str("_Not a Git repository (or no commits yet): no diff available._\n\n");
    }
    out.push_str(&format!(
        "# Task Context Digest\n\n- **Base Ref:** `{base_ref}` (merge-base `{}`)\n- **HEAD:** `{head}`\n- **Modified Files:** {}\n\n",
        base.chars().take(12).collect::<String>(),
        changed.len()
    ));
    let commits = git
        .log_oneline(root, &base, &head, MAX_COMMITS)
        .unwrap_or_default();
    if !commits.is_empty() {
        out.push_str("## Commits\n");
        for c in &commits {
            out.push_str(&format!("- {c}\n"));
        }
        out.push('\n');
    }
    if !changed.is_empty() {
        out.push_str("## Modified Files (Git Diff)\n");
        for f in &changed {
            out.push_str(&format!("- `{f}`{}\n", subsystem_tag(index, f)));
        }
        out.push('\n');
    }
    let docs: Vec<&String> = changed.iter().filter(|f| ext_of(f) == "md").collect();
    if !docs.is_empty() {
        out.push_str("## Documents (Markdown)\n");
        for d in docs {
            out.push_str(&format!("- `{d}`\n"));
        }
        out.push('\n');
    }
    let mut mdo_section = String::new();
    let mut method_section = String::new();
    for path in &changed {
        let ext = ext_of(path);
        if ext == "mdo" {
            let (o, n) = (
                parse_mdo(show(&base, path).as_deref()),
                parse_mdo(show(&head, path).as_deref()),
            );
            if let Some(obj) = n.as_ref().or(o.as_ref()) {
                mdo_section.push_str(&format!("### {}.{} (`{path}`)\n", obj.kind, obj.name));
                for l in mdo::semantic_diff(o.as_ref(), obj) {
                    mdo_section.push_str(&format!("{l}\n"));
                }
            }
        } else if lang::adapter_for_ext(&ext).is_some() {
            let names = changed_methods(
                &ext,
                show(&base, path).as_deref(),
                show(&head, path).as_deref(),
            );
            if !names.is_empty() {
                method_section.push_str(&format!("- `{path}`: {}\n", names.join(", ")));
            }
        }
    }
    if !mdo_section.is_empty() {
        out.push_str(&format!("## Metadata Changes (.mdo)\n{mdo_section}\n"));
    }
    if !method_section.is_empty() {
        out.push_str(&format!("## Changed Methods\n{method_section}\n"));
    }
    if !req.paths.is_empty() {
        out.push_str("## Task-Specified Paths\n");
        for p in req.paths {
            out.push_str(&format!("- `{p}`{}\n", subsystem_tag(index, p)));
        }
        out.push('\n');
    }
    Ok(fit_to_budget(out, req.max_tokens))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::SystemGitBackend;
    use std::fs;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    const CATALOG_V1: &str = "<m:Catalog xmlns:m=\"urn:x\"><name>Items</name><attributes><name>Code</name></attributes></m:Catalog>";
    const CATALOG_V2: &str = "<m:Catalog xmlns:m=\"urn:x\"><name>Items</name><attributes><name>Code</name></attributes><attributes><name>Status</name></attributes></m:Catalog>";

    fn repo() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("task_{}_{nanos}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        write(&root, "docs/spec.md", "# Spec\n\nv1\n");
        write(&root, "src/Catalogs/Items/Items.mdo", CATALOG_V1);
        write(
            &root,
            "src/orders.rs",
            "pub fn process_order(x: u32) -> u32 {\n    x + 1\n}\n\npub fn other_fn(x: u32) -> u32 {\n    x * 2\n}\n\npub fn stable(x: u32) -> u32 {\n    x\n}\n",
        );
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "base"]);
        git(&root, &["checkout", "-q", "-b", "feature"]);
        write(&root, "docs/spec.md", "# Spec\n\nv2 with more\n");
        write(&root, "src/Catalogs/Items/Items.mdo", CATALOG_V2);
        write(
            &root,
            "src/orders.rs",
            "pub fn process_order(x: u32) -> u32 {\n    let y = x + 10;\n    y\n}\n\npub fn other_fn(x: u32) -> u32 {\n    x * 3\n}\n\npub fn stable(x: u32) -> u32 {\n    x\n}\n",
        );
        git(&root, &["add", "-A"]);
        git(
            &root,
            &["commit", "-q", "-m", "feat: rework orders and spec"],
        );
        root
    }

    #[test]
    fn task_digest_l1_includes_commits_md_artifacts_mdo_semantic_diff_and_changed_methods() {
        let root = repo();
        let digest = build_task_digest(
            &root,
            &SystemGitBackend::new(),
            None,
            &TaskDigestRequest {
                paths: &[],
                base_ref: Some("main"),
                target: None,
                max_tokens: 2000,
            },
        )
        .unwrap();
        for needle in [
            "feat: rework orders and spec",
            "docs/spec.md",
            "+Attribute: Status",
            "process_order",
            "other_fn",
        ] {
            assert!(digest.contains(needle), "{needle} missing in:\n{digest}");
        }
        assert!(
            !digest.contains("stable"),
            "unchanged method listed:\n{digest}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn task_digest_l2_returns_specific_method_or_object_hunk() {
        let root = repo();
        let git_backend = SystemGitBackend::new();
        let req = |target| TaskDigestRequest {
            paths: &[],
            base_ref: Some("main"),
            target: Some(target),
            max_tokens: 2000,
        };
        let hunk = build_task_digest(&root, &git_backend, None, &req("process_order")).unwrap();
        assert!(hunk.contains("let y = x + 10"), "{hunk}");
        assert!(!hunk.contains("x * 3"), "other method leaked:\n{hunk}");
        let object = build_task_digest(&root, &git_backend, None, &req("Items")).unwrap();
        assert!(object.contains("+Attribute: Status"), "{object}");
        let missing = build_task_digest(&root, &git_backend, None, &req("nope")).unwrap();
        assert!(missing.contains("No changed method or object"), "{missing}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn digest_rejects_option_like_refs_and_non_repos() {
        let root = repo();
        let out = std::env::temp_dir().join(format!("pwned_{}", std::process::id()));
        let evil = format!("--output={}", out.display());
        let err = build_task_digest(
            &root,
            &SystemGitBackend::new(),
            None,
            &TaskDigestRequest {
                paths: &[],
                base_ref: Some(&evil),
                target: None,
                max_tokens: 2000,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("Invalid base ref"), "{err}");
        assert!(fs::read_dir(std::env::temp_dir())
            .unwrap()
            .flatten()
            .all(|e| !e
                .file_name()
                .to_string_lossy()
                .starts_with(&format!("pwned_{}", std::process::id()))));

        let blank = build_task_digest(
            &root,
            &SystemGitBackend::new(),
            None,
            &TaskDigestRequest {
                paths: &[],
                base_ref: Some("main"),
                target: Some("  "),
                max_tokens: 2000,
            },
        )
        .unwrap_err();
        assert!(blank.to_string().contains("target"), "{blank}");

        let plain = std::env::temp_dir().join(format!("plain_{}", std::process::id()));
        fs::create_dir_all(&plain).unwrap();
        let no_repo = build_task_digest(
            &plain,
            &SystemGitBackend::new(),
            None,
            &TaskDigestRequest {
                paths: &[],
                base_ref: None,
                target: None,
                max_tokens: 2000,
            },
        )
        .unwrap();
        assert!(no_repo.contains("Not a Git repository"), "{no_repo}");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&plain);
    }

    #[test]
    fn same_named_methods_in_different_impls_are_all_compared() {
        let old = "struct A;\nstruct B;\nimpl A { fn new() -> u32 { 1 } }\nimpl B { fn new() -> u32 { 2 } }\n";
        let new = "struct A;\nstruct B;\nimpl A { fn new() -> u32 { 1 } }\nimpl B { fn new() -> u32 { 3 } }\n";
        assert_eq!(changed_methods("rs", Some(old), Some(new)), ["new"]);
        let new2 = "struct A;\nstruct B;\nimpl A { fn new() -> u32 { 9 } }\nimpl B { fn new() -> u32 { 2 } }\n";
        assert_eq!(changed_methods("rs", Some(old), Some(new2)), ["new"]);
    }

    #[test]
    fn budget_is_a_hard_cap() {
        let text = (0..200)
            .map(|i| format!("- line number {i}\n"))
            .collect::<String>();
        let fitted = fit_to_budget(text, 5);
        assert!(count_tokens(&fitted) <= 5, "{fitted}");
    }
}
