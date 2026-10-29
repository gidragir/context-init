//! Cross-platform configuration hierarchy, path resolution, and binary detection (D10–D12, D16, D19, D28).

use anyhow::{bail, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Default language for generated knowledge base content.
pub const DEFAULT_LANGUAGE: &str = "ru";

/// Default hard ceiling of tokens per MCP response (D19).
pub const DEFAULT_MAX_RESPONSE_TOKENS: usize = 4000;

/// Default token size above which a module is split into per-module subsystems.
pub const DEFAULT_SPLIT_THRESHOLD_TOKENS: usize = 15_000;

/// Smallest `max_response_tokens` that fits a paginated reply's cursor footer.
pub const MIN_RESPONSE_TOKENS: usize = 64;

/// Default deterministic role fallback sentinel (D16).
pub const ROLE_NONE: &str = "none";

/// Cross-platform resolved paths for configuration, central L0–L2 storage, cache, and telemetry (D1, D10, D28).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StoragePaths {
    /// Primary global configuration path:
    /// `$XDG_CONFIG_HOME/context-init/config.toml` or `~/.config/context-init/config.toml`.
    pub global_config_path: PathBuf,
    /// Optional platform fallback global config path (e.g. `%APPDATA%\context-init\config.toml` on Windows).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_global_config_path: Option<PathBuf>,
    /// Project-level configuration path: `<repo>/.context-init.toml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_config_path: Option<PathBuf>,
    /// Base data directory: `$XDG_DATA_HOME/context-init` or `~/.local/share/context-init`.
    pub data_dir: PathBuf,
    /// Central L0–L2 projects root: `<data_dir>/projects`.
    pub projects_dir: PathBuf,
    /// Cache directory: `$XDG_CACHE_HOME/context-init` or `~/.cache/context-init`.
    pub cache_dir: PathBuf,
    /// Telemetry directory: `<data_dir>/telemetry`.
    pub telemetry_dir: PathBuf,
}

impl StoragePaths {
    /// Resolves storage and configuration paths using the current process environment and OS directories.
    pub fn resolve(repo_root: Option<&Path>) -> Self {
        Self::resolve_with_env(repo_root, None, |key| std::env::var(key).ok())
    }

    /// Resolves storage and configuration paths using an injected home directory and environment lookup.
    pub fn resolve_with_env<F>(
        repo_root: Option<&Path>,
        home_dir: Option<&Path>,
        get_env: F,
    ) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let non_empty_env = |name: &str| -> Option<PathBuf> {
            get_env(name)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };

        let non_empty_path = |p: PathBuf| -> Option<PathBuf> {
            if p.to_string_lossy().trim().is_empty() {
                None
            } else {
                Some(p)
            }
        };

        let effective_home = home_dir
            .map(Path::to_path_buf)
            .and_then(non_empty_path)
            .or_else(|| non_empty_env("HOME"))
            .or_else(|| non_empty_env("USERPROFILE"))
            .or_else(|| dirs_next::home_dir().and_then(non_empty_path))
            .unwrap_or_else(|| PathBuf::from("."));

        let appdata_config = non_empty_env("APPDATA")
            .or_else(|| {
                if cfg!(windows) {
                    dirs_next::config_dir()
                } else {
                    None
                }
            })
            .map(|dir| dir.join("context-init").join("config.toml"));

        let custom_cfg = non_empty_env("CONTEXT_INIT_CONFIG");
        let xdg_config = non_empty_env("XDG_CONFIG_HOME");
        let has_explicit_config_env = custom_cfg.is_some() || xdg_config.is_some();

        let global_config_path = if let Some(custom_cfg) = custom_cfg {
            custom_cfg
        } else if let Some(xdg_config) = xdg_config {
            xdg_config.join("context-init").join("config.toml")
        } else {
            effective_home
                .join(".config")
                .join("context-init")
                .join("config.toml")
        };

        let fallback_global_config_path = if has_explicit_config_env {
            None
        } else {
            appdata_config.filter(|p| p != &global_config_path)
        };

        let data_dir = if let Some(xdg_data) = non_empty_env("XDG_DATA_HOME") {
            xdg_data.join("context-init")
        } else {
            effective_home
                .join(".local")
                .join("share")
                .join("context-init")
        };

        let projects_dir = data_dir.join("projects");
        let telemetry_dir = data_dir.join("telemetry");

        let cache_dir = if let Some(xdg_cache) = non_empty_env("XDG_CACHE_HOME") {
            xdg_cache.join("context-init")
        } else {
            effective_home.join(".cache").join("context-init")
        };

        let project_config_path = repo_root.map(|root| root.join(".context-init.toml"));

