//! Content metrics of project files (lines, BPE tokens, symbols), cached at three levels:
//!
//! 1. [`AnalyzedFileCache`] — one analysis run: every file is read at most once per set of
//!    requested metrics, and the computed tokens are handed on (`ProjectAnalysis`).
//! 2. A process-wide memo (repeated MCP `context_check` calls on an unchanged tree).
//! 3. An optional [`DiskMetricsCache`] at `<StoragePaths::cache_dir>/<project_hash>/metrics_v1.json`,
//!    strictly outside the working tree (never in `L0_index.json`, see `l0_omits_volatile_fields`).
//!
//! Entries of levels 2 and 3 are keyed by `(len, modified_nanos)`. A file modified less than
//! [`RACY_WINDOW_NANOS`] before its entry was recorded may have changed again within the
//! mtime granularity, so its content is re-hashed (`sha2::Sha256`) before the entry is used.
//! Disk entries also carry `ci_core::VERSION`; any mismatch, a missing or corrupted file is a
//! plain cache miss.

use crate::analysis::scan::{has_binary_extension, header_is_binary};
use crate::config::StoragePaths;
use crate::tokens::count_tokens;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// File name of the disk cache inside `<cache_dir>/<project_hash>/`.
pub const METRICS_CACHE_FILE: &str = "metrics_v1.json";

/// Entries recorded less than this long after the file's mtime are verified by SHA-256.
pub const RACY_WINDOW_NANOS: u64 = 2_000_000_000;

/// Metrics derived from a file's content alone.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetrics {
    /// `lines().count()` of the UTF-8 text; `0` when the file is not valid UTF-8.
    pub lines: usize,
    /// The whole file is valid UTF-8.
    pub text: bool,
    /// Binary by extension, NUL byte or invalid UTF-8 in the first 8 KiB (`is_likely_binary`).
    pub binary: bool,
    /// `estimate_tokens_fast` of a non-binary text file, else `0`.
    pub fast_tokens: usize,
    /// BPE (`cl100k_base`) tokens; `0` for binary or non-UTF-8 files. Filled on demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bpe_tokens: Option<usize>,
    /// Shallow symbols (`deep = false`). Filled on demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<String>>,
    /// Deep symbols (`deep = true`). Filled on demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_symbols: Option<Vec<String>>,
}

/// Optional metrics a caller needs on top of the always-computed ones.
#[derive(Debug, Clone, Copy, Default)]
struct Need {
    bpe: bool,
    symbols: Option<bool>,
}

impl FileMetrics {
    fn from_bytes(rel: &str, bytes: &[u8]) -> Self {
        let text = std::str::from_utf8(bytes).ok();
        let binary = has_binary_extension(Path::new(rel)) || header_is_binary(bytes);
        Self {
            lines: text.map_or(0, |t| t.lines().count()),
            text: text.is_some(),
            binary,
            fast_tokens: match text {
                Some(t) if !binary => crate::analysis::detector::estimate_tokens_fast(t),
                _ => 0,
            },
            ..Self::default()
        }
    }

    /// Metrics of a file that cannot be read: everything empty.
    fn unreadable(rel: &str) -> Self {
        Self {
            binary: has_binary_extension(Path::new(rel)),
            bpe_tokens: Some(0),
            symbols: Some(Vec::new()),
            deep_symbols: Some(Vec::new()),
            ..Self::default()
        }
    }

    fn symbols_slot(&mut self, deep: bool) -> &mut Option<Vec<String>> {
        if deep {
            &mut self.deep_symbols
        } else {
            &mut self.symbols
        }
    }

    fn satisfies(&self, need: Need) -> bool {
        (!need.bpe || self.bpe_tokens.is_some())
            && need.symbols.is_none_or(|deep| {
                if deep {
                    self.deep_symbols.is_some()
                } else {
                    self.symbols.is_some()
                }
            })
    }

