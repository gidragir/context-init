//! Full-text retrieval over the L0 index (D18): a BM25 index with Russian and English
//! stemming. LLM reranking lives in `ci-mcp`; this module is deterministic.

use crate::openviking::{extract_l0_abstract, OkfCard};
use crate::storage::L0Index;
use anyhow::Result;
use rust_stemmers::{Algorithm, Stemmer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SearchHit {
    pub subsystem_id: String,
    pub name: String,
    pub score: f32,
    pub summary: String,
    pub matched_symbols: Vec<String>,
}

pub trait Retriever: Send + Sync {
    fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchHit>>;
}

/// Words of `text` as lower-case stems. CamelCase / snake_case identifiers contribute the
/// whole identifier and its parts; Cyrillic words use the Russian stemmer, the rest English.
fn stems(text: &str) -> Vec<String> {
    let ru = Stemmer::create(Algorithm::Russian);
    let en = Stemmer::create(Algorithm::English);
    let stem = |w: &str| -> String {
        let w = w.to_lowercase().replace('ё', "е");
        if w.chars().any(|c| ('а'..='я').contains(&c)) {
            ru.stem(&w).into_owned()
        } else {
            en.stem(&w).into_owned()
        }
    };
    let mut out = Vec::new();
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        out.push(stem(word));
        // Split at lower -> Upper boundaries (`CheckAuthToken` -> Check, Auth, Token).
        let mut parts: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut prev_lower = false;
        for c in word.chars() {
            if c.is_uppercase() && prev_lower && !cur.is_empty() {
                parts.push(std::mem::take(&mut cur));
            }
            prev_lower = c.is_lowercase();
            cur.push(c);
        }
        parts.push(cur);
        if parts.len() > 1 {
            out.extend(parts.iter().map(|p| stem(p)));
        }
    }
    out
}

fn normalized(text: &str) -> String {
    stems(text).join(" ")
}

const MAX_CARD_BYTES: u64 = 256 * 1024;

