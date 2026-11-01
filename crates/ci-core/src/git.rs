//! Git backend abstraction (`GitBackend`), `SystemGitBackend` (`-z` / `--porcelain=v1 -z`), and OKF card staleness verification (D2, D3).

use crate::openviking::{CardFrontmatter, OkfCard};
use crate::storage::{normalize_line_endings, normalize_path};
use anyhow::{bail, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Single parsed entry from `git status --porcelain=v1 -z` (D3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitStatusEntry {
    /// Index status character (`X` in `XY`).
    pub index_status: char,
    /// Worktree status character (`Y` in `XY`).
    pub worktree_status: char,
    /// Normalized forward-slash relative path.
    pub path: String,
    /// Original normalized path for rename (`R`) or copy (`C`) entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orig_path: Option<String>,
}

impl GitStatusEntry {
    /// Returns the two-character porcelain status code (`XY`).
    pub fn status_code(&self) -> String {
        format!("{}{}", self.index_status, self.worktree_status)
    }
}

/// Batched snapshot of repository Git state for efficient multi-card staleness checking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoGitSnapshot {
    /// Current `HEAD` commit SHA.
    pub head_commit: String,
    /// Working-tree and index status entries.
    pub status_entries: Vec<GitStatusEntry>,
}

/// Result of checking an OKF card's staleness against the current Git state (D2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StalenessReport {
    /// `true` if the card's source blob/tree SHA or files under `directory` changed since generation.
    pub stale: bool,
    /// Commit SHA recorded in the card frontmatter.
    pub recorded_commit_sha: String,
    /// Current `HEAD` commit SHA in the repository.
    pub current_commit_sha: String,
    /// Blob/tree SHA recorded in the card frontmatter.
    pub recorded_blob_sha: String,
    /// Current blob/tree SHA of the card's `directory` / file in the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_blob_sha: Option<String>,
    /// Normalized paths of files under `directory` that changed since `recorded_commit_sha`.
    pub changed_files: Vec<String>,
    /// Compact `git diff --stat` summary when `stale == true`.
    pub diff_stat: String,
}

/// Abstraction over Git operations for staleness validation, diffing, and project identification (D2, D3).
pub trait GitBackend: Send + Sync {
    /// Returns the full SHA-1/SHA-256 of `HEAD`.
    fn head_commit(&self, repo_root: &Path) -> Result<String>;

    /// Returns the URL of the primary remote (`origin`, or first configured remote), if any.
    fn remote_url(&self, repo_root: &Path) -> Result<Option<String>>;

    /// Returns the current Git blob SHA (for a file) or tree SHA (for a directory) of `relative_path`,
    /// or `Ok(None)` if the path does not exist.
    fn blob_sha(&self, repo_root: &Path, relative_path: &Path) -> Result<Option<String>>;

    /// Returns working-tree and index status entries using `--porcelain=v1 -z` with paths normalized to `/`.
    fn status_porcelain_z(&self, repo_root: &Path) -> Result<Vec<GitStatusEntry>>;

    /// Returns normalized `/` relative paths changed between `from_commit` and `to_commit` (`git diff --name-only -z`).
    fn diff_name_only(
        &self,
        repo_root: &Path,
        from_commit: &str,
        to_commit: &str,
    ) -> Result<Vec<String>>;

    /// Returns compact `git diff --stat` output between `from_commit` and `to_commit`, optionally scoped to `paths`.
    fn diff_stat(
        &self,
        repo_root: &Path,
        from_commit: &str,
        to_commit: &str,
        paths: &[String],
    ) -> Result<String>;

    /// Returns a batched Git snapshot combining HEAD commit and working tree status.
    fn repo_snapshot(&self, repo_root: &Path) -> Result<RepoGitSnapshot> {
        let head_commit = self.head_commit(repo_root)?;
        let status_entries = self.status_porcelain_z(repo_root)?;
        Ok(RepoGitSnapshot {
            head_commit,
            status_entries,
        })
    }

