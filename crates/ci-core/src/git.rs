//! Git backend abstraction (`GitBackend`), `SystemGitBackend` (`-z` / `--porcelain=v1 -z`), and OKF card staleness verification (D2, D3).

use crate::openviking::{CardFrontmatter, OkfCard};
use crate::storage::{normalize_line_endings, normalize_path};
use anyhow::{bail, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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

/// Single entry of `git ls-tree -r -t -z HEAD` (`<mode> <type> <sha>\t<path>`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitTreeEntry {
    /// Octal mode (`100644`, `040000`, `160000`, ...).
    pub mode: String,
    /// Object type: `blob`, `tree` or `commit` (submodule gitlink).
    pub object_type: String,
    /// Object id.
    pub sha: String,
}

/// Cache of `git diff --name-only <recorded>..HEAD` results keyed by the recorded commit, so
/// cards sharing a `commit_sha` cost one `git diff` per run. `None` records a failed lookup.
pub type DiffNameCache = BTreeMap<String, Option<Vec<String>>>;

/// Batched snapshot of repository Git state for efficient multi-card staleness checking.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoGitSnapshot {
    /// Current `HEAD` commit SHA.
    pub head_commit: String,
    /// Working-tree and index status entries.
    pub status_entries: Vec<GitStatusEntry>,
    /// `path -> entry` from one `git ls-tree --full-tree -r -t -z HEAD`, keyed relative to the
    /// repository root like `status_entries` (empty when not batched).
    #[serde(default, skip)]
    #[schemars(skip)]
    pub tree_entries: BTreeMap<String, GitTreeEntry>,
    /// `git rev-parse --show-prefix` of the project root inside the repository (`""` at the
    /// repository root, else `"sub/dir/"`).
    #[serde(default, skip)]
    #[schemars(skip)]
    pub repo_prefix: String,
    /// `HEAD^{tree}` (for `directory == "."`); `None` on an unborn `HEAD` or when not batched.
    #[serde(default, skip)]
    #[schemars(skip)]
    pub root_tree_sha: Option<String>,
    /// Repository without commits (unborn `HEAD`).
    #[serde(default, skip)]
    #[schemars(skip)]
    pub unborn_head: bool,
}

impl RepoGitSnapshot {
    /// Whether the snapshot carries the batched tree (`SystemGitBackend::repo_snapshot`), so
    /// [`Self::blob_sha_for`] can answer without one `git` process per path.
    pub fn is_batched(&self) -> bool {
        self.unborn_head || self.root_tree_sha.is_some()
    }

    /// Whether `repo_path` (relative to the repository root) has a `git status` entry.
    fn status_touches(&self, repo_path: &str) -> bool {
        self.status_entries
            .iter()
            .any(|e| e.path == repo_path || e.orig_path.as_deref() == Some(repo_path))
    }

