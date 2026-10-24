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

/// Current `L0_index.json` schema version for central storage (D23).
pub const CURRENT_L0_SCHEMA_VERSION: u32 = 2;

/// Legacy `L0_index.json` schema version (when `schema_version` field is absent in old `.agents/knowledge/L0_index.json`, D23).
pub const LEGACY_L0_SCHEMA_VERSION: u32 = 1;

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
    #[serde(default)]
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

fn default_legacy_schema_version() -> u32 {
    LEGACY_L0_SCHEMA_VERSION
}

fn default_json_schema_uri() -> String {
    "https://json-schema.org/draft/2020-12/schema".to_string()
}

/// Root L0 index structure stored in `L0_index.json` with `schema_version` support (D23).
///
/// When deserializing a legacy `.agents/knowledge/L0_index.json` that lacks `schema_version`,
/// `schema_version` defaults to `LEGACY_L0_SCHEMA_VERSION` (`1`). New and migrated indexes use
/// `CURRENT_L0_SCHEMA_VERSION` (`2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct L0Index {
    #[serde(default = "default_json_schema_uri", rename = "$schema")]
    pub schema: String,
    /// Schema format version (`2` for central storage v0.3+, `1` when reading legacy format without `schema_version`).
    #[serde(default = "default_legacy_schema_version")]
    pub schema_version: u32,
    /// Optional computed `project-id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub project: String,
    #[serde(default)]
    pub description: String,
    pub root_path: String,
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
            root_path: String::new(),
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
    /// Parses `L0Index` from a JSON string (supports both v2 with `schema_version` and legacy v1 without `schema_version`).
    pub fn from_json_str(json: &str) -> Result<Self> {
        let normalized = normalize_line_endings(json);
        let idx: Self =
            serde_json::from_str(&normalized).context("Failed to deserialize L0_index.json")?;
        Ok(idx)
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

    /// Returns `true` if this index was loaded from a legacy format (`schema_version < 2`).
    pub fn is_legacy(&self) -> bool {
        self.schema_version < CURRENT_L0_SCHEMA_VERSION
    }

    /// Upgrades the index to `CURRENT_L0_SCHEMA_VERSION` (`2`) and normalizes all embedded paths to `/`.
    pub fn upgrade_to_current(&mut self) {
        self.schema_version = CURRENT_L0_SCHEMA_VERSION;
        if self.schema.is_empty() {
            self.schema = default_json_schema_uri();
        }
        self.root_path = normalize_path(&self.root_path);
        for ep in &mut self.entrypoints {
            *ep = normalize_path(&*ep);
        }
        for sub in &mut self.subsystems {
            sub.l1_path = normalize_path(&sub.l1_path);
            for ep in &mut sub.entrypoints {
                *ep = normalize_path(&*ep);
            }
            for kf in &mut sub.key_files {
                *kf = normalize_path(&*kf);
            }
            for f in &mut sub.files {
                f.path = normalize_path(&f.path);
            }
        }
    }
}

/// Result of auditing knowledge base drift against the filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriftReport {
    pub missing_in_fs: Vec<String>,
    pub untracked_in_knowledge: Vec<String>,
    pub up_to_date_count: usize,
}

/// Summary report returned by `migrate_in_repo_knowledge` (D23).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MigrationReport {
    /// Original in-repo `.agents/knowledge` directory path.
    pub source_dir: PathBuf,
    /// Target central storage directory path.
    pub target_dir: PathBuf,
    /// Schema version found in the source `L0_index.json` prior to migration.
    pub previous_schema_version: u32,
    /// Updated schema version written to the target `L0_index.json` (`2`).
    pub new_schema_version: u32,
    /// Relative paths (normalized with `/`) of all migrated files.
    pub migrated_files: Vec<String>,
    /// Number of subsystems in `L0_index.json`.
    pub subsystems_count: usize,
    /// Whether the old in-repo `.agents/knowledge` directory was removed after migration.
    pub removed_source: bool,
}