        Self {
            global_config_path,
            fallback_global_config_path,
            project_config_path,
            data_dir,
            projects_dir,
            cache_dir,
            telemetry_dir,
        }
    }

    /// Returns the central L0–L2 storage directory for a given `project_id`:
    /// `<data_dir>/projects/<project-id>`.
    pub fn project_storage_dir(&self, project_id: &str) -> PathBuf {
        self.projects_dir.join(project_id)
    }

    /// Returns the central knowledge directory for a given `project_id` (D1, D21).
    pub fn project_knowledge_dir(&self, project_id: &str) -> PathBuf {
        self.project_storage_dir(project_id)
    }

    /// Returns the effective global config path to read from (falling back to `%APPDATA%` if primary does not exist).
    pub fn effective_global_config_path(&self) -> &Path {
        if !self.global_config_path.exists() {
            if let Some(ref fallback) = self.fallback_global_config_path {
                if fallback.exists() {
                    return fallback.as_path();
                }
            }
        }
        self.global_config_path.as_path()
    }

    /// Formats resolved paths for `context-init config path`.
    pub fn format_paths(&self, project_id: Option<&str>) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "global_config: {}\n",
            self.effective_global_config_path().display()
        ));
        if let Some(ref proj_cfg) = self.project_config_path {
            out.push_str(&format!("project_config: {}\n", proj_cfg.display()));
        }
        out.push_str(&format!("data_dir: {}\n", self.data_dir.display()));
        out.push_str(&format!("projects_dir: {}\n", self.projects_dir.display()));
        if let Some(pid) = project_id {
            out.push_str(&format!(
                "project_storage_dir: {}\n",
                self.project_storage_dir(pid).display()
            ));
        }
        out.push_str(&format!("cache_dir: {}\n", self.cache_dir.display()));
        out.push_str(&format!(
            "telemetry_dir: {}\n",
            self.telemetry_dir.display()
        ));
        out
    }
}

/// RTK terminal integration mode (D12).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RtkMode {
    /// Automatically enable RTK if `rtk` (or `rtk.exe`) is available in `PATH`.
    #[default]
    Auto,
    /// Force RTK prefixing on.
    On,
    /// Disable RTK prefixing.
    Off,
}

impl RtkMode {
    /// Returns string representation (`"auto"`, `"on"`, `"off"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }

    /// Resolves whether RTK is active given the current process `PATH`.
    pub fn is_active(&self) -> bool {
        resolve_rtk_active(*self)
    }

    /// Resolves whether RTK is active given an explicit `PATH` value.
    pub fn is_active_with_path(&self, path_var: Option<&OsStr>) -> bool {
        resolve_rtk_active_with_path(*self, path_var)
    }
}

impl fmt::Display for RtkMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RtkMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "on" | "true" | "1" => Ok(Self::On),
            "off" | "false" | "0" => Ok(Self::Off),
            other => bail!(
                "Invalid RtkMode '{}': expected 'auto', 'on', or 'off'",
                other
            ),
        }
    }
}

/// Where the L0–L2 knowledge base lives: inside the repository or in the central data dir.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    /// `<repo>/.context-init/knowledge/`, committed with the code.
    #[default]
    Repo,
    /// `~/.local/share/context-init/projects/<project-id>/`.
    Central,
}

impl StorageMode {
    /// Returns string representation (`"repo"`, `"central"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Repo => "repo",
            Self::Central => "central",
        }
    }
}

impl FromStr for StorageMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "repo" => Ok(Self::Repo),
            "central" => Ok(Self::Central),
            other => bail!("Invalid storage '{}': expected 'repo' or 'central'", other),
        }
    }
}

/// Checks whether a binary is available in the current process `PATH`, accounting for `.exe` on Windows (D12).
pub fn is_binary_available(bin_name: &str) -> bool {
    let path_var = std::env::var_os("PATH");
    is_binary_available_in_path(bin_name, path_var.as_deref())
}

/// Checks whether a binary is available in the provided `PATH` value, checking both `<bin>` and `<bin>.exe`.
pub fn is_binary_available_in_path(bin_name: &str, path_var: Option<&OsStr>) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };
    is_binary_available_in_dirs(bin_name, std::env::split_paths(path_var))
}

/// Checks whether a binary is available in an iterator of directories, accounting for `.exe` suffix.
pub fn is_binary_available_in_dirs<I, P>(bin_name: &str, dirs: I) -> bool
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let trimmed = bin_name.trim();
    if trimmed.is_empty() {
        return false;
    }

    let has_exe_suffix = trimmed.to_ascii_lowercase().ends_with(".exe");
    let exe_candidate_name = if has_exe_suffix {
        None
    } else {
        Some(format!("{trimmed}.exe"))
    };

    for dir in dirs {
        let dir_path = dir.as_ref();
        if dir_path.join(trimmed).is_file() {
            return true;
        }
        if let Some(ref exe_name) = exe_candidate_name {
            if dir_path.join(exe_name).is_file() {
                return true;
            }
        }
    }
    false
}

/// Convenience check for `rtk` binary availability in `PATH`.
pub fn is_rtk_available() -> bool {
    is_binary_available("rtk")
}

/// Resolves whether RTK integration is active based on `RtkMode` and `PATH` detection (D12).
pub fn resolve_rtk_active(mode: RtkMode) -> bool {
    let path_var = std::env::var_os("PATH");
    resolve_rtk_active_with_path(mode, path_var.as_deref())
}

/// Resolves whether RTK integration is active using an explicit `PATH` value.
pub fn resolve_rtk_active_with_path(mode: RtkMode, path_var: Option<&OsStr>) -> bool {
    match mode {
        RtkMode::On => true,
        RtkMode::Off => false,
        RtkMode::Auto => is_binary_available_in_path("rtk", path_var),
    }
}

/// External tool integrations configuration (`[integrations]`, D12).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationsConfig {
    /// RTK mode: `"auto"`, `"on"`, or `"off"`.
    #[serde(default)]
    pub rtk: RtkMode,
    /// Enable `mise.toml` task discovery/integration.
    #[serde(default)]
    pub mise: bool,
    /// Enable GNU Stow package topology detection.
    #[serde(default)]
    pub stow: bool,
}

