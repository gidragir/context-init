//! Central storage primitives, `project-id` calculation, `L0Index` v2 schema, and in-repo migration (D1, D2, D23, D28).

pub use crate::config::StoragePaths;
pub use crate::openviking::{
    extract_l0_abstract, extract_l0_abstract_with_limit, CardFreshness, CardFrontmatter,
    CardGeneratedBy, CardSource, OkfCard, L0_ABSTRACT_MAX_CHARS,
};

use anyhow::{bail, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Current `L0_index.json` schema version (v3: no `root_path`, line or token counters).
pub const CURRENT_L0_SCHEMA_VERSION: u32 = 3;

/// Normalizes a path to use forward slashes (`/`) across all operating systems,
/// stripping Windows verbatim prefixes (`\\?\`) and redundant `./` or trailing slashes.
pub fn normalize_path(path: impl AsRef<Path>) -> String {
    let raw = path.as_ref().to_string_lossy();
    normalize_path_str(&raw)
}

/// Normalizes a raw path string to forward slashes (`/`).
pub fn normalize_path_str(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut replaced = trimmed.replace('\\', "/");

    // Strip Windows extended-length path prefix (`\\?\` -> `//?/`)
    if let Some(stripped) = replaced.strip_prefix("//?/") {
        replaced = stripped.to_string();
    }

    // Collapse multiple consecutive slashes into a single slash
    while replaced.contains("//") {
        replaced = replaced.replace("//", "/");
    }

    // Collapse redundant `/./` segments
    while replaced.contains("/./") {
        replaced = replaced.replace("/./", "/");
    }

    // Strip leading `./` unless the path is literally `"."`
    while let Some(stripped) = replaced.strip_prefix("./") {
        if stripped.is_empty() {
            replaced = ".".to_string();
            break;
        }
        replaced = stripped.to_string();
    }

    // Strip trailing `/.` unless the path is literally `"."`
    while replaced.len() > 2 && replaced.ends_with("/.") {
        replaced.truncate(replaced.len() - 2);
    }

    // Trim trailing `/` unless it is root `"/"` or drive root `"C:/"`
    if replaced.len() > 1 && replaced.ends_with('/') {
        let is_drive_root =
            replaced.len() == 3 && replaced.as_bytes()[1] == b':' && replaced.as_bytes()[2] == b'/';
        if !is_drive_root {
            replaced = replaced.trim_end_matches('/').to_string();
        }
    }

    replaced
}

/// Normalizes CRLF (`\r\n`) and lone CR (`\r`) line endings to LF (`\n`).
pub fn normalize_line_endings(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Normalizes a Git remote URL by stripping protocol schemes, credentials (`user:token@`),
/// port numbers, trailing slashes, and `.git` suffixes (D1).
pub fn normalize_git_remote(remote_url: &str) -> String {
    let mut s = remote_url.trim().replace('\\', "/");
    if s.is_empty() {
        return String::new();
    }

    // Strip scheme (`https://`, `http://`, `ssh://`, `git://`, `file://`)
    let mut had_scheme = false;
    for scheme in ["https://", "http://", "ssh://", "git://", "file://"] {
        if s.len() >= scheme.len() && s[..scheme.len()].eq_ignore_ascii_case(scheme) {
            s = s[scheme.len()..].to_string();
            had_scheme = true;
            break;
        }
    }

    // Strip credentials before `@` in the authority segment (before first `/` for URLs, or first `/` / `:` for SCP)
    let authority_end = if had_scheme {
        s.find('/').unwrap_or(s.len())
    } else {
        s.find(['/', ':']).unwrap_or(s.len())
    };
    if let Some(at_pos) = s[..authority_end].rfind('@') {
        s = s[at_pos + 1..].to_string();
    }

    // Handle SCP-style `host:owner/repo` vs `host:port/owner/repo`
    if let Some(colon_pos) = s.find(':') {
        let slash_pos = s.find('/');
        if slash_pos.is_none() || colon_pos < slash_pos.unwrap() {
            let after_colon = &s[colon_pos + 1..];
            if let Some(rel_slash) = after_colon.find('/') {
                let between = &after_colon[..rel_slash];
                if had_scheme && !between.is_empty() && between.chars().all(|c| c.is_ascii_digit())
                {
                    // `host:2222/owner/repo` -> strip `:2222`
                    s = format!("{}{}", &s[..colon_pos], &after_colon[rel_slash..]);
                } else {
                    // `host:owner/repo` -> replace `:` with `/`
                    s = format!("{}/{}", &s[..colon_pos], after_colon);
                }
            } else if had_scheme
                && !after_colon.is_empty()
                && after_colon.chars().all(|c| c.is_ascii_digit())
            {
                s = s[..colon_pos].to_string();
            } else {
                s = format!("{}/{}", &s[..colon_pos], after_colon);
            }
        }
    }

    s = s.trim_matches('/').to_string();

    // Strip trailing `.git` (case-insensitive)
    if s.len() > 4 && s[s.len() - 4..].eq_ignore_ascii_case(".git") {
        s.truncate(s.len() - 4);
    }

    s = s.trim_matches('/').to_ascii_lowercase();

    while s.contains("//") {
        s = s.replace("//", "/");
    }

    s
}

/// Computes a deterministic, filesystem-safe `project-id` combining the normalized Git remote
/// (without credentials or `.git`) and a short SHA-256 hash of the canonical project path (D1).
pub fn compute_project_id(project_path: &Path, remote_url: Option<&str>) -> String {
    let canonical = fs::canonicalize(project_path).unwrap_or_else(|_| project_path.to_path_buf());
    let norm_path = normalize_path(&canonical);

    let mut hasher = Sha256::new();
    hasher.update(norm_path.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    let short_hash = &digest[..8];

    let prefix_slug = remote_url
        .map(normalize_git_remote)
        .filter(|r| !r.is_empty())
        .map(|r| sanitize_slug(&r))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let fallback_name = canonical
                .file_name()
                .or_else(|| project_path.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".to_string());
            let s = sanitize_slug(&fallback_name.to_ascii_lowercase());
            if s.is_empty() {
                "project".to_string()
            } else {
                s
            }
        });

    format!("{}-{}", prefix_slug, short_hash)
}

/// Anonymizes a `project-id` using SHA-256 hex digest for telemetry (D28).
pub fn hash_project_id(project_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_id.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn sanitize_slug(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_sep = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
            out.push(ch);
            last_sep = false;
        } else if !last_sep && !out.is_empty() {
            out.push('_');
            last_sep = true;
        }
    }
    out.trim_matches(|c| c == '_' || c == '-' || c == '.')
        .to_string()
}

/// Project type classification detected by marker files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProjectType {
    Rust,
    TypeScript,
    Python,
    Go,
    DevOps,
    Generic,
}