    /// Computes the missing optional metrics in `need` from the file's `text`.
    fn fill(&mut self, rel: &str, text: Option<&str>, need: Need) {
        if need.bpe && self.bpe_tokens.is_none() {
            self.bpe_tokens = Some(match text {
                Some(t) if !self.binary => count_tokens(t),
                _ => 0,
            });
        }
        if let Some(deep) = need.symbols {
            let slot = self.symbols_slot(deep);
            if slot.is_none() {
                *slot = Some(text.map_or_else(Vec::new, |t| {
                    crate::analysis::detector::compute_symbols(rel, t, deep)
                }));
            }
        }
    }

    fn merge_missing(&mut self, other: FileMetrics) {
        if self.bpe_tokens.is_none() {
            self.bpe_tokens = other.bpe_tokens;
        }
        if self.symbols.is_none() {
            self.symbols = other.symbols;
        }
        if self.deep_symbols.is_none() {
            self.deep_symbols = other.deep_symbols;
        }
    }
}

/// One cached file: the stat and content hash it was computed for, plus its metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedFileEntry {
    pub len: u64,
    pub modified_nanos: u64,
    pub sha256_hex: String,
    pub ci_core_version: String,
    /// When the entry was (re)validated against the file content.
    pub recorded_nanos: u64,
    #[serde(flatten)]
    pub metrics: FileMetrics,
}

impl CachedFileEntry {
    fn matches_stat(&self, len: u64, modified_nanos: u64) -> bool {
        modified_nanos != 0 && self.len == len && self.modified_nanos == modified_nanos
    }

    /// The file was last modified well before the entry was recorded, so equal
    /// `(len, mtime)` cannot hide a same-second rewrite.
    fn trusted_without_read(&self) -> bool {
        self.modified_nanos.saturating_add(RACY_WINDOW_NANOS) < self.recorded_nanos
    }
}

/// Hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Stable directory name of a project inside `cache_dir`: the SHA-256 (first 128 bits, hex)
/// of its canonical root path.
pub fn project_hash(root: &Path) -> String {
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut hex = sha256_hex(canonical.to_string_lossy().as_bytes());
    hex.truncate(32);
    hex
}

fn now_nanos() -> u64 {
    nanos_since_epoch(SystemTime::now())
}

fn nanos_since_epoch(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

#[derive(Serialize, Deserialize)]
struct DiskFile {
    ci_core_version: String,
    entries: BTreeMap<String, CachedFileEntry>,
}

/// Persistent per-project metrics at `<cache_dir>/<project_hash>/metrics_v1.json`.
#[derive(Debug)]
pub struct DiskMetricsCache {
    path: PathBuf,
    root: PathBuf,
    version: String,
    entries: BTreeMap<String, CachedFileEntry>,
    touched: BTreeSet<String>,
    dirty: bool,
}

impl DiskMetricsCache {
    /// Loads the cache of `root` from `cache_dir` for the running `ci_core::VERSION`.
    pub fn load(cache_dir: &Path, root: &Path) -> Self {
        Self::load_with_version(cache_dir, root, crate::VERSION)
    }

    /// [`DiskMetricsCache::load`] from [`StoragePaths::cache_dir`].
    pub fn for_storage(paths: &StoragePaths, root: &Path) -> Self {
        Self::load(&paths.cache_dir, root)
    }

    /// Loads entries written by ci-core `version`; anything else (missing file, corrupted
    /// JSON, another version) yields an empty cache.
    pub fn load_with_version(cache_dir: &Path, root: &Path, version: &str) -> Self {
        let path = cache_dir.join(project_hash(root)).join(METRICS_CACHE_FILE);
        let entries = fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice::<DiskFile>(&raw).ok())
            .filter(|file| file.ci_core_version == version)
            .map(|file| {
                file.entries
                    .into_iter()
                    .filter(|(_, e)| e.ci_core_version == version)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            path,
            root: root.to_path_buf(),
            version: version.to_string(),
            entries,
            touched: BTreeSet::new(),
            dirty: false,
        }
    }

    /// Location of the cache file (outside the project tree).
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The stored entry of a project-relative path, unvalidated.
    pub fn entry(&self, rel: &str) -> Option<&CachedFileEntry> {
        self.entries.get(rel)
    }

    fn get(&mut self, rel: &str) -> Option<CachedFileEntry> {
        self.touched.insert(rel.to_string());
        self.entries.get(rel).cloned()
    }

    fn put(&mut self, rel: &str, mut entry: CachedFileEntry) {
        entry.ci_core_version = self.version.clone();
        self.touched.insert(rel.to_string());
        self.entries.insert(rel.to_string(), entry);
        self.dirty = true;
    }

    /// Writes the cache atomically (temp file + rename) when it changed, dropping entries of
    /// files that are neither used by this run nor present on disk any more.
    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let (root, touched) = (&self.root, &self.touched);
        self.entries
            .retain(|rel, _| touched.contains(rel) || root.join(rel).is_file());
        let dir = self
            .path
            .parent()
            .context("metrics cache path has no parent")?;
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create metrics cache dir {}", dir.display()))?;
        let file = DiskFile {
            ci_core_version: self.version.clone(),
            entries: std::mem::take(&mut self.entries),
        };
        let json = serde_json::to_vec(&file);
        self.entries = file.entries;
        let json = json?;
        let tmp = dir.join(format!(
            ".{METRICS_CACHE_FILE}.tmp-{}-{}",
            std::process::id(),
            now_nanos()
        ));
        let written = fs::write(&tmp, &json).and_then(|()| fs::rename(&tmp, &self.path));
        if let Err(err) = written {
            let _ = fs::remove_file(&tmp);
            return Err(err)
                .with_context(|| format!("Failed to write metrics cache {}", self.path.display()));
        }
        self.dirty = false;
        Ok(())
    }
}