    /// Computes the blob or tree SHA for the given subsystem paths.
    fn paths_tree_sha(&self, repo_root: &Path, paths: &[String]) -> Result<String> {
        if paths.is_empty() {
            return self
                .blob_sha(repo_root, Path::new("."))
                .map(|s| s.unwrap_or_default());
        }
        if paths.len() == 1 {
            return self
                .blob_sha(repo_root, Path::new(&paths[0]))
                .map(|s| s.unwrap_or_default());
        }
        let mut parts = Vec::new();
        for p in paths {
            let sha = self.blob_sha(repo_root, Path::new(p))?.unwrap_or_default();
            parts.push(format!("{p}:{sha}"));
        }
        parts.sort();
        let mut hasher = Sha256::new();
        for item in parts {
            hasher.update(item.as_bytes());
            hasher.update(b"\n");
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Current branch name (`HEAD` when detached).
    fn current_branch(&self, _repo_root: &Path) -> Result<String> {
        bail!("branch lookup is not supported by this backend")
    }

    /// Best common ancestor of `a` and `b` (`git merge-base`).
    fn merge_base(&self, _repo_root: &Path, _a: &str, _b: &str) -> Result<String> {
        bail!("merge-base is not supported by this backend")
    }

    /// `git log --oneline from..to`, newest first, at most `limit` entries.
    fn log_oneline(
        &self,
        _repo_root: &Path,
        _from_commit: &str,
        _to_commit: &str,
        _limit: usize,
    ) -> Result<Vec<String>> {
        bail!("log is not supported by this backend")
    }

    /// Content of `path` at `rev` (`git show rev:path`); `None` when it does not exist there.
    fn file_at(&self, _repo_root: &Path, _rev: &str, _path: &str) -> Result<Option<String>> {
        bail!("show is not supported by this backend")
    }

    /// Unified diff (3 lines of context) of `path` between two commits.
    fn diff_file(
        &self,
        _repo_root: &Path,
        _from_commit: &str,
        _to_commit: &str,
        _path: &str,
    ) -> Result<String> {
        bail!("diff is not supported by this backend")
    }
}

/// Resolves the actual `.git` directory for a repository, resolving `.git` gitfiles (submodules, worktrees).
pub fn resolve_git_dir(repo_root: &Path) -> Option<PathBuf> {
    let dot_git = repo_root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    if dot_git.is_file() {
        if let Ok(content) = std::fs::read_to_string(&dot_git) {
            for line in content.lines() {
                let line = line.trim();
                if let Some(target) = line.strip_prefix("gitdir:") {
                    let target = target.trim();
                    let target_path = Path::new(target);
                    let resolved = if target_path.is_absolute() {
                        target_path.to_path_buf()
                    } else {
                        repo_root.join(target_path)
                    };
                    if resolved.exists() {
                        return Some(resolved);
                    }
                }
            }
        }
    }
    None
}

/// Resolves the common git directory for linked worktrees (reading `<git_dir>/commondir` if present).
pub fn resolve_common_git_dir(git_dir: &Path) -> PathBuf {
    let commondir_file = git_dir.join("commondir");
    if let Ok(content) = std::fs::read_to_string(&commondir_file) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            let p = Path::new(trimmed);
            let resolved = if p.is_absolute() {
                p.to_path_buf()
            } else {
                git_dir.join(p)
            };
            if resolved.exists() {
                return resolved;
            }
        }
    }
    git_dir.to_path_buf()
}