impl ProjectType {
    /// Human-readable display label for the project type.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust (Cargo)",
            Self::TypeScript => "TypeScript / JavaScript (Node/Bun/Deno)",
            Self::Python => "Python (uv/pip/poetry)",
            Self::Go => "Go (Go Modules)",
            Self::DevOps => "Infrastructure / DevOps (Docker/K8s/Ansible)",
            Self::Generic => "Generic / Multi-language",
        }
    }
}

/// File entry within a subsystem (`L0_index.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SubsystemFile {
    pub path: String,
    pub is_entrypoint: bool,
    /// Line count; in memory only (used to pick representative files), never serialized.
    #[serde(skip)]
    #[schemars(skip)]
    pub lines: usize,
}

/// Subsystem metadata stored in `L0_index.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SubsystemInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub l1_path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SubsystemFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_symbols: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_command: Option<String>,
    /// BPE token count of the subsystem sources; in memory only (split decisions), never
    /// serialized: consumers compute it live with `analysis::subsystems::subsystem_tokens`.
    #[serde(skip)]
    #[schemars(skip)]
    pub tokens: usize,
    /// Id of the subsystem this one was split off from (oversized crates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Allowed subsystem id shape. Ids become file names (`subsystems/<id>.md`), so they
/// must never contain path separators or `..`.
pub const SUBSYSTEM_ID_PATTERN: &str = "^[a-z0-9][a-z0-9_-]{0,63}$";

