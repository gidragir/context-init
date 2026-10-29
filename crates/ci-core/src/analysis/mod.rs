//! Deterministic project analysis pipeline: detect → (optional LLM enrichment in
//! `ci-llm`) → generate/update the L0–L2 knowledge base. Free of any LLM dependency.

pub mod detector;
pub mod generator;
pub mod lang;
pub mod pack;
pub mod scan;
pub mod scope;
pub mod subsystems;
pub mod task;
pub mod types;

use std::path::{Path, PathBuf};

use crate::config::StorageMode;

pub use types::{DiscoveredModule, McpCheckResult, ProjectAnalysis};

/// Repo-mode knowledge dir, relative to the project root.
pub const REPO_KNOWLEDGE_DIR: &str = ".context-init/knowledge";

/// Resolves the active knowledge dir from the configured storage mode only; the files on
/// disk are never consulted.
pub fn resolve_knowledge_dir(root: &Path, mode: StorageMode, central_kb: &Path) -> PathBuf {
    match mode {
        StorageMode::Repo => root.join(REPO_KNOWLEDGE_DIR),
        StorageMode::Central => central_kb.to_path_buf(),
    }
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
            agents_md: Some(StorageMode::Central),
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
    fn l0_v3_omits_volatile_fields() {
        let root = temp_dir("v3_root");
        let kd = temp_dir("v3_kd").join("knowledge");
        let mut sub = core_subsystem();
        sub.tokens = 123;
        sub.files = vec![crate::storage::SubsystemFile {
            path: "src/lib.rs".to_string(),
            is_entrypoint: true,
            lines: 77,
        }];
        generator::generate(
            &analysis_for(&root),
            &[sub],
            &kd,
            &GenerateOptions::default(),
        )
        .unwrap();
        let json = fs::read_to_string(kd.join("L0_index.json")).unwrap();
        for forbidden in ["\"root_path\"", "\"lines\"", "\"tokens\""] {
            assert!(!json.contains(forbidden), "{forbidden} in {json}");
        }
        let index = crate::storage::L0Index::from_json_str(&json).unwrap();
        assert_eq!(index.schema_version, 3);
        let mut texts = vec![fs::read_to_string(kd.join("L0_index.md")).unwrap()];
        texts.push(fs::read_to_string(kd.join("subsystems/core.md")).unwrap());
        for text in texts {
            for forbidden in ["Строк", "токенов", "Токены"] {
                assert!(!text.contains(forbidden), "{forbidden} in {text}");
            }
        }
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
    fn resolve_knowledge_dir_follows_mode() {
        let root = temp_dir("resolve_root");
        let central = temp_dir("resolve_central").join("kb");
        let repo_kb = root.join(".context-init").join("knowledge");

        fs::create_dir_all(&central).unwrap();
        fs::write(central.join("L0_index.json"), "{}").unwrap();
        assert_eq!(
            resolve_knowledge_dir(&root, StorageMode::Repo, &central),
            repo_kb
        );

        fs::create_dir_all(&repo_kb).unwrap();
        fs::write(repo_kb.join("L0_index.json"), "{}").unwrap();
        fs::remove_file(central.join("L0_index.json")).unwrap();
        assert_eq!(
            resolve_knowledge_dir(&root, StorageMode::Central, &central),
            central
        );
    }
}
