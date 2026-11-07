//! Multi-repository workspace map (D6): child Git repositories, their branch and dirty state,
//! and symbols declared in more than one of them.

use crate::analysis::scope::ScopeFilter;
use crate::config::ScopeConfig;
use crate::git::GitBackend;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Cap on listed cross-project links.
const MAX_LINKS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkspaceKbStatus {
    Ok {
        subsystems: usize,
    },
    Drift {
        subsystems: usize,
        missing: usize,
        untracked: usize,
        git_stale: bool,
    },
    LegacyAgentsDir,
    Uninitialized,
    MissingPath,
}

impl WorkspaceKbStatus {
    pub fn display_label(&self) -> String {
        match self {
            Self::Ok { subsystems } => format!("Ok ({subsystems})"),
            Self::Drift {
                missing, untracked, ..
            } => format!("Drift (+{untracked}/-{missing})"),
            Self::LegacyAgentsDir => "LegacyAgentsDir".to_string(),
            Self::Uninitialized => "Uninitialized".to_string(),
            Self::MissingPath => "MissingPath".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRepo {
    pub name: String,
    pub path: String,
    pub branch: String,
    pub is_dirty: bool,
    pub dirty_files_count: usize,
    pub untracked_files_count: usize,
    pub kb_status: WorkspaceKbStatus,
    pub subsystems_count: usize,
    pub entrypoints: Vec<String>,
    pub key_commands: Vec<String>,
    /// Kept for backwards compatibility with earlier reports.
    #[serde(default)]
    pub dirty_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossLink {
    pub symbol: String,
    pub repos: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceReport {
    pub repos: Vec<WorkspaceRepo>,
    pub links: Vec<CrossLink>,
}

/// Code files read per repository when collecting declared symbols.
const MAX_FILES_PER_REPO: usize = 300;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const DECL_KEYWORDS: [&str; 7] = [
    "struct",
    "enum",
    "trait",
    "class",
    "interface",
    "record",
    "type",
];
const CODE_EXTS: [&str; 10] = [
    "rs", "ts", "tsx", "js", "py", "go", "java", "kt", "cs", "bsl",
];

/// CamelCase type names declared in `source` (`struct Foo`, `interface Foo`, ...).
fn declared_types(source: &str) -> BTreeSet<String> {
    let words: Vec<&str> = source
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect();
    words
        .windows(2)
        .filter(|w| DECL_KEYWORDS.contains(&w[0]))
        .map(|w| w[1])
        .filter(|n| n.len() >= 4 && n.chars().next().is_some_and(char::is_uppercase))
        .map(str::to_string)
        .collect()
}

fn repo_symbols(repo: &Path) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();
    let Ok(filter) = ScopeFilter::build(repo, &ScopeConfig::default()) else {
        return symbols;
    };
    let code = filter.files().iter().filter(|f| {
        Path::new(f)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| CODE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
    });
    for rel in code.take(MAX_FILES_PER_REPO) {
        let path = repo.join(rel);
        if fs::metadata(&path).map_or(true, |m| m.len() > MAX_FILE_BYTES) {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            symbols.extend(declared_types(&text));
        }
    }
    symbols
}

pub fn inspect_kb_default(path: &Path) -> WorkspaceKbStatus {
    if !path.exists() {
        return WorkspaceKbStatus::MissingPath;
    }
    let kd = path.join(".context-init/knowledge");
    let l0_json = kd.join("L0_index.json");
    if l0_json.is_file() {
        if let Ok(content) = fs::read_to_string(&l0_json) {
            if let Ok(idx) = serde_json::from_str::<serde_json::Value>(&content) {
                let count = idx
                    .get("subsystems")
                    .and_then(|s| s.as_array())
                    .map_or(0, |a| a.len());
                return WorkspaceKbStatus::Ok { subsystems: count };
            }
        }
        return WorkspaceKbStatus::Ok { subsystems: 0 };
    }
    if path.join(".agents/knowledge/L0_index.json").is_file() {
        return WorkspaceKbStatus::LegacyAgentsDir;
    }
    WorkspaceKbStatus::Uninitialized
}

/// Child directories of `workspace_root` that are Git repositories (symlinks followed, dot-dirs skipped,
/// nested subdirectories inspected 1 level deeper), with branch, dirty-file count and symbols declared
/// in two or more repositories.
pub fn analyze_workspace(workspace_root: &Path, git: &dyn GitBackend) -> Result<WorkspaceReport> {
    analyze_workspace_with(workspace_root, None, git, &inspect_kb_default)
}

pub fn analyze_workspace_with(
    workspace_root: &Path,
    explicit_paths: Option<&[std::path::PathBuf]>,
    git: &dyn GitBackend,
    inspect_kb: &(dyn Fn(&Path) -> WorkspaceKbStatus + Sync),
) -> Result<WorkspaceReport> {
    let mut dirs: Vec<(String, std::path::PathBuf, bool)> = Vec::new();

    if let Some(explicit) = explicit_paths {
        let mut targets: Vec<(std::path::PathBuf, bool)> = Vec::new();
        let mut seen_canonical = std::collections::HashSet::new();

        for p in explicit {
            let target_path = if p.is_absolute() {
                p.clone()
            } else {
                workspace_root.join(p)
            };
            if target_path.exists() {
                let canonical =
                    fs::canonicalize(&target_path).unwrap_or_else(|_| target_path.clone());
                if seen_canonical.insert(canonical) {
                    targets.push((target_path, true));
                }
            } else {
                targets.push((target_path, false));
            }
        }

        let mut basename_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (path, _) in &targets {
            let base = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "repo".to_string());
            *basename_counts.entry(base).or_insert(0) += 1;
        }

        for (path, exists) in targets {
            let base = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "repo".to_string());
            let name = if basename_counts.get(&base).copied().unwrap_or(0) > 1 {
                let parent = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if parent.is_empty() {
                    base
                } else {
                    format!("{parent}/{base}")
                }
            } else {
                base
            };
            dirs.push((name, path, exists));
        }
    } else {
        let entries = fs::read_dir(workspace_root)
            .with_context(|| format!("Cannot read workspace {workspace_root:?}"))?;

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || !path.is_dir() {
                continue;
            }
            if path.join(".git").exists() {
                dirs.push((name, path, true));
            } else if let Ok(sub_entries) = fs::read_dir(&path) {
                for sub_entry in sub_entries.flatten() {
                    let sub_path = sub_entry.path();
                    let sub_name = sub_entry.file_name().to_string_lossy().into_owned();
                    if sub_name.starts_with('.') || !sub_path.is_dir() {
                        continue;
                    }
                    if sub_path.join(".git").exists() {
                        dirs.push((format!("{name}/{sub_name}"), sub_path, true));
                    }
                }
            }
        }
    }

    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut repos_and_symbols = Vec::with_capacity(dirs.len());
    std::thread::scope(|s| {
        let handles: Vec<_> = dirs
            .iter()
            .map(|(name, path, exists)| {
                s.spawn(move || {
                    if !exists {
                        let repo = WorkspaceRepo {
                            name: name.clone(),
                            path: path.to_string_lossy().into_owned(),
                            branch: "-".to_string(),
                            is_dirty: false,
                            dirty_files_count: 0,
                            untracked_files_count: 0,
                            kb_status: WorkspaceKbStatus::MissingPath,
                            subsystems_count: 0,
                            entrypoints: Vec::new(),
                            key_commands: Vec::new(),
                            dirty_files: 0,
                        };
                        return (repo, BTreeSet::new());
                    }

                    let branch = git.current_branch(path).unwrap_or_else(|_| "-".to_string());
                    let dirty_files = git.status_porcelain_z(path).map_or(0, |s| s.len());
                    let is_dirty = dirty_files > 0;
                    let kb_status = inspect_kb(path);
                    let subsystems_count = match &kb_status {
                        WorkspaceKbStatus::Ok { subsystems }
                        | WorkspaceKbStatus::Drift { subsystems, .. } => *subsystems,
                        _ => 0,
                    };

                    let (entrypoints, key_commands) = if let Ok(content) =
                        fs::read_to_string(path.join(".context-init/knowledge/L0_index.json"))
                    {
                        if let Ok(idx) = serde_json::from_str::<serde_json::Value>(&content) {
                            let ep = idx
                                .get("entrypoints")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|x| x.as_str().map(str::to_string))
                                        .take(5)
                                        .collect()
                                })
                                .unwrap_or_default();
                            let cmd = idx
                                .get("commands")
                                .and_then(|v| v.as_object())
                                .map(|obj| {
                                    obj.iter()
                                        .map(|(k, v)| {
                                            format!("{}: {}", k, v.as_str().unwrap_or_default())
                                        })
                                        .take(5)
                                        .collect()
                                })
                                .unwrap_or_default();
                            (ep, cmd)
                        } else {
                            (Vec::new(), Vec::new())
                        }
                    } else {
                        (Vec::new(), Vec::new())
                    };

                    let symbols = repo_symbols(path);
                    let repo = WorkspaceRepo {
                        name: name.clone(),
                        path: path.to_string_lossy().into_owned(),
                        branch,
                        is_dirty,
                        dirty_files_count: dirty_files,
                        untracked_files_count: 0,
                        kb_status,
                        subsystems_count,
                        entrypoints,
                        key_commands,
                        dirty_files,
                    };
                    (repo, symbols)
                })
            })
            .collect();
        for h in handles {
            repos_and_symbols.push(h.join().unwrap());
        }
    });

    let mut repos = Vec::with_capacity(repos_and_symbols.len());
    let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (repo, symbols) in repos_and_symbols {
        let name = repo.name.clone();
        repos.push(repo);
        for sym in symbols {
            owners.entry(sym).or_default().push(name.clone());
        }
    }

    let links = owners
        .into_iter()
        .filter(|(_, repos)| repos.len() >= 2)
        .map(|(symbol, repos)| CrossLink { symbol, repos })
        .take(MAX_LINKS)
        .collect();
    Ok(WorkspaceReport { repos, links })
}

