//! Types produced by the deterministic project analysis pipeline.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::analysis::cache::AnalyzedFileCache;
use crate::storage::{ProjectCommands, ProjectType, SubsystemFile};

/// A source module (directory) discovered by the detector.
#[derive(Debug, Clone)]
pub struct DiscoveredModule {
    /// Subsystem id: `sanitize_id(dir_path)`, or `<parent>_<module>` for a split-off module.
    pub id: String,
    /// Id of the module this one was split off from (oversized crates).
    pub parent: Option<String>,
    pub dir_path: String,
    pub name: String,
    pub files: Vec<SubsystemFile>,
    pub entrypoints: Vec<String>,
    pub symbols: Vec<String>,
    pub test_cmd: Option<String>,
}

/// Result of analyzing a project root (detector output, generator input).
#[derive(Debug, Clone)]
pub struct ProjectAnalysis {
    pub name: String,
    pub root_dir: PathBuf,
    pub project_type: ProjectType,
    pub description: String,
    pub detected_modules: Vec<DiscoveredModule>,
    pub root_entrypoints: Vec<String>,
    pub commands: ProjectCommands,
    pub workspace_members: Vec<String>,
    pub rules: Vec<String>,
    /// BPE tokens already counted during detection, by project-relative path. In-memory
    /// only (never serialized into `L0_index.json`); consumers count missing files themselves.
    pub detected_token_map: BTreeMap<String, usize>,
    /// The per-run metrics cache of the detection (token/line/symbol reuse downstream,
    /// with the disk cache when one was configured). In-memory only.
    pub file_cache: Option<Arc<AnalyzedFileCache>>,
}

/// Result of a knowledge base drift check (`check` / `context_check`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpCheckResult {
    /// Whether the knowledge base has drifted from the source tree.
    pub has_drift: bool,
    /// Subsystems present in the knowledge base but gone from disk.
    pub missing_in_fs: Vec<String>,
    /// Modules on disk that are missing from the knowledge base.
    pub untracked_in_knowledge: Vec<String>,
    /// Number of up-to-date subsystems.
    pub up_to_date_count: usize,
    /// One-line summary.
    pub message: String,
}
