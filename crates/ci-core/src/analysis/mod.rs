//! Deterministic project analysis pipeline: detect → (optional LLM enrichment in
//! `ci-llm`) → generate/update the L0–L2 knowledge base. Free of any LLM dependency.

pub mod detector;
pub mod generator;
pub mod pack;
pub mod subsystems;
pub mod types;

use std::path::{Path, PathBuf};

pub use types::{DiscoveredModule, McpCheckResult, ProjectAnalysis};

/// Where the active knowledge base lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeLocation {
    /// Central storage (`~/.local/share/context-init/projects/<id>/`, D1/D23).
    Central,
    /// Legacy in-repo `.agents/knowledge` (read via lazy fallback until migrated).
    LegacyInRepo,
}

/// Resolves the active knowledge dir for `root`: central storage if it holds an index,
/// else a legacy in-repo index (direct, nested under `.agents/knowledge`, or a root
/// `L0_index.json`), else central storage (where new knowledge bases are created).
pub fn resolve_knowledge_dir(root: &Path, central_kb: &Path) -> (PathBuf, KnowledgeLocation) {
    if central_kb.join("L0_index.json").is_file() {
        return (central_kb.to_path_buf(), KnowledgeLocation::Central);
    }
    let direct = root.join(".agents").join("knowledge");
    if direct.join("L0_index.json").is_file() {
        return (direct, KnowledgeLocation::LegacyInRepo);
    }
    if let Ok(entries) = std::fs::read_dir(&direct) {
        let mut nested: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.join("L0_index.json").is_file())
            .collect();
        nested.sort();
        if let Some(first) = nested.into_iter().next() {
            return (first, KnowledgeLocation::LegacyInRepo);
        }
    }
    if root.join("L0_index.json").is_file() {
        return (root.to_path_buf(), KnowledgeLocation::LegacyInRepo);
    }
    (central_kb.to_path_buf(), KnowledgeLocation::Central)
}

#[cfg(test)]
mod tests {
    use super::generator::{self, GenerateOptions};
    use super::*;
    use crate::storage::{validate_subsystem_id, ProjectCommands, ProjectType, SubsystemInfo};
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("ci_core_analysis_{tag}_{nanos}"));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn analysis_for(root: &Path) -> ProjectAnalysis {
        ProjectAnalysis {
            name: "demo".to_string(),
            root_dir: root.to_path_buf(),
            project_type: ProjectType::Rust,
            description: "Demo project".to_string(),
            detected_modules: Vec::new(),
            root_entrypoints: Vec::new(),
            commands: ProjectCommands::default(),
            workspace_members: Vec::new(),
            rules: Vec::new(),
        }
    }

    fn core_subsystem() -> SubsystemInfo {
        SubsystemInfo {
            id: "core".to_string(),
            name: "Core".to_string(),
            summary: "Core module".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec![],
            entrypoints: vec![],
            key_files: vec![],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 0,
            parent: None,
        }
    }

    #[test]
    fn generate_writes_to_given_knowledge_dir() {
        let root = temp_dir("gen_root");
        let kd = temp_dir("gen_kd").join("knowledge");
        let opts = GenerateOptions {
            force: false,
            dry_run: false,
            with_agents_md: true,
        };
        generator::generate(&analysis_for(&root), &[core_subsystem()], &kd, &opts).unwrap();
        assert!(kd.join("L0_index.json").is_file());
        assert!(kd.join("subsystems/core.md").is_file());
        assert!(!root.join(".agents").join("knowledge").exists());
        assert!(root.join("AGENTS.md").is_file());
        let index = crate::storage::L0Index::load_from_file(&kd.join("L0_index.json")).unwrap();
        assert_eq!(
            index.schema_version,
            crate::storage::CURRENT_L0_SCHEMA_VERSION
        );
    }

    #[test]
    fn sanitize_id_output_is_valid_subsystem_id() {
        let long = "a".repeat(100);
        for raw in [
            "src/Core",
            "Мой модуль",
            "--x",
            long.as_str(),
            "packages\\core",
        ] {
            let id = generator::sanitize_id(raw);
            assert!(validate_subsystem_id(&id).is_ok(), "{raw:?} -> {id:?}");
        }
        assert_eq!(generator::sanitize_id("src/detector"), "src_detector");
        assert_ne!(
            generator::sanitize_id("Мой модуль"),
            generator::sanitize_id("Другой модуль")
        );
    }

    #[test]
    fn resolve_knowledge_dir_prefers_central_then_legacy() {
        let root = temp_dir("resolve_root");
        let central = temp_dir("resolve_central").join("kb");

        let (dir, loc) = resolve_knowledge_dir(&root, &central);
        assert_eq!((dir, loc), (central.clone(), KnowledgeLocation::Central));

        let legacy = root.join(".agents").join("knowledge");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("L0_index.json"), "{}").unwrap();
        let (dir, loc) = resolve_knowledge_dir(&root, &central);
        assert_eq!((dir, loc), (legacy, KnowledgeLocation::LegacyInRepo));

        fs::create_dir_all(&central).unwrap();
        fs::write(central.join("L0_index.json"), "{}").unwrap();
        let (dir, loc) = resolve_knowledge_dir(&root, &central);
        assert_eq!((dir, loc), (central, KnowledgeLocation::Central));
    }
}