/// Migrates an in-repo `.agents/knowledge` directory to the central project storage directory,
/// upgrading `L0_index.json` to `CURRENT_L0_SCHEMA_VERSION` (`2`) and removing the old in-repo directory (D23).
pub fn migrate_in_repo_knowledge(
    repo_root: &Path,
    target_project_dir: &Path,
) -> Result<MigrationReport> {
    migrate_in_repo_knowledge_with_options(repo_root, target_project_dir, true)
}

/// Migrates in-repo `.agents/knowledge` to `target_project_dir` with control over whether to remove the source directory.
pub fn migrate_in_repo_knowledge_with_options(
    repo_root: &Path,
    target_project_dir: &Path,
    remove_source: bool,
) -> Result<MigrationReport> {
    let opts = MigrateOptions {
        remove_source,
        dry_run: false,
    };
    migrate_knowledge(repo_root, target_project_dir, &opts)
}

/// Options for [`migrate_knowledge`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MigrateOptions {
    /// Delete the in-repo source after a verified copy.
    pub remove_source: bool,
    /// List what would be migrated; write and delete nothing.
    pub dry_run: bool,
}

/// Finds the in-repo knowledge dir holding `L0_index.json`: `.agents/knowledge` itself
/// or its first (alphabetical) subdirectory that has one.
fn legacy_source_dir(repo_root: &Path) -> Result<PathBuf> {
    let direct = repo_root.join(".agents").join("knowledge");
    if direct.join("L0_index.json").is_file() {
        return Ok(direct);
    }
    let mut nested: Vec<PathBuf> = fs::read_dir(&direct)
        .with_context(|| format!("In-repo knowledge directory {:?} does not exist", direct))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("L0_index.json").is_file())
        .collect();
    nested.sort();
    nested
        .into_iter()
        .next()
        .with_context(|| format!("In-repo L0_index.json not found under {:?}", direct))
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

/// Copies every file under `src` that does not exist under `dst` (never overwrites).
/// Returns the copied relative paths. Used by `init` to keep hand-written files (e.g. ADRs)
/// of a legacy knowledge base that the generator does not produce.
pub fn copy_missing_files(src: &Path, dst: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    list_files_recursive(src, src, &mut files)?;
    files.sort();
    let mut copied = Vec::new();
    for rel in files {
        let target = dst.join(&rel);
        if target.exists() {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("Failed to create {:?}", parent))?;
        }
        fs::copy(src.join(&rel), &target)
            .with_context(|| format!("Failed to copy {} into {:?}", rel, dst))?;
        copied.push(rel);
    }
    Ok(copied)
}

/// Migrates a legacy in-repo knowledge base into `target_project_dir` (D23), upgrading
/// `L0_index.json` to `CURRENT_L0_SCHEMA_VERSION`. The source is removed only when
/// requested and only after every source file was verified in the target.
pub fn migrate_knowledge(
    repo_root: &Path,
    target_project_dir: &Path,
    opts: &MigrateOptions,
) -> Result<MigrationReport> {
    let source_dir = legacy_source_dir(repo_root)?;
    let mut l0_index = L0Index::load_from_file(&source_dir.join("L0_index.json"))?;
    let previous_schema_version = l0_index.schema_version;
    l0_index.upgrade_to_current();
    if l0_index.project_id.is_none() {
        if let Some(dir_name) = target_project_dir.file_name() {
            l0_index.project_id = Some(dir_name.to_string_lossy().to_string());
        }
    }

    let mut source_files = Vec::new();
    list_files_recursive(&source_dir, &source_dir, &mut source_files)?;
    source_files.sort();

    if opts.dry_run {
        return Ok(MigrationReport {
            source_dir,
            target_dir: target_project_dir.to_path_buf(),
            previous_schema_version,
            new_schema_version: l0_index.schema_version,
            migrated_files: source_files,
            subsystems_count: l0_index.subsystems.len(),
            removed_source: false,
        });
    }

    fs::create_dir_all(target_project_dir).with_context(|| {
        format!(
            "Failed to create target central storage directory {:?}",
            target_project_dir
        )
    })?;
    let mut migrated_files = Vec::new();
    copy_dir_recursive(
        &source_dir,
        &source_dir,
        target_project_dir,
        &mut migrated_files,
    )?;
    l0_index.save_to_file(&target_project_dir.join("L0_index.json"))?;
    migrated_files.sort();
    migrated_files.dedup();

    let mut removed_source = false;
    if opts.remove_source {
        let missing: Vec<&String> = source_files
            .iter()
            .filter(|rel| !target_project_dir.join(rel.as_str()).is_file())
            .collect();
        if !missing.is_empty() {
            bail!(
                "Refusing to remove {:?}: {} file(s) missing in {:?} after copy: {:?}",
                source_dir,
                missing.len(),
                target_project_dir,
                missing
            );
        }
        fs::remove_dir_all(&source_dir)
            .with_context(|| format!("Failed to remove migrated source {:?}", source_dir))?;
        removed_source = true;
        // Drop `.agents/knowledge` and `.agents` if the migration left them empty.
        for dir in [
            repo_root.join(".agents").join("knowledge"),
            repo_root.join(".agents"),
        ] {
            if fs::read_dir(&dir).is_ok_and(|mut e| e.next().is_none()) {
                let _ = fs::remove_dir(&dir);
            }
        }
    }

    Ok(MigrationReport {
        source_dir,
        target_dir: target_project_dir.to_path_buf(),
        previous_schema_version,
        new_schema_version: l0_index.schema_version,
        migrated_files,
        subsystems_count: l0_index.subsystems.len(),
        removed_source,
    })
}

