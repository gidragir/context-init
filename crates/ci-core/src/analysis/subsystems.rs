//! Deterministic subsystem extraction from a [`ProjectAnalysis`] (no LLM).

use std::collections::BTreeSet;
use std::path::Path;

use crate::analysis::cache::AnalyzedFileCache;
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
            summary_source: crate::storage::SummarySource::Heuristic,
            summary_model: None,
            needs_review: false,
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

            let test_only =
                !m.files.is_empty() && m.files.iter().all(|f| detector::is_test_path(&f.path));
            let summary = if test_only {
                match &m.parent {
                    Some(parent) => format!("Тесты для `{parent}`"),
                    None => "Тесты проекта".to_string(),
                }
            } else {
                summary
            };
            let mut tags = vec![
                clean_id.clone(),
                analysis.project_type.display_name().to_string(),
            ];
            if test_only {
                tags.push("tests".to_string());
            }

            subsystems.push(SubsystemInfo {
                id: clean_id.clone(),
                name: capitalized,
                summary,
                l1_path: format!("subsystems/{}.md", clean_id),
                tags,
                entrypoints: m.entrypoints.clone(),
                key_files,
                files: m.files.clone(),
                key_symbols: m.symbols.clone(),
                test_command: m.test_cmd.clone(),
                tokens: 0,
                parent: m.parent.clone(),
                summary_source: crate::storage::SummarySource::Heuristic,
                summary_model: None,
                needs_review: false,
            });
        }
    }

    match analysis.file_cache.as_deref() {
        // Tokens counted during detection are reused; the rest is counted (in parallel)
        // through the same cache, which also persists them for the next run.
        Some(cache) => {
            let paths: BTreeSet<&str> = subsystems.iter().flat_map(|s| token_paths(s)).collect();
            prefetch_bpe_tokens(cache, &paths.into_iter().collect::<Vec<_>>());
            for sub in &mut subsystems {
                sub.tokens = subsystem_tokens_cached(cache, sub);
            }
            let _ = cache.save();
            cache.release_contents();
        }
        None => {
            for sub in &mut subsystems {
                sub.tokens = subsystem_tokens(&analysis.root_dir, sub);
            }
        }
    }
    subsystems
}

/// Files whose tokens make up a subsystem's size: `files`, else `key_files`.
fn token_paths(sub: &SubsystemInfo) -> Vec<&str> {
    if sub.files.is_empty() {
        sub.key_files.iter().map(String::as_str).collect()
    } else {
        sub.files.iter().map(|f| f.path.as_str()).collect()
    }
}

/// Counts the BPE tokens of `paths` not yet known to `cache` on all cores.
pub fn prefetch_bpe_tokens(cache: &AnalyzedFileCache, paths: &[&str]) {
    let missing: Vec<&str> = paths
        .iter()
        .copied()
        .filter(|p| cache.cached_bpe_tokens(p).is_none())
        .collect();
    if missing.len() < 2 {
        return;
    }
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = missing.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        for part in missing.chunks(chunk) {
            scope.spawn(move || {
                for p in part {
                    cache.bpe_tokens(p);
                }
            });
        }
    });
}

/// BPE token count of the subsystem's current sources through `cache` (memory, then the
/// disk metrics cache when it is valid for the file, else the file itself).
/// Files the cache classifies as binary (extension, NUL or invalid UTF-8 in the first 8 KiB)
/// count as `0`, unlike [`subsystem_tokens`].
pub fn subsystem_tokens_cached(cache: &AnalyzedFileCache, sub: &SubsystemInfo) -> usize {
    token_paths(sub)
        .into_iter()
        .map(|p| cache.bpe_tokens(p))
        .sum()
}

/// Prefixes of the deterministic summaries in this module (the flat-project fallback
/// included). A v3 summary starting with one of them migrates as `Heuristic`; a `Heuristic`
/// summary that no longer starts with one was edited by hand and is kept by `update`.
pub(crate) const GENERATED_SUMMARY_PREFIXES: [&str; 6] = [
    "Основная логика и компоненты проекта",
    "Тесты для `",
    "Тесты проекта",
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
    token_paths(sub)
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