impl IntegrationsConfig {
    /// Resolves whether RTK is active for this configuration in the current process `PATH`.
    pub fn resolve_rtk_active(&self) -> bool {
        resolve_rtk_active(self.rtk)
    }

    /// Resolves whether RTK is active for this configuration given an explicit `PATH`.
    pub fn resolve_rtk_active_with_path(&self, path_var: Option<&OsStr>) -> bool {
        resolve_rtk_active_with_path(self.rtk, path_var)
    }
}

/// Supported LLM provider kinds (`[models.<alias>]`, D14–D15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// Local or remote Ollama HTTP API (`D14`).
    Ollama,
    /// CLI command provider via stdin/stdout template (`claude -p`, `gemini -p`, `agy -p`, `codex exec`, `D14`).
    Command,
    /// Agent work-package delegation submitted via `context_submit` (`D14`).
    Agent,
    /// OpenAI-compatible HTTP endpoint extension point (`D15`).
    OpenaiCompat,
    /// Anthropic HTTP API extension point (`D15`).
    Anthropic,
    /// Gemini HTTP API extension point (`D15`).
    Gemini,
    /// MCP `sampling/createMessage` extension point (`D15`).
    Sampling,
}

impl ModelKind {
    /// Canonical string identifier for the model provider kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Command => "command",
            Self::Agent => "agent",
            Self::OpenaiCompat => "openai_compat",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::Sampling => "sampling",
        }
    }
}

impl fmt::Display for ModelKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ModelKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ollama" => Ok(Self::Ollama),
            "command" => Ok(Self::Command),
            "agent" => Ok(Self::Agent),
            "openai_compat" => Ok(Self::OpenaiCompat),
            "anthropic" => Ok(Self::Anthropic),
            "gemini" => Ok(Self::Gemini),
            "sampling" => Ok(Self::Sampling),
            other => bail!(
                "Unknown model kind '{}': expected one of ollama, command, agent, openai_compat, anthropic, gemini, sampling",
                other
            ),
        }
    }
}

/// Configuration for a named model alias under `[models.<alias>]` (D14–D16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelConfig {
    /// Provider kind (`ollama`, `command`, `agent`, `openai_compat`, `anthropic`, `gemini`, `sampling`).
    pub kind: ModelKind,
    /// Optional model identifier (e.g. `"qwen2.5:14b-ctx64k"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Optional HTTP endpoint URL (e.g. `"http://127.0.0.1:11434"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Optional CLI command and arguments for `kind = "command"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    /// Optional request timeout in milliseconds.
    #[serde(default, alias = "timeout", skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Optional maximum input token budget for this provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<usize>,
}

/// Execution role identifiers (D16).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum RoleKind {
    /// Initial knowledge base generation (`init`).
    Init,
    /// Incremental card refresh and staleness update (`update`).
    Update,
    /// Task digest synthesis (`task`).
    Task,
    /// Search query expansion and top-N reranking (`search`).
    Search,
}

impl RoleKind {
    /// Returns the canonical lowercase role name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Update => "update",
            Self::Task => "task",
            Self::Search => "search",
        }
    }
}

impl fmt::Display for RoleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RoleKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "init" => Ok(Self::Init),
            "update" => Ok(Self::Update),
            "task" => Ok(Self::Task),
            "search" => Ok(Self::Search),
            other => bail!(
                "Unknown role '{}': expected 'init', 'update', 'task', or 'search'",
                other
            ),
        }
    }
}

fn default_role_chain() -> Vec<String> {
    vec![ROLE_NONE.to_string()]
}

/// Role-to-model fallback chains (`[roles]`, D16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RolesConfig {
    /// Fallback chain of model aliases for `init` (`"none"` = deterministic mode).
    #[serde(default = "default_role_chain")]
    pub init: Vec<String>,
    /// Fallback chain of model aliases for `update`.
    #[serde(default = "default_role_chain")]
    pub update: Vec<String>,
    /// Fallback chain of model aliases for `task`.
    #[serde(default = "default_role_chain")]
    pub task: Vec<String>,
    /// Fallback chain of model aliases for `search`.
    #[serde(default = "default_role_chain")]
    pub search: Vec<String>,
}

impl Default for RolesConfig {
    fn default() -> Self {
        Self {
            init: default_role_chain(),
            update: default_role_chain(),
            task: default_role_chain(),
            search: default_role_chain(),
        }
    }
}

impl RolesConfig {
    /// Returns the fallback chain for a given `RoleKind`.
    pub fn get(&self, role: RoleKind) -> &[String] {
        match role {
            RoleKind::Init => &self.init,
            RoleKind::Update => &self.update,
            RoleKind::Task => &self.task,
            RoleKind::Search => &self.search,
        }
    }

    /// Returns the fallback chain for a role by name (`"init"`, `"update"`, `"task"`, `"search"`).
    pub fn get_by_name(&self, role: &str) -> Option<&[String]> {
        RoleKind::from_str(role).ok().map(|rk| self.get(rk))
    }

    /// Sets the fallback chain for a given `RoleKind`.
    pub fn set(&mut self, role: RoleKind, chain: Vec<String>) {
        match role {
            RoleKind::Init => self.init = chain,
            RoleKind::Update => self.update = chain,
            RoleKind::Task => self.task = chain,
            RoleKind::Search => self.search = chain,
        }
    }