/// Validates `id` against [`SUBSYSTEM_ID_PATTERN`].
pub fn validate_subsystem_id(id: &str) -> Result<()> {
    let bytes = id.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-');
    if !valid {
        bail!(
            "Invalid subsystem id {:?}: must match {}",
            id,
            SUBSYSTEM_ID_PATTERN
        );
    }
    Ok(())
}

/// Returns `<knowledge_dir>/subsystems/<id>.md` after validating `id`.
pub fn subsystem_card_path(knowledge_dir: &Path, id: &str) -> Result<PathBuf> {
    validate_subsystem_id(id)?;
    let path = knowledge_dir.join("subsystems").join(format!("{id}.md"));
    if !path.starts_with(knowledge_dir) {
        bail!("Subsystem card path {:?} escapes knowledge dir", path);
    }
    Ok(path)
}

/// Joins an index-supplied relative path onto `base`, rejecting absolute paths and any
/// `..` (index files may come from an untrusted cloned repository).
pub fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    use std::path::Component;
    let rel_path = Path::new(rel);
    let ok = !rel.is_empty()
        && rel_path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    ok.then(|| base.join(rel_path))
}

/// Current UTC time as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil-from-days (Howard Hinnant), valid for all dates after 1970.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Discovered build, test, and lint commands for a project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectCommands {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fmt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom: HashMap<String, String>,
}

fn default_json_schema_uri() -> String {
    "https://json-schema.org/draft/2020-12/schema".to_string()
}

/// Root L0 index structure stored in `L0_index.json` (D23). Only
/// `CURRENT_L0_SCHEMA_VERSION` is readable; older bases must be rebuilt with `init --force`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct L0Index {
    #[serde(default = "default_json_schema_uri", rename = "$schema")]
    pub schema: String,
    /// Schema format version (`CURRENT_L0_SCHEMA_VERSION`).
    pub schema_version: u32,
    /// Optional computed `project-id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub project: String,
    #[serde(default)]
    pub description: String,
    pub updated_at: String,
    #[serde(default)]
    pub project_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoints: Vec<String>,
    #[serde(default)]
    pub commands: ProjectCommands,
    #[serde(default)]
    pub subsystems: Vec<SubsystemInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub core_rules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stow_packages: Vec<String>,
}

impl Default for L0Index {
    fn default() -> Self {
        Self {
            schema: default_json_schema_uri(),
            schema_version: CURRENT_L0_SCHEMA_VERSION,
            project_id: None,
            project: String::new(),
            description: String::new(),
            updated_at: String::new(),
            project_type: String::new(),
            entrypoints: Vec::new(),
            commands: ProjectCommands::default(),
            subsystems: Vec::new(),
            core_rules: Vec::new(),
            stow_packages: Vec::new(),
        }
    }
}

impl L0Index {
    /// Parses `L0Index` from a JSON string; any `schema_version` other than the current one
    /// (a missing field counts as `1`) is an error.
    pub fn from_json_str(json: &str) -> Result<Self> {
        let normalized = normalize_line_endings(json);
        let value: serde_json::Value =
            serde_json::from_str(&normalized).context("Failed to deserialize L0_index.json")?;
        let found = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        if found != u64::from(CURRENT_L0_SCHEMA_VERSION) {
            bail!(
                "knowledge base uses an outdated format (schema v{found}); rebuild it with `context-init init --force`"
            );
        }
        serde_json::from_value(value).context("Failed to deserialize L0_index.json")
    }

