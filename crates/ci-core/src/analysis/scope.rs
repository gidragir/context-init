//! File scope shared by `scan` and the detector (D9): `.gitignore` rules plus the `[scope]`
//! globs. A single walk decides which files and directories exist for the whole analysis, so
//! `scan`, `init`, `update` and `pack` always agree. Symlinks are never followed or listed,
//! so nothing outside the project root can leak in.

use crate::analysis::detector::IGNORED_DIRS;
use crate::analysis::file_kind::canonical_file_ext;
use crate::config::ScopeConfig;

pub const ALWAYS_IGNORED_RUNTIME_DIRS: &[&str] = &[
    ".git",
    ".context-init",
    "node_modules",
    "target",
    ".venv",
    "venv",
    ".cache",
    ".worktrees",
    ".claude/worktrees",
];
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const IGNORED_LOCK_AND_ARTIFACT_FILES: &[&str] = &[
    "Cargo.lock",
    "pnpm-lock.yaml",
    "package-lock.json",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
    "uv.lock",
    "poetry.lock",
    "Pipfile.lock",
    "composer.lock",
    "Gemfile.lock",
    "flake.lock",
    "skills-lock.json",
    "lazy-lock.json",
];

pub const IGNORED_FILE_SUFFIXES: &[&str] = &[
    ".tsbuildinfo",
    ".min.js",
    ".min.css",
    ".bundle.js",
    ".js.map",
    ".css.map",
    ".d.ts.map",
];

pub fn is_ignored_filename(name: &str) -> bool {
    IGNORED_LOCK_AND_ARTIFACT_FILES.contains(&name)
        || IGNORED_FILE_SUFFIXES.iter().any(|suf| name.ends_with(suf))
}

fn explicitly_included(rel: &str, include: &GlobSet) -> bool {
    !include.is_empty() && include.is_match(rel)
}

pub(crate) fn include_allows_dir_traversal(rel_dir: &str, patterns: &[String]) -> bool {
    let rel_dir = rel_dir.trim_matches('/');
    if rel_dir.is_empty() {
        return true;
    }
    for pat in patterns {
        let pat = pat.trim().trim_start_matches("./").trim_matches('/');
        if pat.is_empty() {
            continue;
        }
        let segments: Vec<&str> = pat.split('/').collect();
        // If the first segment contains glob wildcards (*, ?, [, {),
        // it cannot open hidden dot-directories or ignored dirs.
        if segments.is_empty()
            || segments[0].contains('*')
            || segments[0].contains('?')
            || segments[0].contains('[')
            || segments[0].contains('{')
        {
            continue;
        }

        let mut lit_segments = Vec::new();
        let mut has_wildcard_after = false;
        for &seg in &segments {
            if seg.contains('*') || seg.contains('?') || seg.contains('[') || seg.contains('{') {
                has_wildcard_after = true;
                break;
            }
            lit_segments.push(seg);
        }

        if lit_segments.is_empty() {
            continue;
        }

        let lit_prefix = lit_segments.join("/");
        if rel_dir == lit_prefix {
            return true;
        }
        if lit_prefix.starts_with(&format!("{rel_dir}/")) {
            return true;
        }
        if has_wildcard_after && rel_dir.starts_with(&format!("{lit_prefix}/")) {
            return true;
        }
    }
    false
}

fn glob_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        let pat = pat.trim().trim_start_matches("./");
        let glob = Glob::new(pat).with_context(|| format!("Invalid scope glob '{pat}'"))?;
        builder.add(glob);
    }
    Ok(builder.build()?)
}

/// Fails when a `[scope]` glob does not compile, without touching the file system.
pub fn validate_scope(scope: &ScopeConfig) -> Result<()> {
    glob_set(&scope.include)?;
    glob_set(&scope.exclude)?;
    Ok(())
}