/// Workspace L0 map as Markdown.
pub fn render_workspace_l0(report: &WorkspaceReport) -> String {
    let mut md = format!(
        "# Workspace map (L0)\n\n- **Repositories:** {}\n\n",
        report.repos.len()
    );
    md.push_str("| Repository | Branch | Dirty | KB Status | Subsystems |\n| :--- | :--- | :--- | :--- | :--- |\n");
    for r in &report.repos {
        let dirty_str = if r.dirty_files_count > 0 {
            format!("{}", r.dirty_files_count)
        } else {
            "0".to_string()
        };
        md.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} |\n",
            r.name,
            r.branch,
            dirty_str,
            r.kb_status.display_label(),
            r.subsystems_count
        ));
    }
    if !report.links.is_empty() {
        md.push_str("\n## Cross-Repository Shared Symbols\n");
        for l in &report.links {
            md.push_str(&format!("- `{}`: {}\n", l.symbol, l.repos.join(", ")));
        }
    }
    md
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

    fn make_repo(parent: &Path, name: &str, branch: &str, files: &[(&str, &str)]) {
        let root = parent.join(name);
        fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", branch]);
        for (rel, text) in files {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "init"]);
    }

    #[test]
    fn workspace_discovers_child_git_repos_branches_dirty_counts_and_shared_symbols() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let ws = std::env::temp_dir().join(format!("ws_{}_{nanos}", std::process::id()));
        fs::create_dir_all(ws.join("not-a-repo")).unwrap();
        make_repo(
            &ws,
            "billing-service",
            "main",
            &[(
                "src/lib.rs",
                "pub struct OrderDto { pub id: u32 }\npub struct BillingOnly;\n",
            )],
        );
        make_repo(
            &ws,
            "gateway-service",
            "develop",
            &[(
                "src/api.ts",
                "export interface OrderDto { id: number }\nexport class GatewayOnly {}\n",
            )],
        );
        fs::write(ws.join("gateway-service/src/dirty.ts"), "x").unwrap();

        let report = analyze_workspace(&ws, &SystemGitBackend::new()).unwrap();
        let names: Vec<&str> = report.repos.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["billing-service", "gateway-service"], "{report:?}");
        assert_eq!(report.repos[0].branch, "main");
        assert_eq!(report.repos[1].branch, "develop");
        assert_eq!(
            (report.repos[0].dirty_files, report.repos[1].dirty_files),
            (0, 1)
        );
        assert_eq!(report.links.len(), 1, "{report:?}");
        assert_eq!(report.links[0].symbol, "OrderDto");
        assert_eq!(
            report.links[0].repos,
            ["billing-service", "gateway-service"]
        );
        let md = render_workspace_l0(&report);
        assert!(
            md.contains("billing-service") && md.contains("develop") && md.contains("OrderDto"),
            "{md}"
        );
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn workspace_follows_symlinks_and_scans_nested_repos_and_skips_dot_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();

        // 1. Direct repo
        make_repo(
            ws,
            "repo-a",
            "main",
            &[("src/lib.rs", "pub struct SharedType;\n")],
        );

        // 2. Symlinked repo
        let external = tmp.path().join("external_repo");
        make_repo(
            tmp.path(),
            "external_repo",
            "main",
            &[("src/lib.rs", "pub struct SharedType;\n")],
        );
        #[cfg(unix)]
        std::os::unix::fs::symlink(&external, ws.join("repo-symlink")).unwrap();

        // 3. Dot directory with repo (should be ignored)
        let dot_dir = ws.join(".hidden");
        make_repo(
            &dot_dir,
            "hidden-repo",
            "main",
            &[("src/lib.rs", "pub struct Hidden;\n")],
        );

        // 4. Nested container directory with child repos (e.g. wasmbridge-projects/sub-repo)
        let container = ws.join("group");
        fs::create_dir_all(&container).unwrap();
        make_repo(
            &container,
            "sub-repo",
            "main",
            &[("src/lib.rs", "pub struct SharedType;\n")],
        );

        let report = analyze_workspace(ws, &SystemGitBackend::new()).unwrap();
        let names: Vec<String> = report.repos.iter().map(|r| r.name.clone()).collect();
        assert!(names.contains(&"repo-a".to_string()));
        #[cfg(unix)]
        assert!(names.contains(&"repo-symlink".to_string()));
        assert!(!names.iter().any(|n| n.starts_with('.')));
        assert!(names.contains(&"group/sub-repo".to_string()));
    }
}
