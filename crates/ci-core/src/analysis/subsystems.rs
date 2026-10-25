//! Deterministic subsystem extraction from a [`ProjectAnalysis`] (no LLM).

use std::path::Path;

use crate::analysis::detector;
use crate::analysis::ProjectAnalysis;
use crate::storage::SubsystemInfo;

/// Builds one subsystem per detected module (or a single `core` for flat projects),
/// with token counts of their files. Split-off modules keep their `parent` link.
pub fn build_deterministic(analysis: &ProjectAnalysis) -> Vec<SubsystemInfo> {
    let mut subsystems: Vec<SubsystemInfo> = Vec::new();
    if analysis.detected_modules.is_empty() {
        // Плоский проект
        subsystems.push(SubsystemInfo {
            id: "core".to_string(),
            name: "Core Logic".to_string(),
            summary: format!("Основная логика и компоненты проекта {}", analysis.name),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec!["core".to_string(), "main".to_string()],
            entrypoints: analysis.root_entrypoints.clone(),
            key_files: analysis.root_entrypoints.clone(),
            files: Vec::new(),
            key_symbols: Vec::new(),
            test_command: analysis.commands.test.clone(),
            tokens: 0,
            parent: None,
        });
    } else {
        for m in &analysis.detected_modules {
            let clean_id = m.id.clone();
            let capitalized = m
                .name
                .replace(['_', '-'], " ")
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            let summary = module_summary(
                &m.dir_path,
                &m.entrypoints,
                m.files.len(),
                analysis.root_dir.join(&m.dir_path).is_file(),
            );

            let key_files = m.files.iter().map(|f| f.path.clone()).collect();

            subsystems.push(SubsystemInfo {
                id: clean_id.clone(),
                name: capitalized,
                summary,
                l1_path: format!("subsystems/{}.md", clean_id),
                tags: vec![clean_id, analysis.project_type.display_name().to_string()],
                entrypoints: m.entrypoints.clone(),
                key_files,
                files: m.files.clone(),
                key_symbols: m.symbols.clone(),
                test_command: m.test_cmd.clone(),
                tokens: 0,
                parent: m.parent.clone(),
            });
        }
    }

    for sub in &mut subsystems {
        sub.tokens = subsystem_tokens(&analysis.root_dir, sub);
    }
    subsystems
}

/// Prefixes of the deterministic summaries below; `update` refreshes summaries that
/// start with one of them and keeps everything else (hand-written or LLM-enriched).
pub(crate) const GENERATED_SUMMARY_PREFIXES: [&str; 3] = [
    "Подсистема `",
    "Модуль и компоненты директории `",
    "Модуль-файл `",
];

/// Deterministic summary of a module: a single-file module (split off a large crate)
/// is named as a file, not as a directory.
fn module_summary(dir_path: &str, entrypoints: &[String], files: usize, is_file: bool) -> String {
    if is_file {
        format!("Модуль-файл `{dir_path}`")
    } else if !entrypoints.is_empty() {
        format!(
            "Подсистема `{dir_path}` с ключевыми точками входа: {}",
            entrypoints.join(", ")
        )
    } else {
        format!("Модуль и компоненты директории `{dir_path}` ({files} файлов)")
    }
}

/// BPE token count of the subsystem's current sources, read from disk.
pub fn subsystem_tokens(root_dir: &Path, sub: &SubsystemInfo) -> usize {
    let paths: Vec<&str> = if sub.files.is_empty() {
        sub.key_files.iter().map(String::as_str).collect()
    } else {
        sub.files.iter().map(|f| f.path.as_str()).collect()
    };
    paths
        .into_iter()
        .filter_map(|p| std::fs::read_to_string(root_dir.join(p)).ok())
        .map(|content| detector::count_tokens(&content))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_file_module_is_described_as_a_file() {
        let s = module_summary("crates/a/src/config.rs", &[], 1, true);
        assert_eq!(s, "Модуль-файл `crates/a/src/config.rs`");
        assert!(!s.contains("директории"));
    }

    #[test]
    fn directory_modules_keep_their_wording() {
        let eps = vec!["a/src/lib.rs".to_string()];
        assert!(module_summary("a", &eps, 3, false).starts_with("Подсистема `a`"));
        assert_eq!(
            module_summary("docs", &[], 2, false),
            "Модуль и компоненты директории `docs` (2 файлов)"
        );
        for s in [
            module_summary("x.rs", &[], 1, true),
            module_summary("a", &eps, 3, false),
            module_summary("docs", &[], 2, false),
        ] {
            assert!(GENERATED_SUMMARY_PREFIXES.iter().any(|p| s.starts_with(p)));
        }
    }
}
