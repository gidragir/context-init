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

            let summary = if !m.entrypoints.is_empty() {
                format!(
                    "Подсистема `{}` с ключевыми точками входа: {}",
                    m.dir_path,
                    m.entrypoints.join(", ")
                )
            } else {
                format!(
                    "Модуль и компоненты директории `{}` ({} файлов)",
                    m.dir_path,
                    m.files.len()
                )
            };

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
        compute_subsystem_tokens(&analysis.root_dir, sub);
    }
    subsystems
}

fn compute_subsystem_tokens(root_dir: &Path, sub: &mut SubsystemInfo) {
    let mut total_tokens = 0;
    if !sub.files.is_empty() {
        for f in &sub.files {
            let path = root_dir.join(&f.path);
            if let Ok(content) = std::fs::read_to_string(&path) {
                total_tokens += detector::count_tokens(&content);
            }
        }
    } else {
        for path_str in &sub.key_files {
            let path = root_dir.join(path_str);
            if let Ok(content) = std::fs::read_to_string(&path) {
                total_tokens += detector::count_tokens(&content);
            }
        }
    }
    sub.tokens = total_tokens;
}