fn copy_dir_recursive(
    root_src: &Path,
    current_src: &Path,
    root_dst: &Path,
    migrated_files: &mut Vec<String>,
) -> Result<()> {
    for entry in fs::read_dir(current_src)
        .with_context(|| format!("Failed to read directory {:?}", current_src))?
    {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root_src).unwrap_or(&path);
        let dst_path = root_dst.join(rel);

        if path.is_dir() {
            fs::create_dir_all(&dst_path)
                .with_context(|| format!("Failed to create directory {:?}", dst_path))?;
            copy_dir_recursive(root_src, &path, root_dst, migrated_files)?;
        } else if path.is_file() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &dst_path)
                .with_context(|| format!("Failed to copy {:?} to {:?}", path, dst_path))?;
            migrated_files.push(normalize_path(rel));
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
mod migrate_tests {
    use super::*;

    fn sandbox(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ci_migrate_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn seed(kd: &Path) {
        fs::create_dir_all(kd.join("subsystems")).unwrap();
        L0Index {
            project: "demo".to_string(),
            ..L0Index::default()
        }
        .save_to_file(&kd.join("L0_index.json"))
        .unwrap();
        fs::write(kd.join("subsystems/core.md"), "# Core\n").unwrap();
    }

    #[test]
    fn migrate_dry_run_writes_nothing() {
        let root = sandbox("dry");
        seed(&root.join(".agents/knowledge"));
        let target = root.join("central");
        let opts = MigrateOptions {
            remove_source: true,
            dry_run: true,
        };
        let report = migrate_knowledge(&root, &target, &opts).unwrap();
        assert!(!target.exists());
        assert!(root.join(".agents/knowledge/L0_index.json").is_file());
        assert!(report.migrated_files.contains(&"L0_index.json".to_string()));
        assert!(report
            .migrated_files
            .contains(&"subsystems/core.md".to_string()));
        assert!(!report.removed_source);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn migrate_nested_legacy_layout() {
        let root = sandbox("nested");
        seed(&root.join(".agents/knowledge/dotfiles"));
        let target = root.join("central");
        let report = migrate_knowledge(&root, &target, &MigrateOptions::default()).unwrap();
        assert!(target.join("L0_index.json").is_file());
        assert!(target.join("subsystems/core.md").is_file());
        assert!(report.source_dir.ends_with("dotfiles"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn remove_source_only_after_copy_verified() {
        let root = sandbox("remove");
        seed(&root.join(".agents/knowledge"));
        let target = root.join("central");
        let opts = MigrateOptions {
            remove_source: true,
            dry_run: false,
        };
        let report = migrate_knowledge(&root, &target, &opts).unwrap();
        assert!(report.removed_source);
        assert!(!root.join(".agents").exists());
        for rel in &report.migrated_files {
            assert!(target.join(rel).is_file(), "{rel} missing in target");
        }
        let _ = fs::remove_dir_all(&root);
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