/// Reads current HEAD commit SHA in-process without shelling out to git.
pub fn read_head_commit_in_process(repo_root: &Path) -> Option<String> {
    let git_dir = resolve_git_dir(repo_root)?;
    let head_content = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head_trimmed = head_content.trim();
    if is_hex_sha(head_trimmed) {
        return Some(head_trimmed.to_string());
    }
    let ref_path = head_trimmed.strip_prefix("ref:")?.trim();
    // 1. Try loose ref in git_dir
    let loose_in_git = git_dir.join(ref_path);
    if let Ok(c) = std::fs::read_to_string(&loose_in_git) {
        let sha = c.trim();
        if is_hex_sha(sha) {
            return Some(sha.to_string());
        }
    }
    // 2. Try loose ref in common_dir
    let common_dir = resolve_common_git_dir(&git_dir);
    if common_dir != git_dir {
        let loose_in_common = common_dir.join(ref_path);
        if let Ok(c) = std::fs::read_to_string(&loose_in_common) {
            let sha = c.trim();
            if is_hex_sha(sha) {
                return Some(sha.to_string());
            }
        }
    }
    // 3. Try packed-refs in common_dir, then git_dir
    for dir in [&common_dir, &git_dir] {
        let packed_file = dir.join("packed-refs");
        if let Ok(packed) = std::fs::read_to_string(&packed_file) {
            for line in packed.lines() {
                let line = line.trim();
                if line.starts_with('#') || line.starts_with('^') || line.is_empty() {
                    continue;
                }
                let mut parts = line.split_whitespace();
                if let (Some(sha), Some(r)) = (parts.next(), parts.next()) {
                    if r == ref_path && is_hex_sha(sha) {
                        return Some(sha.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Reads the primary remote URL in-process from `.git/config` without shelling out to git.
pub fn read_remote_url_in_process(repo_root: &Path) -> Option<String> {
    let git_dir = resolve_git_dir(repo_root)?;
    let common_dir = resolve_common_git_dir(&git_dir);
    let config_paths = if common_dir != git_dir {
        vec![common_dir.join("config"), git_dir.join("config")]
    } else {
        vec![git_dir.join("config")]
    };

    for config_path in config_paths {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut current_section = String::new();
            let mut origin_url = None;
            let mut first_remote_url = None;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    current_section = trimmed[1..trimmed.len() - 1].trim().to_string();
                    continue;
                }
                if let Some((k, v)) = trimmed.split_once('=') {
                    let k = k.trim();
                    let v = v.trim();
                    if k == "url" {
                        if current_section == "remote \"origin\"" {
                            origin_url = Some(v.to_string());
                        } else if current_section.starts_with("remote ")
                            && first_remote_url.is_none()
                        {
                            first_remote_url = Some(v.to_string());
                        }
                    }
                }
            }

            if let Some(url) = origin_url {
                return Some(url);
            }
            if let Some(url) = first_remote_url {
                return Some(url);
            }
        }
    }
    None
}

/// System `git` CLI implementation of `GitBackend` using `-z` / `--porcelain=v1 -z` (D3).
#[derive(Debug, Clone, Default)]
pub struct SystemGitBackend {
    git_binary: String,
}

impl SystemGitBackend {
    /// Creates a new `SystemGitBackend` invoking `"git"` from `PATH`.
    pub fn new() -> Self {
        Self {
            git_binary: "git".to_string(),
        }
    }

    /// Creates a `SystemGitBackend` with a custom `git` executable path.
    pub fn with_binary(git_binary: impl Into<String>) -> Self {
        Self {
            git_binary: git_binary.into(),
        }
    }

    fn run_git(&self, repo_root: &Path, args: &[&str]) -> Result<std::process::Output> {
        Command::new(&self.git_binary)
            .arg("-C")
            .arg(repo_root)
            .arg("-c")
            .arg("core.quotePath=false")
            .args(args)
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute '{} -C {:?} {}'",
                    self.git_binary,
                    repo_root,
                    args.join(" ")
                )
            })
    }
}

impl GitBackend for SystemGitBackend {
    fn head_commit(&self, repo_root: &Path) -> Result<String> {
        if let Some(sha) = read_head_commit_in_process(repo_root) {
            return Ok(sha);
        }
        let output = self.run_git(repo_root, &["rev-parse", "HEAD"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "git rev-parse HEAD failed in {:?}: {}",
                repo_root,
                stderr.trim()
            );
        }
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if sha.is_empty() {
            bail!(
                "git rev-parse HEAD returned empty output in {:?}",
                repo_root
            );
        }
        Ok(sha)
    }

    fn remote_url(&self, repo_root: &Path) -> Result<Option<String>> {
        if let Some(url) = read_remote_url_in_process(repo_root) {
            return Ok(Some(url));
        }
        let origin_out = self.run_git(repo_root, &["config", "--get", "remote.origin.url"])?;
        if origin_out.status.success() {
            let url = String::from_utf8_lossy(&origin_out.stdout)
                .trim()
                .to_string();
            if !url.is_empty() {
                return Ok(Some(url));
            }
        }

        // Fallback: inspect first remote in `git remote`
        let remotes_out = self.run_git(repo_root, &["remote"])?;
        if !remotes_out.status.success() {
            return Ok(None);
        }
        let remotes_text = String::from_utf8_lossy(&remotes_out.stdout);
        let Some(first_remote) = remotes_text.lines().map(str::trim).find(|l| !l.is_empty()) else {
            return Ok(None);
        };

        let key = format!("remote.{first_remote}.url");
        let fallback_out = self.run_git(repo_root, &["config", "--get", &key])?;
        if fallback_out.status.success() {
            let url = String::from_utf8_lossy(&fallback_out.stdout)
                .trim()
                .to_string();
            if !url.is_empty() {
                return Ok(Some(url));
            }
        }
        Ok(None)
    }

    fn blob_sha(&self, repo_root: &Path, relative_path: &Path) -> Result<Option<String>> {
        let stripped_rel = relative_path
            .strip_prefix(repo_root)
            .unwrap_or(relative_path);
        let norm_rel = normalize_path(stripped_rel);
        let full_path = if norm_rel.is_empty() || norm_rel == "." {
            repo_root.to_path_buf()
        } else {
            repo_root.join(&norm_rel)
        };

        if !full_path.exists() {
            return Ok(None);
        }

        if full_path.is_file() {
            let out = self.run_git(repo_root, &["hash-object", "--", &norm_rel])?;
            if out.status.success() {
                let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !sha.is_empty() {
                    return Ok(Some(sha));
                }
            }
        }

        // Directory or fallback: query Git tree/blob object at HEAD
        let spec = if norm_rel.is_empty() || norm_rel == "." {
            "HEAD^{tree}".to_string()
        } else {
            format!("HEAD:{norm_rel}")
        };
        let out = self.run_git(repo_root, &["rev-parse", &spec])?;
        if out.status.success() {
            let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !sha.is_empty() {
                return Ok(Some(sha));
            }
        }

        Ok(None)
    }

    fn status_porcelain_z(&self, repo_root: &Path) -> Result<Vec<GitStatusEntry>> {
        let output = self.run_git(repo_root, &["status", "--porcelain=v1", "-z"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "git status --porcelain=v1 -z failed in {:?}: {}",
                repo_root,
                stderr.trim()
            );
        }
        Ok(parse_porcelain_z(&output.stdout))
    }

    fn diff_name_only(
        &self,
        repo_root: &Path,
        from_commit: &str,
        to_commit: &str,
    ) -> Result<Vec<String>> {
        reject_option_like(&[from_commit, to_commit])?;
        let range = format!("{}..{}", from_commit.trim(), to_commit.trim());
        let output = self.run_git(repo_root, &["diff", "--name-only", "-z", &range])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "git diff --name-only -z {} failed in {:?}: {}",
                range,
                repo_root,
                stderr.trim()
            );
        }
        Ok(parse_nul_paths(&output.stdout))
    }

    fn diff_stat(
        &self,
        repo_root: &Path,
        from_commit: &str,
        to_commit: &str,
        paths: &[String],
    ) -> Result<String> {
        reject_option_like(&[from_commit, to_commit])?;
        let range = format!("{}..{}", from_commit.trim(), to_commit.trim());
        let mut args: Vec<String> = vec!["diff".to_string(), "--stat".to_string(), range];

        let scoped: Vec<String> = paths
            .iter()
            .map(normalize_path)
            .filter(|p| !p.is_empty() && p != ".")
            .collect();

        if !scoped.is_empty() {
            args.push("--".to_string());
            args.extend(scoped);
        }

        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = self.run_git(repo_root, &arg_refs)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "git diff --stat failed in {:?}: {}",
                repo_root,
                stderr.trim()
            );
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(normalize_line_endings(&text).trim().to_string())
    }

    fn current_branch(&self, repo_root: &Path) -> Result<String> {
        let output = self.run_git(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        if !output.status.success() {
            bail!(
                "git rev-parse --abbrev-ref HEAD failed in {:?}: {}",
                repo_root,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn merge_base(&self, repo_root: &Path, a: &str, b: &str) -> Result<String> {
        reject_option_like(&[a, b])?;
        let output = self.run_git(repo_root, &["merge-base", a.trim(), b.trim()])?;
        if !output.status.success() {
            bail!(
                "git merge-base failed in {:?}: {}",
                repo_root,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn log_oneline(
        &self,
        repo_root: &Path,
        from_commit: &str,
        to_commit: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        reject_option_like(&[from_commit, to_commit])?;
        let range = format!("{}..{}", from_commit.trim(), to_commit.trim());
        let max = format!("--max-count={limit}");
        let output = self.run_git(repo_root, &["log", "--oneline", &max, &range])?;
        if !output.status.success() {
            bail!(
                "git log {} failed in {:?}: {}",
                range,
                repo_root,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(
            normalize_line_endings(&String::from_utf8_lossy(&output.stdout))
                .lines()
                .map(str::to_string)
                .collect(),
        )
    }

    fn file_at(&self, repo_root: &Path, rev: &str, path: &str) -> Result<Option<String>> {
        reject_option_like(&[rev])?;
        let spec = format!("{}:{}", rev.trim(), normalize_path(path));
        let output = self.run_git(repo_root, &["show", &spec])?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
    }

    fn diff_file(
        &self,
        repo_root: &Path,
        from_commit: &str,
        to_commit: &str,
        path: &str,
    ) -> Result<String> {
        reject_option_like(&[from_commit, to_commit])?;
        let range = format!("{}..{}", from_commit.trim(), to_commit.trim());
        let norm = normalize_path(path);
        let output = self.run_git(repo_root, &["diff", "-U3", &range, "--", &norm])?;
        if !output.status.success() {
            bail!(
                "git diff {} failed in {:?}: {}",
                range,
                repo_root,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(normalize_line_endings(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }
}

/// Refuses revisions that git would parse as options (`--output=...`): they can come from
/// untrusted card frontmatter or tool arguments.
fn reject_option_like(revs: &[&str]) -> Result<()> {
    for r in revs {
        if r.trim().starts_with('-') {
            bail!("refusing option-like git revision {:?}", r);
        }
    }
    Ok(())
}

/// A recorded commit/blob id: hex only, abbreviated or full.
fn is_hex_sha(s: &str) -> bool {
    (7..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Parses raw NUL-delimited `git status --porcelain=v1 -z` bytes into normalized `GitStatusEntry` items.
pub fn parse_porcelain_z(bytes: &[u8]) -> Vec<GitStatusEntry> {
    let mut entries = Vec::new();
    let mut parts = bytes.split(|&b| b == 0).filter(|s| !s.is_empty());

    while let Some(part) = parts.next() {
        let raw = String::from_utf8_lossy(part);
        let mut chars = raw.chars();
        let Some(x) = chars.next() else {
            continue;
        };
        let Some(y) = chars.next() else {
            continue;
        };
        // Next character must be a space separating XY from PATH
        if chars.next() != Some(' ') {
            continue;
        }
        let path_str: String = chars.collect();
        let path = normalize_path(&path_str);
        if path.is_empty() {
            continue;
        }

        let is_rename_or_copy = x == 'R' || y == 'R' || x == 'C' || y == 'C';
        let orig_path = if is_rename_or_copy {
            parts
                .next()
                .map(|orig_bytes| normalize_path(String::from_utf8_lossy(orig_bytes).as_ref()))
        } else {
            None
        };

        entries.push(GitStatusEntry {
            index_status: x,
            worktree_status: y,
            path,
            orig_path,
        });
    }

    entries
}

/// Parses NUL-delimited path lists (`git diff --name-only -z`) and normalizes paths to `/`.
pub fn parse_nul_paths(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| normalize_path(String::from_utf8_lossy(s).as_ref()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Verifies whether an `OkfCard` is stale relative to the current repository state via `GitBackend` (D2).
pub fn check_card_staleness(
    backend: &dyn GitBackend,
    repo_root: &Path,
    card: &OkfCard,
) -> Result<StalenessReport> {
    check_frontmatter_staleness(backend, repo_root, &card.frontmatter)
}

/// Verifies whether a `CardFrontmatter` is stale relative to the current repository state via `GitBackend` (D2).
pub fn check_frontmatter_staleness(
    backend: &dyn GitBackend,
    repo_root: &Path,
    frontmatter: &CardFrontmatter,
) -> Result<StalenessReport> {
    let current_commit = backend.head_commit(repo_root)?;
    let mut status_lookup_failed = false;
    let snapshot = match backend.repo_snapshot(repo_root) {
        Ok(s) => s,
        Err(_) => {
            status_lookup_failed = true;
            RepoGitSnapshot {
                head_commit: current_commit,
                status_entries: Vec::new(),
            }
        }
    };
    let mut report =
        check_frontmatter_staleness_with_snapshot(backend, repo_root, &snapshot, frontmatter)?;
    if status_lookup_failed {
        report.stale = true;
        if report.diff_stat.is_empty() {
            report.diff_stat = "working tree status check failed".to_string();
        }
    }
    Ok(report)
}

/// Verifies whether subsystem paths are stale using a precomputed Git repository snapshot.
pub fn check_subsystem_staleness_with_snapshot(
    backend: &dyn GitBackend,
    repo_root: &Path,
    snapshot: &RepoGitSnapshot,
    paths: &[String],
    recorded_commit_sha: &str,
    recorded_blob_sha: &str,
) -> Result<StalenessReport> {
    let recorded_commit = recorded_commit_sha.trim().to_string();
    let recorded_blob = recorded_blob_sha.trim().to_string();
    let current_commit = snapshot.head_commit.clone();
    let current_blob = backend.paths_tree_sha(repo_root, paths).ok();

    let mut changed_files = Vec::new();
    let mut commit_lookup_failed = false;

    let recorded_commit_ok = recorded_commit.is_empty() || is_hex_sha(&recorded_commit);
    if !recorded_commit_ok {
        commit_lookup_failed = true;
    } else if !recorded_commit.is_empty() && recorded_commit != current_commit {
        match backend.diff_name_only(repo_root, &recorded_commit, &current_commit) {
            Ok(diff_files) => {
                for f in diff_files {
                    if paths.is_empty() || paths.iter().any(|dir| path_matches_directory(&f, dir)) {
                        changed_files.push(f);
                    }
                }
            }
            Err(_) => {
                commit_lookup_failed = true;
            }
        }
    }

    for entry in &snapshot.status_entries {
        if paths.is_empty()
            || paths
                .iter()
                .any(|dir| path_matches_directory(&entry.path, dir))
        {
            changed_files.push(entry.path.clone());
        }
        if let Some(orig) = &entry.orig_path {
            if paths.is_empty() || paths.iter().any(|dir| path_matches_directory(orig, dir)) {
                changed_files.push(orig.clone());
            }
        }
    }

    changed_files.sort();
    changed_files.dedup();

    let blob_mismatch = current_blob.as_deref().unwrap_or("") != recorded_blob.as_str();
    let stale = blob_mismatch || !changed_files.is_empty() || commit_lookup_failed;

    let diff_stat = if stale {
        let scope_paths: Vec<String> = paths
            .iter()
            .map(|p| normalize_path(p))
            .filter(|p| !p.is_empty() && p != ".")
            .collect();

        let stat_from_git = if recorded_commit_ok
            && !recorded_commit.is_empty()
            && recorded_commit != current_commit
        {
            backend
                .diff_stat(repo_root, &recorded_commit, &current_commit, &scope_paths)
                .unwrap_or_default()
        } else {
            String::new()
        };

        if !stat_from_git.is_empty() {
            stat_from_git
        } else if !changed_files.is_empty() {
            format!(
                "{} file(s) modified in working tree ({})",
                changed_files.len(),
                changed_files.join(", ")
            )
        } else {
            format!(
                "{} blob changed ({} -> {})",
                if paths.is_empty() { "." } else { &paths[0] },
                if recorded_blob.is_empty() {
                    "none"
                } else {
                    &recorded_blob
                },
                current_blob.as_deref().unwrap_or("missing")
            )
        }
    } else {
        String::new()
    };

    Ok(StalenessReport {
        stale,
        recorded_commit_sha: recorded_commit,
        current_commit_sha: current_commit,
        recorded_blob_sha: recorded_blob,
        current_blob_sha: current_blob,
        changed_files,
        diff_stat,
    })
}

/// Verifies whether an `OkfCard` is stale relative to the current repository state via `GitBackend` using a snapshot.
pub fn check_card_staleness_with_snapshot(
    backend: &dyn GitBackend,
    repo_root: &Path,
    snapshot: &RepoGitSnapshot,
    card: &OkfCard,
) -> Result<StalenessReport> {
    check_frontmatter_staleness_with_snapshot(backend, repo_root, snapshot, &card.frontmatter)
}

/// Verifies whether a `CardFrontmatter` is stale relative to the current repository state via `GitBackend` using a snapshot.
pub fn check_frontmatter_staleness_with_snapshot(
    backend: &dyn GitBackend,
    repo_root: &Path,
    snapshot: &RepoGitSnapshot,
    frontmatter: &CardFrontmatter,
) -> Result<StalenessReport> {
    let dir = normalize_path(&frontmatter.directory);
    let paths = if dir.is_empty() || dir == "." {
        Vec::new()
    } else {
        vec![dir]
    };
    check_subsystem_staleness_with_snapshot(
        backend,
        repo_root,
        snapshot,
        &paths,
        &frontmatter.source.commit_sha,
        &frontmatter.source.blob_sha,
    )
}

fn path_matches_directory(file_path: &str, directory: &str) -> bool {
    let norm_file = normalize_path(file_path);
    let norm_dir = normalize_path(directory);
    if norm_dir.is_empty() || norm_dir == "." {
        return true;
    }
    if norm_file == norm_dir {
        return true;
    }
    norm_file
        .strip_prefix(&norm_dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn in_process_head_commit_and_remote_url_work_without_git_binary_and_support_gitfile() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let real_git = root.join(".actual_git");
        std::fs::create_dir_all(real_git.join("refs/heads")).unwrap();
        // .git is a gitfile pointing to .actual_git (worktree/submodule style)
        std::fs::write(root.join(".git"), "gitdir: .actual_git\n").unwrap();
        let fake_sha = "0123456789abcdef0123456789abcdef01234567";
        std::fs::write(real_git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(real_git.join("refs/heads/main"), format!("{fake_sha}\n")).unwrap();
        std::fs::write(
            real_git.join("config"),
            "[core]\n\trepositoryformatversion = 0\n[remote \"origin\"]\n\turl = git@github.com:org/repo.git\n",
        ).unwrap();

        // Use a non-existent git binary to prove zero-fork in-process parsing!
        let git = SystemGitBackend::with_binary("/nonexistent/git-binary");
        assert_eq!(git.head_commit(root).unwrap(), fake_sha);
        assert_eq!(
            git.remote_url(root).unwrap().as_deref(),
            Some("git@github.com:org/repo.git")
        );
    }

    #[test]
    fn repo_snapshot_matches_individual_staleness_checks() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        run_git(root, &["init"]);
        run_git(root, &["config", "user.email", "t@example.com"]);
        run_git(root, &["config", "user.name", "Test"]);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "-m", "init"]);

        let git = SystemGitBackend::new();
        let snap = git.repo_snapshot(root).unwrap();
        let paths = vec!["src".to_string()];
        let sha = git.paths_tree_sha(root, &paths).unwrap();
        let head = git.head_commit(root).unwrap();

        let report =
            check_subsystem_staleness_with_snapshot(&git, root, &snap, &paths, &head, &sha)
                .unwrap();
        assert!(!report.stale);
    }
}