/// Files and directories of a project that pass `.gitignore` and `[scope]`.
#[derive(Debug, Clone)]
pub struct ScopeFilter {
    root: PathBuf,
    files: Vec<String>,
    file_set: HashSet<String>,
    dirs: HashSet<String>,
}

impl ScopeFilter {
    /// Walks `root` once (respecting `.gitignore`, `.ignore`, `.git/info/exclude`).
    pub fn build(root: &Path, scope: &ScopeConfig) -> Result<Self> {
        Self::build_diagnostic(root, scope, false)
    }

    /// Diagnostic walk that can include hidden dot-directories when `include_hidden` is true.
    pub fn build_diagnostic(
        root: &Path,
        scope: &ScopeConfig,
        include_hidden: bool,
    ) -> Result<Self> {
        let (standard, diagnostic) = Self::build_with_diagnostic(root, scope, include_hidden)?;
        Ok(diagnostic.unwrap_or(standard))
    }

    /// A single walk returning the standard scope and, when `include_hidden` is true, the
    /// diagnostic scope that also descends into hidden dot-directories (and [`IGNORED_DIRS`])
    /// not opened by `[scope].include`. Entries reachable only through such a directory are
    /// left out of the standard scope, so it equals [`ScopeFilter::build`].
    pub fn build_with_diagnostic(
        root: &Path,
        scope: &ScopeConfig,
        include_hidden: bool,
    ) -> Result<(Self, Option<Self>)> {
        let include = glob_set(&scope.include)?;
        let exclude = glob_set(&scope.exclude)?;
        let extensions: HashSet<String> = scope
            .extensions
            .iter()
            .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|e| !e.is_empty())
            .collect();

        let inc = Arc::new(include.clone());
        let scope_patterns: Arc<Vec<String>> = Arc::new(scope.include.clone());
        let root_buf = root.to_path_buf();
        let walker = WalkBuilder::new(root)
            .hidden(false)
            .git_global(false)
            .require_git(false)
            .follow_links(false)
            .filter_entry(move |entry| {
                let name = entry.file_name().to_string_lossy();
                if name == ".git" {
                    return false;
                }
                let depth = entry.depth();
                if depth == 0 {
                    return true;
                }
                let Ok(rel_path) = entry.path().strip_prefix(&root_buf) else {
                    return true;
                };
                let rel = rel_path.to_string_lossy().replace('\\', "/");
                let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
                if is_dir {
                    if ALWAYS_IGNORED_RUNTIME_DIRS
                        .iter()
                        .any(|d| rel == *d || rel.starts_with(&format!("{d}/")))
                    {
                        return false;
                    }
                    let explicitly_allowed = explicitly_included(&rel, &inc)
                        || include_allows_dir_traversal(&rel, &scope_patterns);
                    if (name.starts_with('.') || IGNORED_DIRS.contains(&name.as_ref()))
                        && !explicitly_allowed
                        && !include_hidden
                    {
                        return false;
                    }
                    if entry.path().join(".git").exists() && !explicitly_allowed {
                        return false;
                    }
                    true
                } else {
                    if is_ignored_filename(&name) && !explicitly_included(&rel, &inc) {
                        return false;
                    }
                    true
                }
            })
            .build();