    /// Loads `L0Index` from a file on disk.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("Failed to read L0_index.json at {:?}", path))?;
        Self::from_json_str(&raw)
    }

    /// Serializes `L0Index` to pretty JSON with a single trailing newline.
    pub fn to_json_pretty(&self) -> Result<String> {
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize L0_index.json")?;
        Ok(format!("{json}\n"))
    }

    /// Writes `L0Index` to `path`, creating parent directories if needed.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }
        let content = self.to_json_pretty()?;
        fs::write(path, content)
            .with_context(|| format!("Failed to write L0_index.json to {:?}", path))
    }
}

/// Result of auditing knowledge base drift against the filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriftReport {
    pub missing_in_fs: Vec<String>,
    pub untracked_in_knowledge: Vec<String>,
    pub up_to_date_count: usize,
}

/// Options for [`copy_knowledge`].
#[derive(Debug, Clone, Copy, Default)]
pub struct CopyKbOptions {
    /// List what would be copied; write and delete nothing.
    pub dry_run: bool,
    /// Delete `src` after the copy was verified.
    pub remove_source: bool,
}

/// Result of [`copy_knowledge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyKbReport {
    /// Relative paths (`/`-separated, sorted) of every file in the source base.
    pub files: Vec<String>,
    /// Number of subsystems in the source `L0_index.json`.
    pub subsystems_count: usize,
    /// Whether `src` was removed.
    pub removed_source: bool,
}

/// Copies a whole knowledge base (hand-written files such as ADRs included) from `src` to
/// `dst`. `src` must hold a loadable `L0_index.json`; the copy is verified by loading
/// `dst/L0_index.json`, and `src` is removed only after that and only on request.
pub fn copy_knowledge(src: &Path, dst: &Path, opts: &CopyKbOptions) -> Result<CopyKbReport> {
    let src_index = src.join("L0_index.json");
    if !src_index.is_file() {
        bail!("Knowledge base not found: {:?} does not exist", src_index);
    }
    let index = L0Index::load_from_file(&src_index)?;
    let mut files = Vec::new();
    list_files_recursive(src, src, &mut files)?;
    files.sort();

    let mut report = CopyKbReport {
        files,
        subsystems_count: index.subsystems.len(),
        removed_source: false,
    };
    if opts.dry_run {
        return Ok(report);
    }

    for rel in &report.files {
        let target = dst.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("Failed to create {:?}", parent))?;
        }
        fs::copy(src.join(rel), &target)
            .with_context(|| format!("Failed to copy {} into {:?}", rel, dst))?;
    }
    L0Index::load_from_file(&dst.join("L0_index.json"))
        .with_context(|| format!("Copied knowledge base at {:?} failed verification", dst))?;

    if opts.remove_source {
        if let Some(missing) = report.files.iter().find(|rel| !dst.join(rel).is_file()) {
            bail!(
                "Refusing to remove {:?}: {} missing in {:?}",
                src,
                missing,
                dst
            );
        }
        fs::remove_dir_all(src).with_context(|| format!("Failed to remove {:?}", src))?;
        report.removed_source = true;
    }
    Ok(report)
}