    /// Sets the fallback chain for a role by name.
    pub fn set_by_name(&mut self, role: &str, chain: Vec<String>) -> Result<()> {
        let rk = RoleKind::from_str(role)?;
        self.set(rk, chain);
        Ok(())
    }

    /// Overrides all roles to use a single model alias (`--model <alias>`).
    pub fn set_all(&mut self, alias: &str) {
        let chain: Vec<String> = alias
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(str::to_string)
            .collect();
        self.init.clone_from(&chain);
        self.update.clone_from(&chain);
        self.task.clone_from(&chain);
        self.search = chain;
    }
}

/// Telemetry opt-in configuration (`[telemetry]`, D28).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TelemetryConfig {
    /// Opt-in telemetry recording flag (defaults to `false`).
    #[serde(default)]
    pub enabled: bool,
}

/// Command-line overrides with highest priority in the configuration cascade (D11, D16).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliOverrides {
    /// Override `language`.
    pub language: Option<String>,
    /// Override `max_response_tokens`.
    pub max_response_tokens: Option<usize>,
    /// Override `touch_agents_md`.
    pub touch_agents_md: Option<bool>,
    /// Override `[integrations].rtk`.
    pub rtk: Option<RtkMode>,
    /// Override `[integrations].mise`.
    pub mise: Option<bool>,
    /// Override `[integrations].stow`.
    pub stow: Option<bool>,
    /// Global `--model <alias>` override applied across all roles before role-specific overrides (D16).
    pub model: Option<String>,
    /// Role-specific overrides (`--role <role>=<alias[,alias2,...]>`, D16).
    pub roles: BTreeMap<String, Vec<String>>,
    /// Override `[telemetry].enabled`.
    pub telemetry_enabled: Option<bool>,
}

impl CliOverrides {
    /// Parses a `--role <role>=<alias[,alias2]>` specification into `(RoleKind, Vec<String>)`.
    pub fn parse_role_override(spec: &str) -> Result<(RoleKind, Vec<String>)> {
        let Some((role_str, aliases_str)) = spec.split_once('=') else {
            bail!(
                "Invalid role override '{}': expected '<role>=<alias>'",
                spec
            );
        };
        let role = RoleKind::from_str(role_str)?;
        let chain: Vec<String> = aliases_str
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        if chain.is_empty() {
            bail!(
                "Invalid role override '{}': model alias after '=' cannot be empty",
                spec
            );
        }
        Ok((role, chain))
    }

    /// Builder helper to set `--model <alias>`.
    pub fn with_model(mut self, alias: impl Into<String>) -> Self {
        self.model = Some(alias.into());
        self
    }

    /// Builder helper to parse and insert a `--role <role>=<alias>` override.
    pub fn with_role_override(mut self, spec: &str) -> Result<Self> {
        let (role, chain) = Self::parse_role_override(spec)?;
        self.roles.insert(role.as_str().to_string(), chain);
        Ok(self)
    }

    /// Sets a role override chain directly.
    pub fn set_role(&mut self, role: impl Into<String>, chain: Vec<String>) {
        self.roles.insert(role.into(), chain);
    }
}

fn default_language() -> String {
    DEFAULT_LANGUAGE.to_string()
}

fn default_max_response_tokens() -> usize {
    DEFAULT_MAX_RESPONSE_TOKENS
}

fn default_split_threshold_tokens() -> usize {
    DEFAULT_SPLIT_THRESHOLD_TOKENS
}

/// Complete resolved configuration for `context-init` (D1, D10–D12, D16, D19, D28).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Config {
    /// Target natural language for generated summaries and cards (default `"ru"`).
    #[serde(default = "default_language")]
    pub language: String,
    /// Hard upper bound on MCP response tokens (default `4000`, D19).
    #[serde(default = "default_max_response_tokens")]
    pub max_response_tokens: usize,
    /// Source size (BPE tokens) above which a module such as a workspace crate is split
    /// into per-module subsystems (default `15000`; `0` disables splitting).
    #[serde(default = "default_split_threshold_tokens")]
    pub split_threshold_tokens: usize,
    /// Whether to modify in-repo `AGENTS.md` by default (default `false`, D1).
    #[serde(default)]
    pub touch_agents_md: bool,
    /// Refresh a Git-stale L1 card on `context_get_subsystem` instead of only reporting it (default `false`, D2).
    #[serde(default)]
    pub auto_refresh: bool,
    /// File scope on top of `.gitignore` (`[scope]`, D9).
    #[serde(default)]
    pub scope: ScopeConfig,
    /// Knowledge base location (default `repo`).
    #[serde(default)]
    pub storage: StorageMode,
    /// External integrations configuration (`[integrations]`, D12).
    #[serde(default)]
    pub integrations: IntegrationsConfig,
    /// Named model provider definitions (`[models.<alias>]`, D14–D16).
    #[serde(default)]
    pub models: BTreeMap<String, ModelConfig>,
    /// Role fallback chains (`[roles]`, D16).
    #[serde(default)]
    pub roles: RolesConfig,
    /// Opt-in telemetry settings (`[telemetry]`, D28).
    #[serde(default)]
    pub telemetry: TelemetryConfig,
    /// Config files applied by `load` (not serialized).
    #[serde(skip)]
    #[schemars(skip)]
    pub loaded_sources: Vec<PathBuf>,
}

/// `[scope]`: which files `init`, `update`, `pack` and `scan` look at, on top of `.gitignore` (D9).
/// `include`/`exclude` are globs relative to the project root; `extensions` are file extensions
/// without the dot. Empty lists mean "no restriction".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ScopeConfig {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub extensions: Vec<String>,
}