    /// Same result as [`GitBackend::blob_sha`] of `SystemGitBackend`, served from the batched
    /// tree: `"."` from `root_tree_sha`, directories (and submodules) from `tree_entries`, clean
    /// tracked files from `tree_entries`; dirty/untracked/ignored files, symlinks and files on
    /// an unborn `HEAD` are hashed from the working tree (`git hash-object`).
    ///
    /// Files are looked up as `repo_prefix + path` (the project may sit in a subdirectory of
    /// the repository); directories as `path` from the repository root, exactly like the
    /// `rev-parse HEAD:<path>` of `blob_sha`.
    ///
    /// Known limitation: a file marked `assume-unchanged` / `skip-worktree` with local edits
    /// has no `git status` entry, so its committed blob is returned where `blob_sha` would
    /// hash the working-tree content.
    pub fn blob_sha_for(&self, repo_root: &Path, relative_path: &Path) -> Result<Option<String>> {
        let stripped_rel = relative_path
            .strip_prefix(repo_root)
            .unwrap_or(relative_path);
        let norm_rel = normalize_path(stripped_rel);
        let is_root = norm_rel.is_empty() || norm_rel == ".";
        let full_path = if is_root {
            repo_root.to_path_buf()
        } else {
            repo_root.join(&norm_rel)
        };

        if !full_path.exists() {
            return Ok(None);
        }

        if full_path.is_file() {
            let is_symlink = std::fs::symlink_metadata(&full_path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(true);
            let repo_path = format!("{}{norm_rel}", self.repo_prefix);
            let clean_tracked =
                (!self.unborn_head && !is_symlink && !self.status_touches(&repo_path))
                    .then(|| self.tree_entries.get(&repo_path))
                    .flatten()
                    .filter(|e| e.object_type == "blob");
            if let Some(entry) = clean_tracked {
                return Ok(Some(entry.sha.clone()));
            }
            if let Some(sha) = SystemGitBackend::new().hash_object(repo_root, &norm_rel)? {
                return Ok(Some(sha));
            }
        }

        if is_root {
            return Ok(self.root_tree_sha.clone());
        }
        if self.unborn_head {
            return Ok(None);
        }
        Ok(self.tree_entries.get(&norm_rel).map(|e| e.sha.clone()))
    }

    /// Same result as [`GitBackend::paths_tree_sha`], computed from [`Self::blob_sha_for`].
    pub fn paths_tree_sha(&self, repo_root: &Path, paths: &[String]) -> Result<String> {
        paths_tree_sha_with(paths, |p| self.blob_sha_for(repo_root, p))
    }
}

/// Shared `paths_tree_sha` algorithm: `"."` for no paths, the raw SHA for one path, else a
/// SHA-256 of the sorted `"{path}:{sha}\n"` lines.
fn paths_tree_sha_with(
    paths: &[String],
    mut blob_sha: impl FnMut(&Path) -> Result<Option<String>>,
) -> Result<String> {
    if paths.is_empty() {
        return blob_sha(Path::new(".")).map(|s| s.unwrap_or_default());
    }
    if paths.len() == 1 {
        return blob_sha(Path::new(&paths[0])).map(|s| s.unwrap_or_default());
    }
    let mut parts = Vec::new();
    for p in paths {
        let sha = blob_sha(Path::new(p))?.unwrap_or_default();
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

/// `paths_tree_sha` from the batched snapshot when available, else from `backend`.
pub fn snapshot_paths_tree_sha(
    backend: &dyn GitBackend,
    repo_root: &Path,
    snapshot: &RepoGitSnapshot,
    paths: &[String],
) -> Result<String> {
    if snapshot.is_batched() {
        snapshot.paths_tree_sha(repo_root, paths)
    } else {
        backend.paths_tree_sha(repo_root, paths)
    }
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
            ..RepoGitSnapshot::default()
        })
    }

