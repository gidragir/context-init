//! Types produced by the deterministic project analysis pipeline.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::storage::{ProjectCommands, ProjectType, SubsystemFile};

/// A source module (directory) discovered by the detector.
#[derive(Debug, Clone)]
pub struct DiscoveredModule {
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