/// Process-wide memo by absolute path (same validation as the disk cache).
fn process_memo() -> &'static Mutex<HashMap<PathBuf, CachedFileEntry>> {
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, CachedFileEntry>>> = OnceLock::new();
    MEMO.get_or_init(Default::default)
}

/// Metrics of the files of one analysis run, keyed by project-relative path. `Sync`, so the
/// parallel tokenizer threads share it.
pub struct AnalyzedFileCache {
    root: PathBuf,
    run: Mutex<HashMap<String, CachedFileEntry>>,
    disk: Option<Mutex<DiskMetricsCache>>,
}

impl std::fmt::Debug for AnalyzedFileCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyzedFileCache")
            .field("root", &self.root)
            .field("files", &self.run.lock().map_or(0, |r| r.len()))
            .field(
                "disk",
                &self
                    .disk
                    .as_ref()
                    .and_then(|d| d.lock().ok().map(|d| d.path.clone())),
            )
            .finish()
    }
}

impl AnalyzedFileCache {
    /// In-memory cache (plus the process memo) for the project at `root`.
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            run: Mutex::new(HashMap::new()),
            disk: None,
        }
    }

    /// Cache backed by a persistent [`DiskMetricsCache`]; call [`AnalyzedFileCache::save`].
    pub fn with_disk(root: &Path, disk: DiskMetricsCache) -> Self {
        Self {
            disk: Some(Mutex::new(disk)),
            ..Self::new(root)
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Always-computed metrics of `rel` (lines, binary flag, fast token estimate).
    pub fn metrics(&self, rel: &str) -> FileMetrics {
        self.get(rel, Need::default())
    }

    pub fn lines(&self, rel: &str) -> usize {
        self.metrics(rel).lines
    }

    /// `estimate_tokens_fast` of a non-binary UTF-8 file, `0` otherwise.
    pub fn fast_tokens(&self, rel: &str) -> usize {
        self.metrics(rel).fast_tokens
    }

    /// BPE tokens of `rel` (`0` for binary, non-UTF-8 or unreadable files).
    pub fn bpe_tokens(&self, rel: &str) -> usize {
        let need = Need {
            bpe: true,
            symbols: None,
        };
        self.get(rel, need).bpe_tokens.unwrap_or(0)
    }

    /// BPE tokens of `rel` already known to this run, without reading the file.
    pub fn cached_bpe_tokens(&self, rel: &str) -> Option<usize> {
        let run = self.run.lock().ok()?;
        run.get(rel).and_then(|e| e.metrics.bpe_tokens)
    }

    /// Symbols of `rel` (empty when unreadable).
    pub fn symbols(&self, rel: &str, deep: bool) -> Vec<String> {
        let need = Need {
            bpe: false,
            symbols: Some(deep),
        };
        self.get(rel, need)
            .symbols_slot(deep)
            .take()
            .unwrap_or_default()
    }

    /// Every BPE token count known to this run, by project-relative path.
    pub fn token_map(&self) -> BTreeMap<String, usize> {
        self.run.lock().map_or_else(
            |_| BTreeMap::new(),
            |run| {
                run.iter()
                    .filter_map(|(rel, e)| e.metrics.bpe_tokens.map(|t| (rel.clone(), t)))
                    .collect()
            },
        )
    }

    /// Persists the disk cache, if any.
    pub fn save(&self) -> Result<()> {
        match &self.disk {
            Some(disk) => disk
                .lock()
                .map_err(|_| anyhow::anyhow!("metrics cache lock poisoned"))?
                .save(),
            None => Ok(()),
        }
    }

    fn get(&self, rel: &str, need: Need) -> FileMetrics {
        let run_entry = self.run.lock().ok().and_then(|r| r.get(rel).cloned());
        if let Some(e) = &run_entry {
            if e.metrics.satisfies(need) {
                return e.metrics.clone();
            }
        }
        let abs = self.root.join(rel);
        let Ok(meta) = fs::metadata(&abs) else {
            return FileMetrics::unreadable(rel);
        };
        let len = meta.len();
        let modified = meta.modified().map_or(0, nanos_since_epoch);
        let mut bytes: Option<Vec<u8>> = None;
        let mut entry = run_entry;
        let mut changed = false;

        if entry.is_none() {
            let memo = process_memo()
                .lock()
                .ok()
                .and_then(|m| m.get(&abs).cloned());
            let disk = self
                .disk
                .as_ref()
                .and_then(|d| d.lock().ok().and_then(|mut d| d.get(rel)));
            let mut current_sha: Option<String> = None;
            // A disk cache missing (or holding a stale) entry is refreshed below.
            changed |= self.disk.is_some()
                && !disk.as_ref().is_some_and(|d| d.matches_stat(len, modified));
            for cand in [memo, disk].into_iter().flatten() {
                if !cand.matches_stat(len, modified) {
                    continue;
                }
                if !cand.trusted_without_read() {
                    if bytes.is_none() {
                        bytes = fs::read(&abs).ok();
                    }
                    let Some(b) = &bytes else { continue };
                    let sha = current_sha.get_or_insert_with(|| sha256_hex(b));
                    if *sha != cand.sha256_hex {
                        continue;
                    }
                    changed = true;
                }
                match &mut entry {
                    None => entry = Some(cand),
                    Some(e) => e.metrics.merge_missing(cand.metrics),
                }
            }
        }

        let satisfied = entry.as_ref().is_some_and(|e| e.metrics.satisfies(need));
        if !satisfied {
            if bytes.is_none() {
                bytes = fs::read(&abs).ok();
            }
            let Some(b) = &bytes else {
                return FileMetrics::unreadable(rel);
            };
            let sha = sha256_hex(b);
            let mut e = match entry.take() {
                Some(e) if e.sha256_hex == sha => e,
                _ => CachedFileEntry {
                    len,
                    modified_nanos: modified,
                    sha256_hex: sha,
                    ci_core_version: crate::VERSION.to_string(),
                    recorded_nanos: 0,
                    metrics: FileMetrics::from_bytes(rel, b),
                },
            };
            e.metrics.fill(rel, std::str::from_utf8(b).ok(), need);
            entry = Some(e);
            changed = true;
        }
        let Some(mut entry) = entry else {
            return FileMetrics::unreadable(rel);
        };
        if changed {
            entry.recorded_nanos = now_nanos();
            if let Ok(mut memo) = process_memo().lock() {
                memo.insert(abs, entry.clone());
            }
            if let Some(disk) = &self.disk {
                if let Ok(mut d) = disk.lock() {
                    d.put(rel, entry.clone());
                }
            }
        }
        let metrics = entry.metrics.clone();
        if let Ok(mut run) = self.run.lock() {
            run.insert(rel.to_string(), entry);
        }
        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::count_tokens;
    use std::fs;

    #[test]
    fn disk_metrics_cache_lives_outside_repo_and_invalidates_on_content_change() {
        let repo = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        let root = repo.path();
        fs::create_dir_all(root.join("src")).unwrap();
        let original = "pub fn alpha() {}\n";
        fs::write(root.join("src/lib.rs"), original).unwrap();

        let disk = DiskMetricsCache::load(cache_dir.path(), root);
        let expected = cache_dir
            .path()
            .join(project_hash(root))
            .join("metrics_v1.json");
        assert_eq!(disk.path(), expected.as_path());
        assert!(!disk.path().starts_with(root));

        let cache = AnalyzedFileCache::with_disk(root, disk);
        assert_eq!(cache.lines("src/lib.rs"), 1);
        assert_eq!(cache.bpe_tokens("src/lib.rs"), count_tokens(original));
        assert!(cache
            .symbols("src/lib.rs", false)
            .iter()
            .any(|s| s.contains("alpha")));
        assert_eq!(
            cache.token_map().get("src/lib.rs"),
            Some(&count_tokens(original))
        );
        cache.save().unwrap();
        assert!(expected.is_file(), "cache file must be written");
        let repo_entries: Vec<_> = walkdir_files(root);
        assert_eq!(repo_entries, vec!["src/lib.rs".to_string()]);

        let reloaded = DiskMetricsCache::load(cache_dir.path(), root);
        let entry = reloaded.entry("src/lib.rs").expect("persisted entry");
        assert_eq!(entry.ci_core_version, crate::VERSION);
        assert_eq!(entry.len, original.len() as u64);
        assert_eq!(entry.metrics.bpe_tokens, Some(count_tokens(original)));

        // Same length, same mtime, different content: the fresh entry is verified by SHA-256.
        let path = root.join("src/lib.rs");
        let mtime = fs::metadata(&path).unwrap().modified().unwrap();
        let changed = "pub fn omega() {}\n";
        assert_eq!(changed.len(), original.len());
        fs::write(&path, changed).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), mtime);
        let cache = AnalyzedFileCache::with_disk(root, reloaded);
        let symbols = cache.symbols("src/lib.rs", false);
        assert!(symbols.iter().any(|s| s.contains("omega")), "{symbols:?}");
        assert!(!symbols.iter().any(|s| s.contains("alpha")), "{symbols:?}");
        assert_eq!(cache.bpe_tokens("src/lib.rs"), count_tokens(changed));
        cache.save().unwrap();
        let reloaded = DiskMetricsCache::load(cache_dir.path(), root);
        assert_eq!(
            reloaded.entry("src/lib.rs").unwrap().sha256_hex,
            sha256_hex(changed.as_bytes())
        );

        // Another ci-core version invalidates the whole file.
        let other = DiskMetricsCache::load_with_version(cache_dir.path(), root, "0.0.0-other");
        assert!(other.is_empty());

        // A corrupted file is a cache miss, never an error.
        fs::write(&expected, "{not json").unwrap();
        assert!(DiskMetricsCache::load(cache_dir.path(), root).is_empty());
        let cache =
            AnalyzedFileCache::with_disk(root, DiskMetricsCache::load(cache_dir.path(), root));
        assert_eq!(cache.lines("src/lib.rs"), 1);
        cache.save().unwrap();
        assert!(DiskMetricsCache::load(cache_dir.path(), root)
            .entry("src/lib.rs")
            .is_some());
        let leftovers: Vec<_> = fs::read_dir(expected.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            leftovers,
            vec!["metrics_v1.json".to_string()],
            "atomic write leaves no tmp"
        );
    }

    fn walkdir_files(root: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push(
                        p.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        out.sort();
        out
    }
}