        // Diagnostic-only directories: hidden (or ignored) and not opened by `include`, or
        // below such a directory. The walk yields a directory before its contents.
        let hidden_only = |name: &str, rel: &str| {
            (name.starts_with('.') || IGNORED_DIRS.contains(&name))
                && !(explicitly_included(rel, &include)
                    || include_allows_dir_traversal(rel, &scope.include))
        };
        let mut hidden_dirs: HashSet<String> = HashSet::new();
        let mut files = Vec::new();
        let mut dirs = HashSet::new();
        let mut diag_files = Vec::new();
        let mut diag_dirs = HashSet::new();
        for entry in walker.flatten() {
            if entry.depth() == 0 {
                continue;
            }
            let Some(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(root) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            let under_hidden = include_hidden && {
                let parent = rel.rsplit_once('/').map_or("", |(p, _)| p);
                hidden_dirs.contains(parent)
            };
            if kind.is_dir() {
                let name = rel.rsplit('/').next().unwrap_or(&rel);
                let is_hidden = include_hidden && (under_hidden || hidden_only(name, &rel));
                if include_hidden {
                    diag_dirs.insert(rel.clone());
                }
                if is_hidden {
                    hidden_dirs.insert(rel);
                } else {
                    dirs.insert(rel);
                }
                continue;
            }
            if !kind.is_file() || exclude.is_match(&rel) {
                continue;
            }
            if !include.is_empty() && !include.is_match(&rel) {
                continue;
            }
            if !extensions.is_empty() {
                let canon_ext = canonical_file_ext(&rel);
                let raw_ext = Path::new(&rel)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_ascii_lowercase);
                let matches_canon = canon_ext.as_ref().is_some_and(|e| extensions.contains(e));
                let matches_raw = raw_ext.as_ref().is_some_and(|e| extensions.contains(e));
                if !matches_canon && !matches_raw {
                    continue;
                }
            }
            if include_hidden {
                diag_files.push(rel.clone());
            }
            if !under_hidden {
                files.push(rel);
            }
        }
        let standard = Self::from_parts(root, files, dirs);
        let diagnostic = include_hidden.then(|| Self::from_parts(root, diag_files, diag_dirs));
        Ok((standard, diagnostic))
    }

    fn from_parts(root: &Path, mut files: Vec<String>, dirs: HashSet<String>) -> Self {
        files.sort();
        let file_set = files.iter().cloned().collect();
        Self {
            root: root.to_path_buf(),
            files,
            file_set,
            dirs,
        }
    }

    /// Files in scope below the relative directory `rel_dir` (`""` = every file), as a
    /// contiguous slice of the sorted [`ScopeFilter::files`] found by binary search.
    pub fn files_under(&self, rel_dir: &str) -> &[String] {
        let rel_dir = rel_dir.trim_matches('/');
        if rel_dir.is_empty() {
            return &self.files;
        }
        let prefix = format!("{rel_dir}/");
        let start = self.files.partition_point(|f| f.as_str() < prefix.as_str());
        let len = self.files[start..].partition_point(|f| f.starts_with(&prefix));
        &self.files[start..start + len]
    }

    /// Project-relative (`/`-separated) form of `path`; `None` when outside the root.
    pub fn relative(&self, path: &Path) -> Option<String> {
        self.rel(path)
    }

    /// Relative (`/`-separated) paths of all files in scope, sorted.
    pub fn files(&self) -> &[String] {
        &self.files
    }

    fn rel(&self, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        if rel.is_absolute() {
            return None;
        }
        Some(rel.to_string_lossy().replace('\\', "/"))
    }

    pub fn allows_file(&self, path: &Path) -> bool {
        self.rel(path).is_some_and(|r| self.file_set.contains(&r))
    }

    /// The root itself and every directory that survived `.gitignore` are allowed.
    pub fn allows_dir(&self, path: &Path) -> bool {
        match self.rel(path) {
            Some(r) => r.is_empty() || self.dirs.contains(&r),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_excludes_lockfiles_dot_dirs_and_nested_git_clones() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n").unwrap();
        std::fs::write(root.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();
        std::fs::write(root.join("Cargo.lock"), "version = 3\n").unwrap();
        std::fs::write(root.join("uv.lock"), "version = 1\n").unwrap();
        std::fs::write(root.join("tsconfig.tsbuildinfo"), "{}").unwrap();

        // Unlisted dot-directories (.agent, .changeset, .github)
        std::fs::create_dir_all(root.join(".agent/skills/foo")).unwrap();
        std::fs::write(root.join(".agent/skills/foo/SKILL.md"), "# skill").unwrap();
        std::fs::create_dir_all(root.join(".changeset")).unwrap();
        std::fs::write(root.join(".changeset/config.json"), "{}").unwrap();

        // Nested foreign git clone (docs/OpenViking/.git)
        std::fs::create_dir_all(root.join("docs/OpenViking/.git")).unwrap();
        std::fs::write(root.join("docs/OpenViking/main.py"), "print(1)\n").unwrap();

        let scope = ScopeFilter::build(root, &ScopeConfig::default()).unwrap();
        let files = scope.files();
        assert_eq!(files, &["src/lib.rs".to_string()]);
    }

    #[test]
    fn single_walk_with_diagnostic_matches_separate_walks() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for (rel, text) in [
            ("src/lib.rs", "pub fn ok() {}\n"),
            ("src/.hidden/x.rs", "fn x() {}\n"),
            (".agents/guide.md", "# guide\n"),
            (".agents/deep/more.md", "# more\n"),
            (".config/app/settings.toml", "k = 1\n"),
            ("vendor/dep.rs", "fn dep() {}\n"),
            ("docs/notes.md", "# notes\n"),
            (".gitignore", "ignored/\n"),
            ("ignored/skip.rs", "fn skip() {}\n"),
        ] {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        for scope in [
            ScopeConfig::default(),
            ScopeConfig {
                include: vec![".config/app/**".into(), "src/**".into()],
                ..Default::default()
            },
        ] {
            let (standard, diagnostic) =
                ScopeFilter::build_with_diagnostic(root, &scope, true).unwrap();
            let separate_std = ScopeFilter::build(root, &scope).unwrap();
            let separate_diag = ScopeFilter::build_diagnostic(root, &scope, true).unwrap();
            let diagnostic = diagnostic.expect("diagnostic scope");
            assert_eq!(standard.files(), separate_std.files(), "{scope:?}");
            assert_eq!(standard.dirs, separate_std.dirs, "{scope:?}");
            assert_eq!(diagnostic.files(), separate_diag.files(), "{scope:?}");
            if scope.include.is_empty() {
                assert!(diagnostic
                    .files()
                    .contains(&".agents/deep/more.md".to_string()));
                assert!(!standard
                    .files()
                    .contains(&".agents/deep/more.md".to_string()));
            }
            let (only_std, none) = ScopeFilter::build_with_diagnostic(root, &scope, false).unwrap();
            assert!(none.is_none());
            assert_eq!(only_std.files(), standard.files());
        }
        let scope = ScopeFilter::build(root, &ScopeConfig::default()).unwrap();
        assert_eq!(scope.files_under("src"), &["src/lib.rs".to_string()]);
        assert_eq!(scope.files_under(""), scope.files());
        assert!(scope.files_under("sr").is_empty());
    }

    #[test]
    fn scope_include_prefix_descends_into_explicit_dot_dir_but_not_wildcard() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".config/app")).unwrap();
        std::fs::write(root.join(".config/app/settings.toml"), "key = 1\n").unwrap();
        std::fs::create_dir_all(root.join(".agents")).unwrap();
        std::fs::write(root.join(".agents/secret.md"), "secret\n").unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/config"), "bare = false\n").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();

        let explicit_scope = ScopeConfig {
            include: vec![".config/app/**".into()],
            ..Default::default()
        };
        let filter = ScopeFilter::build(root, &explicit_scope).unwrap();
        assert_eq!(filter.files(), &[".config/app/settings.toml".to_string()]);
        assert!(!filter.files().contains(&".git/config".to_string()));

        let wildcard_scope = ScopeConfig {
            include: vec!["**/*.toml".into(), "**/*.md".into()],
            ..Default::default()
        };
        let filter2 = ScopeFilter::build(root, &wildcard_scope).unwrap();
        assert!(!filter2
            .files()
            .contains(&".config/app/settings.toml".to_string()));
        assert!(!filter2.files().contains(&".agents/secret.md".to_string()));
        assert!(!filter2.files().contains(&".git/config".to_string()));
    }
}