    /// Computes the blob or tree SHA for the given subsystem paths.
    fn paths_tree_sha(&self, repo_root: &Path, paths: &[String]) -> Result<String> {
        paths_tree_sha_with(paths, |p| self.blob_sha(repo_root, p))
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

    /// `git hash-object -- <norm_rel>` of the working-tree file; `None` when git fails.
    fn hash_object(&self, repo_root: &Path, norm_rel: &str) -> Result<Option<String>> {
        let out = self.run_git(repo_root, &["hash-object", "--", norm_rel])?;
        if out.status.success() {
            let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !sha.is_empty() {
                return Ok(Some(sha));
            }
        }
        Ok(None)
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
            if let Some(sha) = self.hash_object(repo_root, &norm_rel)? {
                return Ok(Some(sha));
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

    /// One `git status`, one `git rev-parse --show-prefix --verify HEAD^{tree}` and one
    /// `git ls-tree --full-tree -r -t -z HEAD`; an unborn `HEAD` yields `unborn_head = true`
    /// instead of an error (outside a repository `git status` still fails).
    fn repo_snapshot(&self, repo_root: &Path) -> Result<RepoGitSnapshot> {
        let status_entries = self.status_porcelain_z(repo_root)?;
        let tree_out = self.run_git(
            repo_root,
            &[
                "rev-parse",
                "--show-prefix",
                "--verify",
                "-q",
                "HEAD^{tree}",
            ],
        )?;
        // stdout: `<prefix>\n<tree sha>\n` (the prefix line is empty at the repository root).
        let stdout = String::from_utf8_lossy(&tree_out.stdout).into_owned();
        let lines: Vec<&str> = stdout.lines().collect();
        let parsed = match lines.as_slice() {
            [prefix, sha] if tree_out.status.success() && is_hex_sha(sha.trim()) => {
                Some((normalize_prefix(prefix), sha.trim().to_string()))
            }
            _ => None,
        };
        let Some((repo_prefix, root_tree_sha)) = parsed else {
            return unparsed_tree_snapshot(repo_root, self.head_commit(repo_root), status_entries);
        };
        let head_commit = self.head_commit(repo_root)?;
        let ls_out = self.run_git(
            repo_root,
            &["ls-tree", "--full-tree", "-r", "-t", "-z", "HEAD"],
        )?;
        if !ls_out.status.success() {
            bail!(
                "git ls-tree --full-tree -r -t -z HEAD failed in {:?}: {}",
                repo_root,
                String::from_utf8_lossy(&ls_out.stderr).trim()
            );
        }
        Ok(RepoGitSnapshot {
            head_commit,
            status_entries,
            tree_entries: parse_ls_tree_z(&ls_out.stdout),
            repo_prefix,
            root_tree_sha: Some(root_tree_sha),
            unborn_head: false,
        })
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

/// Snapshot for an unparsable `rev-parse --show-prefix --verify HEAD^{tree}` reply: an unborn
/// `HEAD` (no commit yet) when `head` failed, else an error, so callers fall back to per-card
/// Git calls instead of silently treating a repository with commits as unborn.
fn unparsed_tree_snapshot(
    repo_root: &Path,
    head: Result<String>,
    status_entries: Vec<GitStatusEntry>,
) -> Result<RepoGitSnapshot> {
    if let Ok(head) = head {
        bail!(
            "Unexpected `git rev-parse --show-prefix --verify HEAD^{{tree}}` output in {:?} (HEAD {})",
            repo_root,
            head
        );
    }
    Ok(RepoGitSnapshot {
        status_entries,
        unborn_head: true,
        ..RepoGitSnapshot::default()
    })
}

/// `git rev-parse --show-prefix` output as `""` or a normalized `"sub/dir/"`.
fn normalize_prefix(raw: &str) -> String {
    let norm = normalize_path(raw);
    let norm = norm.trim_end_matches('/');
    if norm.is_empty() || norm == "." {
        String::new()
    } else {
        format!("{norm}/")
    }
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

/// Parses NUL-delimited `git ls-tree -r -t -z` records (`<mode> <type> <sha>\t<path>`) into a
/// map keyed by the normalized path. The header is split from the path at the first `\t`
/// only (paths may contain tabs and spaces); malformed records are skipped. Submodules come
/// through as `commit` entries.
pub fn parse_ls_tree_z(bytes: &[u8]) -> BTreeMap<String, GitTreeEntry> {
    let mut entries = BTreeMap::new();
    for record in bytes.split(|&b| b == 0).filter(|s| !s.is_empty()) {
        let Some(tab) = record.iter().position(|&b| b == b'\t') else {
            continue;
        };
        let header = String::from_utf8_lossy(&record[..tab]);
        let mut fields = header.split(' ');
        let (Some(mode), Some(object_type), Some(sha), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if mode.is_empty() || object_type.is_empty() || !is_hex_sha(sha) {
            continue;
        }
        let path = normalize_path(String::from_utf8_lossy(&record[tab + 1..]).as_ref());
        if path.is_empty() {
            continue;
        }
        entries.insert(
            path,
            GitTreeEntry {
                mode: mode.to_string(),
                object_type: object_type.to_string(),
                sha: sha.to_string(),
            },
        );
    }
    entries
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
                ..RepoGitSnapshot::default()
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
    check_subsystem_staleness_cached(
        backend,
        repo_root,
        snapshot,
        paths,
        recorded_commit_sha,
        recorded_blob_sha,
        &mut DiffNameCache::new(),
        true,
    )
}

/// [`check_subsystem_staleness_with_snapshot`] sharing `git diff --name-only` results across
/// calls through `diffs` (one lookup per distinct recorded commit). With `want_diff_stat ==
/// false` no `git diff --stat` runs: a stale report then carries only the local summary
/// (changed files or blob change) in `diff_stat`.
#[allow(clippy::too_many_arguments)] // the shared caches travel with the card's provenance
pub fn check_subsystem_staleness_cached(
    backend: &dyn GitBackend,
    repo_root: &Path,
    snapshot: &RepoGitSnapshot,
    paths: &[String],
    recorded_commit_sha: &str,
    recorded_blob_sha: &str,
    diffs: &mut DiffNameCache,
    want_diff_stat: bool,
) -> Result<StalenessReport> {
    if snapshot.unborn_head {
        // Same as the per-card path, where `head_commit` fails on a repository without commits.
        bail!("HEAD is unborn in {:?}", repo_root);
    }
    let recorded_commit = recorded_commit_sha.trim().to_string();
    let recorded_blob = recorded_blob_sha.trim().to_string();
    let current_commit = snapshot.head_commit.clone();
    let current_blob = snapshot_paths_tree_sha(backend, repo_root, snapshot, paths).ok();

    let mut changed_files = Vec::new();
    let mut commit_lookup_failed = false;

    let recorded_commit_ok = recorded_commit.is_empty() || is_hex_sha(&recorded_commit);
    if !recorded_commit_ok {
        commit_lookup_failed = true;
    } else if !recorded_commit.is_empty() && recorded_commit != current_commit {
        let diff = diffs.entry(recorded_commit.clone()).or_insert_with(|| {
            backend
                .diff_name_only(repo_root, &recorded_commit, &current_commit)
                .ok()
        });
        match diff {
            Some(diff_files) => {
                for f in diff_files.iter() {
                    if paths.is_empty() || paths.iter().any(|dir| path_matches_directory(f, dir)) {
                        changed_files.push(f.clone());
                    }
                }
            }
            None => {
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
            .map(normalize_path)
            .filter(|p| !p.is_empty() && p != ".")
            .collect();

        let stat_from_git = if want_diff_stat
            && recorded_commit_ok
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
    check_frontmatter_staleness_cached(
        backend,
        repo_root,
        snapshot,
        frontmatter,
        &mut DiffNameCache::new(),
        true,
    )
}

/// [`check_frontmatter_staleness_with_snapshot`] sharing `git diff --name-only` results
/// across cards through `diffs`; `want_diff_stat` as in [`check_subsystem_staleness_cached`].
pub fn check_frontmatter_staleness_cached(
    backend: &dyn GitBackend,
    repo_root: &Path,
    snapshot: &RepoGitSnapshot,
    frontmatter: &CardFrontmatter,
    diffs: &mut DiffNameCache,
    want_diff_stat: bool,
) -> Result<StalenessReport> {
    let dir = normalize_path(&frontmatter.directory);
    let paths = if dir.is_empty() || dir == "." {
        Vec::new()
    } else {
        vec![dir]
    };
    check_subsystem_staleness_cached(
        backend,
        repo_root,
        snapshot,
        &paths,
        &frontmatter.source.commit_sha,
        &frontmatter.source.blob_sha,
        diffs,
        want_diff_stat,
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

    #[test]
    fn unparsed_tree_output_is_unborn_only_without_head() {
        let root = Path::new("/repo");
        let err = unparsed_tree_snapshot(root, Ok("a".repeat(40)), Vec::new());
        assert!(
            err.is_err(),
            "a repository with commits must not look unborn"
        );
        let unborn = unparsed_tree_snapshot(root, Err(anyhow::anyhow!("no HEAD")), Vec::new())
            .expect("unborn snapshot");
        assert!(unborn.unborn_head);
        assert!(unborn.head_commit.is_empty());
    }

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

    /// `git` isolated from the user's global/system config.
    fn git_q(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .arg("-C")
            .arg(dir)
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

    fn assert_parity(git: &SystemGitBackend, snap: &RepoGitSnapshot, root: &Path, rels: &[&str]) {
        for rel in rels {
            assert_eq!(
                snap.blob_sha_for(root, Path::new(rel)).unwrap(),
                git.blob_sha(root, Path::new(rel)).unwrap(),
                "blob_sha parity for {rel:?}"
            );
        }
        let path_sets: [&[&str]; 4] = [
            &[],
            &["src/a.rs"],
            &["src/a.rs", "src/sub/b.rs"],
            &["src", "src/new.rs"],
        ];
        for set in path_sets {
            let paths: Vec<String> = set.iter().map(|s| s.to_string()).collect();
            assert_eq!(
                snap.paths_tree_sha(root, &paths).unwrap(),
                git.paths_tree_sha(root, &paths).unwrap(),
                "paths_tree_sha parity for {paths:?}"
            );
        }
    }

    #[test]
    fn snapshot_tree_shas_match_blob_sha_on_dirs_root_dirty_files_and_unborn_head() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let rels = [
            ".",
            "src",
            "src/sub",
            "src/a.rs",
            "src/sub/b.rs",
            "src/new.rs",
            "missing",
            "src/missing.rs",
        ];
        git_q(root, &["init", "-q"]);
        std::fs::create_dir_all(root.join("src/sub")).unwrap();
        std::fs::write(root.join("src/a.rs"), "pub fn a() {}\n").unwrap();
        std::fs::write(root.join("src/sub/b.rs"), "pub fn b() {}\n").unwrap();
        let git = SystemGitBackend::new();

        // Unborn HEAD: the snapshot is still built.
        let snap = git.repo_snapshot(root).unwrap();
        assert!(snap.unborn_head);
        assert!(snap.root_tree_sha.is_none());
        assert!(snap.tree_entries.is_empty());
        assert_parity(&git, &snap, root, &rels);

        git_q(root, &["add", "-A"]);
        git_q(root, &["commit", "-q", "-m", "init"]);
        let snap = git.repo_snapshot(root).unwrap();
        assert!(!snap.unborn_head);
        assert!(snap.root_tree_sha.is_some());
        assert_eq!(
            snap.tree_entries.get("src").map(|e| e.object_type.as_str()),
            Some("tree")
        );
        assert_eq!(
            snap.tree_entries
                .get("src/sub/b.rs")
                .map(|e| e.object_type.as_str()),
            Some("blob")
        );
        assert_parity(&git, &snap, root, &rels);

        // Unstaged edit + untracked file: single files are hashed from the working tree.
        let committed_a = snap.tree_entries["src/a.rs"].sha.clone();
        std::fs::write(root.join("src/a.rs"), "pub fn a() { /* dirty */ }\n").unwrap();
        std::fs::write(root.join("src/new.rs"), "pub fn new() {}\n").unwrap();
        let snap = git.repo_snapshot(root).unwrap();
        let dirty_a = snap
            .blob_sha_for(root, Path::new("src/a.rs"))
            .unwrap()
            .unwrap();
        assert_ne!(dirty_a, committed_a);
        assert_parity(&git, &snap, root, &rels);

        // Deleted tracked file: gone for both.
        std::fs::remove_file(root.join("src/sub/b.rs")).unwrap();
        let snap = git.repo_snapshot(root).unwrap();
        assert_parity(&git, &snap, root, &rels);
    }

    #[test]
    fn snapshot_matches_blob_sha_for_project_nested_in_repo_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let proj = repo.join("proj");
        let rels = [
            ".",
            "src",
            "src/sub",
            "src/a.rs",
            "src/sub/b.rs",
            "src/new.rs",
            "missing",
        ];
        git_q(repo, &["init", "-q"]);
        std::fs::create_dir_all(proj.join("src/sub")).unwrap();
        std::fs::write(proj.join("src/a.rs"), "pub fn a() {}\n").unwrap();
        std::fs::write(proj.join("src/sub/b.rs"), "pub fn b() {}\n").unwrap();
        // A top-level `src` too: directory lookups must resolve from the same base as
        // `rev-parse HEAD:<path>` (the repository root).
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("src/x.rs"), "pub fn x() {}\n").unwrap();
        git_q(repo, &["add", "-A"]);
        git_q(repo, &["commit", "-q", "-m", "init"]);
        let git = SystemGitBackend::new();

        let snap = git.repo_snapshot(&proj).unwrap();
        assert!(snap.is_batched());
        assert_eq!(snap.repo_prefix, "proj/");
        assert_parity(&git, &snap, &proj, &rels);

        // Unstaged edit + untracked file inside the nested project.
        let clean_a = snap
            .blob_sha_for(&proj, Path::new("src/a.rs"))
            .unwrap()
            .unwrap();
        std::fs::write(proj.join("src/a.rs"), "pub fn a() { /* dirty */ }\n").unwrap();
        std::fs::write(proj.join("src/new.rs"), "pub fn new() {}\n").unwrap();
        let snap = git.repo_snapshot(&proj).unwrap();
        let dirty_a = snap
            .blob_sha_for(&proj, Path::new("src/a.rs"))
            .unwrap()
            .unwrap();
        assert_ne!(dirty_a, clean_a);
        assert_parity(&git, &snap, &proj, &rels);
    }

    #[test]
    fn snapshot_parses_ls_tree_with_submodules_and_tabs_safely() {
        let s1 = "1".repeat(40);
        let s2 = "2".repeat(40);
        let s3 = "3".repeat(40);
        let s4 = "4".repeat(40);
        let s5 = "5".repeat(40);
        let raw = format!(
            "040000 tree {s1}\tsrc\0\
             100644 blob {s2}\tsrc/a b.rs\0\
             160000 commit {s3}\tvendor/sub\0\
             100755 blob {s4}\tдок/файл с пробелом.md\0\
             100644 blob {s5}\tweird\tname.rs\0\
             garbage-without-tab\0\
             100644 blob\tno-sha\0"
        );
        let entries = parse_ls_tree_z(raw.as_bytes());
        assert_eq!(entries.len(), 5, "{entries:?}");
        assert_eq!(entries["src"].object_type, "tree");
        assert_eq!(entries["src"].sha, s1);
        assert_eq!(entries["src/a b.rs"].sha, s2);
        let sub = &entries["vendor/sub"];
        assert_eq!(
            (
                sub.mode.as_str(),
                sub.object_type.as_str(),
                sub.sha.as_str()
            ),
            ("160000", "commit", s3.as_str())
        );
        assert_eq!(entries["док/файл с пробелом.md"].mode, "100755");
        assert_eq!(entries["weird\tname.rs"].sha, s5);
    }
}