/// Reads an L1 card from inside `knowledge_dir` only: absolute paths, `..`, non-regular
/// files (e.g. `/dev/zero`) and oversized files are ignored.
fn read_card_text(knowledge_dir: &Path, rel: &str) -> Option<String> {
    let path = crate::storage::safe_join(knowledge_dir, rel)?;
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_CARD_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Tag marking a test-only subsystem (see the detector); its hits are demoted.
const TESTS_TAG: &str = "tests";
/// Score multiplier for subsystems tagged [`TESTS_TAG`].
const TESTS_SCORE_FACTOR: f32 = 0.35;

struct Entry {
    name: String,
    tags: Vec<String>,
    summary: String,
    symbols: Vec<String>,
    id_stems: HashSet<String>,
}

/// In-memory tantivy index over the subsystems of an [`L0Index`].
pub struct Bm25Retriever {
    index: Index,
    reader: IndexReader,
    id: Field,
    fields: Vec<(Field, f32)>,
    entries: HashMap<String, Entry>,
}

impl Bm25Retriever {
    /// Indexes name, summary, tags, symbols, files and the L1 card abstract of every subsystem.
    pub fn build(index: &L0Index, knowledge_dir: &Path) -> Result<Self> {
        let mut sb = Schema::builder();
        let id = sb.add_text_field("id", STRING | STORED);
        let weighted = [
            ("name", 3.0f32),
            ("symbols", 3.0),
            ("summary", 2.0),
            ("tags", 1.5),
            ("card", 1.0),
            ("files", 1.0),
        ];
        let fields: Vec<(Field, f32)> = weighted
            .iter()
            .map(|(n, w)| (sb.add_text_field(n, TEXT), *w))
            .collect();
        let tindex = Index::create_in_ram(sb.build());
        let mut writer: IndexWriter = tindex.writer_with_num_threads(1, 15_000_000)?;
        let mut entries = HashMap::new();
        for sub in &index.subsystems {
            let card = read_card_text(knowledge_dir, &sub.l1_path)
                .map(|t| match OkfCard::parse(&t) {
                    Ok(c) => c.l0_abstract(),
                    Err(_) => extract_l0_abstract(&t),
                })
                .unwrap_or_default();
            let files = sub
                .files
                .iter()
                .map(|f| f.path.as_str())
                .chain(sub.key_files.iter().map(String::as_str))
                .chain(sub.entrypoints.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(" ");
            let texts = [
                sub.name.clone(),
                sub.key_symbols.join(" "),
                sub.summary.clone(),
                format!("{} {}", sub.id, sub.tags.join(" ")),
                card,
                files,
            ];
            let mut doc = TantivyDocument::default();
            doc.add_text(id, &sub.id);
            for ((field, _), text) in fields.iter().zip(&texts) {
                doc.add_text(*field, normalized(text));
            }
            writer.add_document(doc)?;
            let id_and_paths = format!(
                "{} {} {}",
                sub.id,
                sub.key_files.join(" "),
                sub.files
                    .iter()
                    .map(|f| f.path.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            let id_stems: HashSet<String> = stems(&id_and_paths).into_iter().collect();
            entries.insert(
                sub.id.clone(),
                Entry {
                    name: sub.name.clone(),
                    tags: sub.tags.clone(),
                    summary: sub.summary.clone(),
                    symbols: sub.key_symbols.clone(),
                    id_stems,
                },
            );
        }
        writer.commit()?;
        let reader = tindex
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        Ok(Self {
            index: tindex,
            reader,
            id,
            fields,
            entries,
        })
    }
}

impl Retriever for Bm25Retriever {
    fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchHit>> {
        let terms = stems(query);
        let top_k = top_k.min(self.entries.len());
        if terms.is_empty() || top_k == 0 {
            return Ok(Vec::new());
        }
        let mut parser =
            QueryParser::for_index(&self.index, self.fields.iter().map(|(f, _)| *f).collect());
        for (field, boost) in &self.fields {
            parser.set_field_boost(*field, *boost);
        }
        let (parsed, _ignored) = parser.parse_query_lenient(&terms.join(" "));
        let searcher = self.reader.searcher();
        let top = searcher.search(&parsed, &TopDocs::with_limit(top_k * 2))?;
        let wanted: HashSet<&String> = terms.iter().collect();
        let is_test_query = wanted
            .iter()
            .any(|t| t.contains("test") || t.contains("bench") || t.contains("тест"));
        let mut hits = Vec::new();
        for (score, addr) in top {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let Some(sub_id) = doc.get_first(self.id).and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(entry) = self.entries.get(sub_id) else {
                continue;
            };
            let matched_symbols = entry
                .symbols
                .iter()
                .filter(|sym| stems(sym).iter().any(|t| wanted.contains(t)))
                .cloned()
                .collect();
            let id_matches = wanted
                .iter()
                .filter(|t| entry.id_stems.contains(**t))
                .count();
            let score =
                if entry.tags.iter().any(|t| t == TESTS_TAG) && id_matches == 0 && !is_test_query {
                    score * TESTS_SCORE_FACTOR
                } else {
                    score
                };
            let score = score * (1.0 + 0.6 * (id_matches as f32));
            hits.push(SearchHit {
                subsystem_id: sub_id.to_string(),
                name: entry.name.clone(),
                score,
                summary: entry.summary.clone(),
                matched_symbols,
            });
        }
        // Stable sort keeps the BM25 order among equal scores.
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(top_k);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SubsystemInfo;

    fn sub(id: &str, name: &str, summary: &str, symbols: &[&str]) -> SubsystemInfo {
        SubsystemInfo {
            id: id.to_string(),
            name: name.to_string(),
            summary: summary.to_string(),
            l1_path: format!("subsystems/{id}.md"),
            files: Vec::new(),
            entrypoints: Vec::new(),
            key_files: Vec::new(),
            key_symbols: symbols.iter().map(|s| s.to_string()).collect(),
            tags: Vec::new(),
            test_command: None,
            tokens: 1,
            parent: None,
            summary_source: crate::storage::SummarySource::Heuristic,
            summary_model: None,
            needs_review: false,
        }
    }

    fn index() -> L0Index {
        L0Index {
            subsystems: vec![
                sub(
                    "costing",
                    "Себестоимость",
                    "Проведение документов расчёта себестоимости",
                    &["РассчитатьСебестоимость"],
                ),
                sub(
                    "auth",
                    "Аутентификация",
                    "Аутентификация и сессии пользователей",
                    &["CheckAuthToken"],
                ),
                sub(
                    "billing",
                    "Billing",
                    "Invoices and payment processing",
                    &["CreateInvoice"],
                ),
                sub(
                    "reports",
                    "Отчёты",
                    "Формирование печатных форм и отчётов",
                    &["BuildReport"],
                ),
                sub(
                    "storage",
                    "Storage",
                    "Persistent storage and caching layer",
                    &["WriteBlob"],
                ),
            ],
            ..L0Index::default()
        }
    }

    #[test]
    fn card_paths_cannot_escape_the_knowledge_dir() {
        let mut idx = index();
        idx.subsystems[0].l1_path = "/dev/zero".to_string();
        idx.subsystems[1].l1_path = "../../etc/passwd".to_string();
        // Must return promptly instead of reading unbounded or foreign files.
        let r = Bm25Retriever::build(&idx, Path::new(".")).unwrap();
        assert!(!r
            .search("root", 3)
            .unwrap()
            .iter()
            .any(|h| h.subsystem_id == "auth"));
    }

    #[test]
    fn bm25_retriever_ranks_russian_stemmed_and_symbol_queries() {
        let retriever = Bm25Retriever::build(&index(), Path::new(".")).unwrap();
        let hits = retriever
            .search("расчет себестоимость документ", 3)
            .unwrap();
        assert_eq!(
            hits.first().map(|h| h.subsystem_id.as_str()),
            Some("costing"),
            "{hits:?}"
        );
        let hits = retriever.search("CheckAuthToken", 3).unwrap();
        assert_eq!(
            hits.first().map(|h| h.subsystem_id.as_str()),
            Some("auth"),
            "{hits:?}"
        );
        assert_eq!(hits[0].matched_symbols, ["CheckAuthToken"]);
        let hits = retriever.search("payments invoice", 3).unwrap();
        assert_eq!(
            hits.first().map(|h| h.subsystem_id.as_str()),
            Some("billing"),
            "{hits:?}"
        );
        assert!(retriever.search("zzzz-unknown", 3).unwrap().is_empty());
        assert!(retriever.search("   ", 3).unwrap().is_empty());
        // An absurd top_k must neither panic nor over-allocate.
        assert_eq!(retriever.search("storage", usize::MAX).unwrap().len(), 1);
    }

    #[test]
    fn tests_subsystem_ranks_below_its_parent() {
        // The tests subsystem repeats the query term, so by raw BM25 it outranks its parent.
        let mut tests = sub(
            "detector_tests",
            "Детектор",
            "детектор детектор проекта",
            &["Детектор"],
        );
        tests.tags = vec!["tests".to_string()];
        let idx = L0Index {
            subsystems: vec![
                sub("detector", "Детектор", "детектор проекта", &["Детектор"]),
                tests,
            ],
            ..L0Index::default()
        };
        let hits = Bm25Retriever::build(&idx, Path::new("."))
            .unwrap()
            .search("детектор", 2)
            .unwrap();
        let ids: Vec<&str> = hits.iter().map(|h| h.subsystem_id.as_str()).collect();
        assert_eq!(ids, ["detector", "detector_tests"], "{hits:?}");
    }

    #[test]
    fn search_ranks_test_subsystem_first_when_query_matches_its_id_or_path() {
        let mut index = L0Index::default();
        let mut rep = sub(
            "reports",
            "benchmark regression reports",
            "BENCHMARK_REPORT.md — benchmark regression reports summary",
            &[],
        );
        rep.key_files = vec!["reports/BENCHMARK_REPORT.md".into()];
        let mut bench = sub(
            "tests_regression_benchmark",
            "Tests regression_benchmark",
            "Регрессионные тесты бенчмарка",
            &["bench_search_quality_and_latency"],
        );
        bench.key_files = vec!["tests/regression_benchmark.rs".into()];
        bench.tags = vec!["tests".into()];
        index.subsystems = vec![rep, bench];

        let tmp = tempfile::tempdir().unwrap();
        let retriever = Bm25Retriever::build(&index, tmp.path()).unwrap();
        let hits = retriever.search("benchmark regression", 5).unwrap();
        assert_eq!(hits[0].subsystem_id, "tests_regression_benchmark");
    }
}
