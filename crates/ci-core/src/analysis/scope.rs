//! File scope shared by `scan` and the detector (D9): `.gitignore` rules plus the `[scope]`
//! globs. A single walk decides which files and directories exist for the whole analysis, so
//! `scan`, `init`, `update` and `pack` always agree. Symlinks are never followed or listed,
//! so nothing outside the project root can leak in.

use crate::analysis::detector::IGNORED_DIRS;
use crate::config::ScopeConfig;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn glob_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        let pat = pat.trim().trim_start_matches("./");
        let glob = Glob::new(pat).with_context(|| format!("Invalid scope glob '{pat}'"))?;
        builder.add(glob);
    }
    Ok(builder.build()?)
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
        let include = glob_set(&scope.include)?;
        let exclude = glob_set(&scope.exclude)?;
        let extensions: HashSet<String> = scope
            .extensions
            .iter()
            .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|e| !e.is_empty())
            .collect();

        let walker = WalkBuilder::new(root)
            .hidden(false)
            .git_global(false)
            .require_git(false)
            .follow_links(false)
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                if name == ".git" {
                    return false;
                }
                let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
                !(is_dir && IGNORED_DIRS.contains(&name.as_ref()))
            })
            .build();

        let mut files = Vec::new();
        let mut dirs = HashSet::new();
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
            if kind.is_dir() {
                dirs.insert(rel);
                continue;
            }
            if !kind.is_file() || exclude.is_match(&rel) {
                continue;
            }
            if !include.is_empty() && !include.is_match(&rel) {
                continue;
            }
            if !extensions.is_empty() {
                let ext = Path::new(&rel)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !extensions.contains(&ext) {
                    continue;
                }
            }
            files.push(rel);
        }
        files.sort();
        let file_set = files.iter().cloned().collect();
        Ok(Self {
            root: root.to_path_buf(),
            files,
            file_set,
            dirs,
        })
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