impl ScopeConfig {
    /// `true` when no restriction is configured.
    pub fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty() && self.extensions.is_empty()
    }
}

/// Alias for `Config`.
pub type AppConfig = Config;

impl Default for Config {
    fn default() -> Self {
        Self {
            language: default_language(),
            max_response_tokens: default_max_response_tokens(),
            split_threshold_tokens: default_split_threshold_tokens(),
            touch_agents_md: false,
            auto_refresh: false,
            scope: ScopeConfig::default(),
            storage: StorageMode::default(),
            integrations: IntegrationsConfig::default(),
            models: BTreeMap::new(),
            roles: RolesConfig::default(),
            telemetry: TelemetryConfig::default(),
            loaded_sources: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialIntegrationsConfig {
    rtk: Option<RtkMode>,
    mise: Option<bool>,
    stow: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialRolesConfig {
    init: Option<Vec<String>>,
    update: Option<Vec<String>>,
    task: Option<Vec<String>>,
    search: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialTelemetryConfig {
    enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialModelConfig {
    kind: Option<ModelKind>,
    model: Option<String>,
    endpoint: Option<String>,
    command: Option<Vec<String>>,
    #[serde(alias = "timeout")]
    timeout_ms: Option<u64>,
    max_input_tokens: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialConfig {
    language: Option<String>,
    max_response_tokens: Option<usize>,
    split_threshold_tokens: Option<usize>,
    touch_agents_md: Option<bool>,
    auto_refresh: Option<bool>,
    scope: Option<ScopeConfig>,
    storage: Option<StorageMode>,
    integrations: Option<PartialIntegrationsConfig>,
    models: Option<BTreeMap<String, PartialModelConfig>>,
    roles: Option<PartialRolesConfig>,
    telemetry: Option<PartialTelemetryConfig>,
}

impl Config {
    /// Loads the configuration using the full priority cascade (D11):
    /// `CliOverrides` > `ENV (CONTEXT_INIT_*)` > `<repo>/.context-init.toml` > global `config.toml` > built-in defaults.
    pub fn load(repo_root: Option<&Path>, cli: &CliOverrides) -> Result<Self> {
        Self::load_with_env(repo_root, None, |k| std::env::var(k).ok(), cli)
    }

    /// Loads configuration from filesystem paths resolved via an injected environment and home directory.
    pub fn load_with_env<F>(
        repo_root: Option<&Path>,
        home_dir: Option<&Path>,
        get_env: F,
        cli: &CliOverrides,
    ) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let paths = StoragePaths::resolve_with_env(repo_root, home_dir, &get_env);
        let global_path = paths.effective_global_config_path();
        let global_toml = if global_path.is_file() {
            Some(
                fs::read_to_string(global_path)
                    .with_context(|| format!("Failed to read global config {:?}", global_path))?,
            )
        } else {
            None
        };

        let project_toml =
            if let Some(ref proj_path) = paths.project_config_path {
                if proj_path.is_file() {
                    Some(fs::read_to_string(proj_path).with_context(|| {
                        format!("Failed to read project config {:?}", proj_path)
                    })?)
                } else {
                    None
                }
            } else {
                None
            };

        let mut cfg = Self::from_layers(
            global_toml.as_deref(),
            project_toml.as_deref(),
            get_env,
            cli,
        )?;
        if global_toml.is_some() {
            cfg.loaded_sources.push(global_path.to_path_buf());
        }
        if let (Some(_), Some(path)) = (&project_toml, &paths.project_config_path) {
            cfg.loaded_sources.push(path.clone());
        }
        if cfg.max_response_tokens < MIN_RESPONSE_TOKENS {
            eprintln!(
                "context-init: warning: max_response_tokens = {} is below {MIN_RESPONSE_TOKENS}; replies may exceed it",
                cfg.max_response_tokens
            );
        }
        Ok(cfg)
    }

    /// Resolves configuration from raw TOML strings, an environment lookup function, and `CliOverrides` (D11).
    pub fn from_layers<F>(
        global_toml: Option<&str>,
        project_toml: Option<&str>,
        get_env: F,
        cli: &CliOverrides,
    ) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut cfg = Self::default();

        if let Some(gt) = global_toml {
            cfg.apply_toml_str(gt)
                .context("Failed to parse global config.toml")?;
        }

        if let Some(pt) = project_toml {
            cfg.apply_toml_str(pt)
                .context("Failed to parse project .context-init.toml")?;
        }

        cfg.apply_env(&get_env)?;
        cfg.apply_cli_overrides(cli)?;

        Ok(cfg)
    }

    /// Applies a partial TOML layer onto `self`.
    pub fn apply_toml_str(&mut self, toml_content: &str) -> Result<()> {
        let partial: PartialConfig = toml::from_str(toml_content)?;
        if let Some(lang) = partial.language {
            self.language = lang;
        }
        if let Some(max_tokens) = partial.max_response_tokens {
            self.max_response_tokens = max_tokens;
        }
        if let Some(threshold) = partial.split_threshold_tokens {
            self.split_threshold_tokens = threshold;
        }
        if let Some(touch) = partial.touch_agents_md {
            self.touch_agents_md = touch;
        }
        if let Some(refresh) = partial.auto_refresh {
            self.auto_refresh = refresh;
        }
        if let Some(scope) = partial.scope {
            self.scope = scope;
        }
        if let Some(storage) = partial.storage {
            self.storage = storage;
        }
        if let Some(integrations) = partial.integrations {
            if let Some(rtk) = integrations.rtk {
                self.integrations.rtk = rtk;
            }
            if let Some(mise) = integrations.mise {
                self.integrations.mise = mise;
            }
            if let Some(stow) = integrations.stow {
                self.integrations.stow = stow;
            }
        }
        if let Some(models) = partial.models {
            for (alias, pm) in models {
                if let Some(existing) = self.models.get_mut(&alias) {
                    if let Some(kind) = pm.kind {
                        existing.kind = kind;
                    }
                    if pm.model.is_some() {
                        existing.model = pm.model;
                    }
                    if pm.endpoint.is_some() {
                        existing.endpoint = pm.endpoint;
                    }
                    if pm.command.is_some() {
                        existing.command = pm.command;
                    }
                    if pm.timeout_ms.is_some() {
                        existing.timeout_ms = pm.timeout_ms;
                    }
                    if pm.max_input_tokens.is_some() {
                        existing.max_input_tokens = pm.max_input_tokens;
                    }
                } else {
                    let Some(kind) = pm.kind else {
                        bail!(
                            "Model alias '{}' in [models.{}] is missing required field 'kind'",
                            alias,
                            alias
                        );
                    };
                    self.models.insert(
                        alias,
                        ModelConfig {
                            kind,
                            model: pm.model,
                            endpoint: pm.endpoint,
                            command: pm.command,
                            timeout_ms: pm.timeout_ms,
                            max_input_tokens: pm.max_input_tokens,
                        },
                    );
                }
            }
        }
        if let Some(roles) = partial.roles {
            if let Some(init) = roles.init {
                self.roles.init = init;
            }
            if let Some(update) = roles.update {
                self.roles.update = update;
            }
            if let Some(task) = roles.task {
                self.roles.task = task;
            }
            if let Some(search) = roles.search {
                self.roles.search = search;
            }
        }
        if let Some(telemetry) = partial.telemetry {
            if let Some(enabled) = telemetry.enabled {
                self.telemetry.enabled = enabled;
            }
        }
        Ok(())
    }

    /// Applies `CONTEXT_INIT_*` environment variable overrides onto `self`.
    pub fn apply_env<F>(&mut self, get_env: &F) -> Result<()>
    where
        F: Fn(&str) -> Option<String>,
    {
        let non_empty = |key: &str| -> Option<String> {
            get_env(key)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        if let Some(lang) = non_empty("CONTEXT_INIT_LANGUAGE") {
            self.language = lang;
        }

        if let Some(tokens_str) = non_empty("CONTEXT_INIT_MAX_RESPONSE_TOKENS") {
            let tokens: usize = tokens_str.parse().with_context(|| {
                format!(
                    "Invalid CONTEXT_INIT_MAX_RESPONSE_TOKENS value '{}'",
                    tokens_str
                )
            })?;
            self.max_response_tokens = tokens;
        }

        if let Some(threshold_str) = non_empty("CONTEXT_INIT_SPLIT_THRESHOLD_TOKENS") {
            self.split_threshold_tokens = threshold_str.parse().with_context(|| {
                format!(
                    "Invalid CONTEXT_INIT_SPLIT_THRESHOLD_TOKENS value '{}'",
                    threshold_str
                )
            })?;
        }

        if let Some(refresh_str) = non_empty("CONTEXT_INIT_AUTO_REFRESH") {
            self.auto_refresh = parse_env_bool("CONTEXT_INIT_AUTO_REFRESH", &refresh_str)?;
        }

        if let Some(touch_str) = non_empty("CONTEXT_INIT_TOUCH_AGENTS_MD") {
            self.touch_agents_md = parse_env_bool("CONTEXT_INIT_TOUCH_AGENTS_MD", &touch_str)?;
        }

        if let Some(storage_str) = non_empty("CONTEXT_INIT_STORAGE") {
            self.storage = StorageMode::from_str(&storage_str)?;
        }

        if let Some(rtk_str) =
            non_empty("CONTEXT_INIT_RTK").or_else(|| non_empty("CONTEXT_INIT_INTEGRATIONS_RTK"))
        {
            self.integrations.rtk = RtkMode::from_str(&rtk_str)?;
        }

        if let Some(mise_str) =
            non_empty("CONTEXT_INIT_MISE").or_else(|| non_empty("CONTEXT_INIT_INTEGRATIONS_MISE"))
        {
            self.integrations.mise = parse_env_bool("CONTEXT_INIT_MISE", &mise_str)?;
        }

        if let Some(stow_str) =
            non_empty("CONTEXT_INIT_STOW").or_else(|| non_empty("CONTEXT_INIT_INTEGRATIONS_STOW"))
        {
            self.integrations.stow = parse_env_bool("CONTEXT_INIT_STOW", &stow_str)?;
        }

        if let Some(tel_str) = non_empty("CONTEXT_INIT_TELEMETRY_ENABLED")
            .or_else(|| non_empty("CONTEXT_INIT_TELEMETRY"))
        {
            self.telemetry.enabled = parse_env_bool("CONTEXT_INIT_TELEMETRY_ENABLED", &tel_str)?;
        }

        if let Some(model_alias) = non_empty("CONTEXT_INIT_MODEL") {
            self.roles.set_all(&model_alias);
        }

        for (env_key, role_kind) in [
            ("CONTEXT_INIT_ROLE_INIT", RoleKind::Init),
            ("CONTEXT_INIT_ROLE_UPDATE", RoleKind::Update),
            ("CONTEXT_INIT_ROLE_TASK", RoleKind::Task),
            ("CONTEXT_INIT_ROLE_SEARCH", RoleKind::Search),
        ] {
            if let Some(val) = non_empty(env_key) {
                let chain: Vec<String> = val
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();
                if !chain.is_empty() {
                    self.roles.set(role_kind, chain);
                }
            }
        }

        Ok(())
    }

    /// Applies `CliOverrides` onto `self` (highest priority in D11).
    pub fn apply_cli_overrides(&mut self, cli: &CliOverrides) -> Result<()> {
        if let Some(ref lang) = cli.language {
            self.language.clone_from(lang);
        }
        if let Some(max_tokens) = cli.max_response_tokens {
            self.max_response_tokens = max_tokens;
        }
        if let Some(touch) = cli.touch_agents_md {
            self.touch_agents_md = touch;
        }
        if let Some(rtk) = cli.rtk {
            self.integrations.rtk = rtk;
        }
        if let Some(mise) = cli.mise {
            self.integrations.mise = mise;
        }
        if let Some(stow) = cli.stow {
            self.integrations.stow = stow;
        }
        if let Some(ref model_alias) = cli.model {
            self.roles.set_all(model_alias);
        }
        for (role_name, chain) in &cli.roles {
            if chain.is_empty() {
                bail!(
                    "CLI role override for '{}' cannot have an empty chain",
                    role_name
                );
            }
            self.roles.set_by_name(role_name, chain.clone())?;
        }
        if let Some(tel) = cli.telemetry_enabled {
            self.telemetry.enabled = tel;
        }
        Ok(())
    }

    /// Validates configuration invariants; fails with every problem found by [`Config::lint`].
    pub fn validate(&self) -> Result<()> {
        let problems = self.lint();
        if !problems.is_empty() {
            bail!("Invalid configuration:\n- {}", problems.join("\n- "));
        }
        Ok(())
    }

    /// Lists every configuration problem (D11, D16, D19) without failing fast:
    /// `max_response_tokens >= MIN_RESPONSE_TOKENS`, non-empty `language`, `command` models
    /// with a command, `ollama` endpoints that are http(s) URLs, and role chains whose
    /// aliases are `none` or defined in `[models]`.
    pub fn lint(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.max_response_tokens < MIN_RESPONSE_TOKENS {
            problems.push(format!(
                "max_response_tokens = {} is below the minimum of {MIN_RESPONSE_TOKENS} (a paginated reply's footer alone needs ~50 tokens)",
                self.max_response_tokens
            ));
        }
        if self.language.trim().is_empty() {
            problems.push("language must not be empty".to_string());
        }
        for (alias, model_cfg) in &self.models {
            if model_cfg.kind == ModelKind::Command {
                let valid_cmd = model_cfg
                    .command
                    .as_ref()
                    .is_some_and(|cmd| !cmd.is_empty() && !cmd[0].trim().is_empty());
                if !valid_cmd {
                    problems.push(format!(
                        "model '{alias}' with kind = 'command' must specify a non-empty 'command' list"
                    ));
                }
            }
            if model_cfg.kind == ModelKind::Ollama {
                if let Some(endpoint) = &model_cfg.endpoint {
                    let host = endpoint
                        .strip_prefix("http://")
                        .or_else(|| endpoint.strip_prefix("https://"));
                    if host.is_none_or(|h| h.trim().is_empty()) {
                        problems.push(format!(
                            "model '{alias}': endpoint '{endpoint}' must be an http:// or https:// URL"
                        ));
                    }
                }
            }
        }
        for role in [
            RoleKind::Init,
            RoleKind::Update,
            RoleKind::Task,
            RoleKind::Search,
        ] {
            let chain = self.roles.get(role);
            if chain.is_empty() {
                problems.push(format!("role '{role}' fallback chain must not be empty"));
            }
            for alias in chain {
                let trimmed = alias.trim();
                if trimmed.is_empty() {
                    problems.push(format!("role '{role}' contains an empty model alias"));
                } else if trimmed != ROLE_NONE && !self.models.contains_key(trimmed) {
                    problems.push(format!(
                        "role '{role}' references unknown model alias '{trimmed}' (not in [models] and not 'none')"
                    ));
                }
            }
        }
        problems
    }

    /// Config files actually applied by [`Config::load`], lowest priority first.
    pub fn loaded_sources(&self) -> &[PathBuf] {
        &self.loaded_sources
    }

    /// Serializes the effective configuration as formatted TOML (`context-init config show`).
    pub fn show_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("Failed to serialize configuration to TOML")
    }

    /// Generates a documented template TOML configuration (`context-init config init`).
    pub fn default_template_toml() -> String {
        generate_template_toml()
    }

    /// Writes the template TOML configuration to `target_path` (`context-init config init`).
    pub fn init_config_file(target_path: &Path, force: bool) -> Result<PathBuf> {
        if target_path.exists() && !force {
            bail!(
                "Configuration file {:?} already exists (use force to overwrite)",
                target_path
            );
        }
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory {:?}", parent))?;
        }
        let template = generate_template_toml();
        fs::write(target_path, template)
            .with_context(|| format!("Failed to write config file {:?}", target_path))?;
        Ok(target_path.to_path_buf())
    }

    /// Resolves whether RTK is active for this config in the current process environment.
    pub fn resolve_rtk_active(&self) -> bool {
        self.integrations.resolve_rtk_active()
    }
}

/// Generates the standard documented TOML configuration template (`config init`).
pub fn generate_template_toml() -> String {
    r#"# context-init configuration file
language = "ru"
max_response_tokens = 4000
split_threshold_tokens = 15000
touch_agents_md = false
auto_refresh = false

[integrations]
rtk = "auto"
mise = true
stow = false

[models.local]
kind = "ollama"
model = "qwen2.5:14b-ctx64k"
endpoint = "http://127.0.0.1:11434"
timeout_ms = 60000
max_input_tokens = 32000

[roles]
init = ["local", "none"]
update = ["local", "none"]
task = ["local", "none"]
search = ["none"]

[telemetry]
enabled = false
"#
    .to_string()
}

fn parse_env_bool(var_name: &str, raw: &str) -> Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => bail!(
            "Invalid boolean value '{}' for environment variable {}",
            other,
            var_name
        ),
    }
}

/// Helper to build a platform `PATH` `OsString` from directory paths (useful for tests and cross-platform callers).
pub fn join_search_paths<I, P>(dirs: I) -> Result<OsString>
where
    I: IntoIterator<Item = P>,
    P: AsRef<OsStr>,
{
    std::env::join_paths(dirs).context("Failed to join search paths")
}

#[cfg(test)]
mod lint_tests {
    use super::*;

    fn cfg(toml: &str) -> Config {
        let mut c = Config::default();
        c.apply_toml_str(toml).unwrap();
        c
    }

    #[test]
    fn lint_clean_default_config() {
        assert!(Config::default().lint().is_empty());
    }

    #[test]
    fn lint_reports_each_rule() {
        let cases = [
            (
                "[roles]\ninit = [\"ghost\"]\n",
                "unknown model alias 'ghost'",
            ),
            ("[models.c]\nkind = \"command\"\n", "non-empty 'command'"),
            (
                "[models.o]\nkind = \"ollama\"\nendpoint = \"localhost:11434\"\n",
                "endpoint",
            ),
            ("max_response_tokens = 10\n", "64"),
            ("language = \" \"\n", "language"),
        ];
        for (toml, needle) in cases {
            let problems = cfg(toml).lint();
            assert_eq!(problems.len(), 1, "{toml:?} -> {problems:?}");
            assert!(problems[0].contains(needle), "{toml:?} -> {problems:?}");
        }
    }

    #[test]
    fn split_threshold_defaults_and_layers() {
        assert_eq!(
            Config::default().split_threshold_tokens,
            DEFAULT_SPLIT_THRESHOLD_TOKENS
        );
        assert_eq!(
            cfg("split_threshold_tokens = 0\n").split_threshold_tokens,
            0
        );
        let env = Config::from_layers(
            Some("split_threshold_tokens = 9000\n"),
            None,
            |k| (k == "CONTEXT_INIT_SPLIT_THRESHOLD_TOKENS").then(|| "20000".to_string()),
            &CliOverrides::default(),
        )
        .unwrap();
        assert_eq!(env.split_threshold_tokens, 20000);
    }

    #[test]
    fn lint_collects_all_problems() {
        let problems = cfg("max_response_tokens = 10\n[roles]\ninit = [\"ghost\"]\n").lint();
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    #[test]
    fn loaded_sources_lists_applied_files_in_order() {
        let dir = std::env::temp_dir().join(format!(
            "ci_cfg_sources_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = dir.join("home");
        let repo = dir.join("repo");
        fs::create_dir_all(home.join(".config/context-init")).unwrap();
        fs::create_dir_all(&repo).unwrap();
        fs::write(
            home.join(".config/context-init/config.toml"),
            "language = \"en\"\n",
        )
        .unwrap();
        fs::write(
            repo.join(".context-init.toml"),
            "max_response_tokens = 3000\n",
        )
        .unwrap();
        let c = Config::load_with_env(Some(&repo), Some(&home), |_| None, &CliOverrides::default())
            .unwrap();
        assert_eq!(c.loaded_sources().len(), 2, "{:?}", c.loaded_sources());
        assert!(c.loaded_sources()[1].ends_with(".context-init.toml"));
        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    #[test]
    fn storage_defaults_to_repo() {
        assert_eq!(Config::default().storage, StorageMode::Repo);
    }

    #[test]
    fn storage_from_project_toml() {
        let mut c = Config::default();
        c.apply_toml_str("storage = \"central\"").unwrap();
        assert_eq!(c.storage, StorageMode::Central);
    }

    #[test]
    fn storage_env_overrides_file() {
        let c = Config::from_layers(
            None,
            Some("storage = \"central\""),
            |k| (k == "CONTEXT_INIT_STORAGE").then(|| "repo".to_string()),
            &CliOverrides::default(),
        )
        .unwrap();
        assert_eq!(c.storage, StorageMode::Repo);
    }

    #[test]
    fn storage_invalid_value_errors() {
        let mut c = Config::default();
        assert!(c.apply_toml_str("storage = \"cloud\"").is_err());
        let err = Config::from_layers(
            None,
            None,
            |k| (k == "CONTEXT_INIT_STORAGE").then(|| "cloud".to_string()),
            &CliOverrides::default(),
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("cloud"));
    }
}