fn list_files_recursive(root: &Path, current: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(current).with_context(|| format!("Failed to read {:?}", current))? {
        let path = entry?.path();
        if path.is_dir() {
            list_files_recursive(root, &path, out)?;
        } else if path.is_file() {
            out.push(normalize_path(path.strip_prefix(root).unwrap_or(&path)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod subsystem_id_tests {
    use super::*;

    #[test]
    fn subsystem_id_validation() {
        for ok in ["core", "src_mcp", "a-1"] {
            assert!(validate_subsystem_id(ok).is_ok(), "{ok} must be valid");
        }
        let long = "x".repeat(65);
        for bad in ["", "../x", "/tmp/x", "a/b", "A", long.as_str()] {
            assert!(
                validate_subsystem_id(bad).is_err(),
                "{bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn subsystem_card_path_stays_inside_knowledge_dir() {
        let kd = Path::new("/kb");
        assert_eq!(
            subsystem_card_path(kd, "core").unwrap(),
            PathBuf::from("/kb/subsystems/core.md")
        );
        assert!(subsystem_card_path(kd, "../escape").is_err());
    }

    #[test]
    fn now_rfc3339_has_utc_shape() {
        let ts = now_rfc3339();
        assert_eq!(ts.len(), 20, "{ts}");
        assert!(ts.ends_with('Z') && ts.as_bytes()[10] == b'T', "{ts}");
    }
}

#[cfg(test)]
mod schema_tests {
    use super::*;

    #[test]
    fn l0_old_schema_is_rejected() {
        let err = L0Index::from_json_str(
            r#"{"schema_version": 2, "project": "p", "updated_at": "2026-01-01"}"#,
        )
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("schema v2"), "{msg}");
        assert!(msg.contains("init --force"), "{msg}");

        let missing =
            L0Index::from_json_str(r#"{"project": "p", "updated_at": "2026-01-01"}"#).unwrap_err();
        assert!(format!("{missing:#}").contains("schema v1"));
    }
}

#[cfg(test)]
mod copy_kb_tests {
    use super::*;

    fn sandbox(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ci_copy_kb_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seed(kd: &Path) {
        fs::create_dir_all(kd.join("adr")).unwrap();
        L0Index {
            project: "p".to_string(),
            ..L0Index::default()
        }
        .save_to_file(&kd.join("L0_index.json"))
        .unwrap();
        fs::write(kd.join("adr/x.md"), "# ADR x\n").unwrap();
    }

    #[test]
    fn copy_knowledge_copies_and_verifies() {
        let root = sandbox("copy");
        seed(&root.join("src"));
        let report = copy_knowledge(
            &root.join("src"),
            &root.join("dst"),
            &CopyKbOptions::default(),
        )
        .unwrap();
        assert_eq!(report.files, vec!["L0_index.json", "adr/x.md"]);
        assert!(!report.removed_source);
        assert_eq!(
            fs::read_to_string(root.join("dst/adr/x.md")).unwrap(),
            "# ADR x\n"
        );
        assert!(root.join("src/L0_index.json").is_file());
    }

    #[test]
    fn copy_knowledge_rejects_missing_index() {
        let root = sandbox("missing");
        fs::create_dir_all(root.join("src")).unwrap();
        let err = copy_knowledge(
            &root.join("src"),
            &root.join("dst"),
            &CopyKbOptions::default(),
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("L0_index.json"), "{err:#}");
    }

    #[test]
    fn copy_knowledge_dry_run_writes_nothing() {
        let root = sandbox("dry");
        seed(&root.join("src"));
        let report = copy_knowledge(
            &root.join("src"),
            &root.join("dst"),
            &CopyKbOptions {
                dry_run: true,
                remove_source: true,
            },
        )
        .unwrap();
        assert_eq!(report.files.len(), 2);
        assert!(!root.join("dst").exists());
        assert!(root.join("src/L0_index.json").is_file());
    }

    #[test]
    fn copy_knowledge_remove_source() {
        let root = sandbox("rm");
        seed(&root.join("src"));
        let report = copy_knowledge(
            &root.join("src"),
            &root.join("dst"),
            &CopyKbOptions {
                dry_run: false,
                remove_source: true,
            },
        )
        .unwrap();
        assert!(report.removed_source);
        assert!(!root.join("src").exists());
        assert!(root.join("dst/L0_index.json").is_file());
    }
}

#[cfg(test)]
mod safe_join_tests {
    use super::*;

    #[test]
    fn safe_join_rejects_escaping_paths() {
        let base = Path::new("/base");
        assert_eq!(
            safe_join(base, "src/a.rs"),
            Some(PathBuf::from("/base/src/a.rs"))
        );
        assert_eq!(
            safe_join(base, "./a.md"),
            Some(PathBuf::from("/base/./a.md"))
        );
        for bad in ["/etc/passwd", "../x", "a/../../x", "a/..", ""] {
            assert_eq!(safe_join(base, bad), None, "{bad:?}");
        }
    }
}
