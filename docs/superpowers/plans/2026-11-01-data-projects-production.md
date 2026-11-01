# Production Readiness for `/data/projects` & Benchmark SLO Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate all 5 architectural blockers across the regression benchmark and all 29 repositories in `/data/projects` (MCP root/schema resolution, scope/binary hygiene, deep subsystem discovery, polyglot AST compression + bilingual BM25 ranking, and zero-fork/batched Git operations).

**Architecture:** Harden `ScopeFilter` and `scan_project` against lockfiles, binary files, dot-directories, and nested `.git` clones; replace depth-limited directory discovery and asymmetric splitting in `detector.rs` while removing `tiktoken_rs` from the `analyze`/`audit` hot path; add zero-dependency lexical AST compression for TypeScript/JS/Kotlin/Java and Python plus conditional `TESTS_SCORE_FACTOR` and ID/path stem boosting in `Bm25Retriever`; parse `.git/HEAD`, `.git/packed-refs`, and `.git/config` (`remote_url` with gitfile support) in-process and batch staleness checks into a 2-command `RepoGitSnapshot`; and upgrade `ci-mcp` project resolution (`workspace_roots`), parallel symlink-aware `discover_workspace`, error-path `stale_binary_footer`, and `L0Index` schema forward-compatibility.

**Tech Stack:** Rust (Workspace: `ci-core`, `ci-llm`, `ci-mcp`, `context-init`), `ignore`, `tantivy`, `rust-stemmers`, `serde_json`, `rmcp`.

**Spec:** `docs/superpowers/specs/2026-11-01-data-projects-production-design.md`

## Global Constraints

- Host command execution invariant: prefix every shell command with `rtk` (`rtk cargo test`, `rtk git status`, `rtk rg`, `rtk fd`). Never use GNU `find` or `grep`.
- Zero heavy AST parser dependencies (`tree-sitter`, `swc`, `ruff_python_ast`, `gix` are prohibited); keep lexical adapters zero-dependency in `ci-core::analysis::lang`.
- OpenViking L0–L2 contract compatibility: preserve `CURRENT_L0_SCHEMA_VERSION = 4` and OKF frontmatter format while supporting forward-compatible reads when `found > CURRENT_L0_SCHEMA_VERSION`.
- Commit messages must follow Conventional Commits in English (`feat(core): ...`, `fix(mcp): ...`, `perf(git): ...`).

## Review Focus

1. **Chezmoi / Dotfiles repositories (`win-dotfiles/dot_config`, `ai-dotfiles/infra/stow/antigravity/.gemini`):** Excluding `.agent`/`.claude`/`.github` dot-directories at top/module discovery must not exclude `dot_config` (which does not start with `.`) or explicitly included paths in `[scope].include`, nor drop files inside Stow packages when `infra/stow` is already selected as a module root.
2. **Cargo/pnpm Workspace Member with its own `.git` vs Foreign Cloned Repo (`wasmbridge-projects/docs/OpenViking`):** Stopping at nested `.git` boundaries must skip `docs/OpenViking/.git` while still allowing declared root workspace members in `Cargo.toml` / `pnpm-workspace.yaml`.
3. **UTF-8 multibyte boundary at 8192-byte binary sniff (`scan.rs`):** Slicing the first `8192` bytes of a Cyrillic UTF-8 file (`.bsl`, `.md`) can split a 2- or 3-byte codepoint at byte `8191`; `is_likely_binary` must check for `0x00` (`NUL`) bytes and ignore `Utf8Error` when `error_len().is_none()` (incomplete trailing sequence).
4. **Git worktrees and submodules where `.git` is a file (`gitdir: ...`):** In-process `head_commit` and `remote_url` in `git.rs` must resolve `.git` files (`gitdir: <path>`) and `commondir` before reading `HEAD`, `packed-refs`, and `config`.
5. **Relative `path: "walle"` when MCP `cwd` is `/data/projects/context-init` (`server.rs`):** `cwd.join("walle")` does not exist on disk; `resolve_project_root` must NOT call `find_project_root_upward` on a non-existent path (which climbs back to `/data/projects/context-init`), and must instead resolve `"walle"` via `child_repo` across sibling and `workspace_roots` directories.

---

### Task 1: Scope & Binary Hygiene (`ci-core::analysis::scope` & `ci-core::analysis::scan`)

**Files:**
- Modify: `crates/ci-core/src/analysis/detector.rs:11-40`
- Modify: `crates/ci-core/src/analysis/scope.rs:35-139`
- Modify: `crates/ci-core/src/analysis/scan.rs:15-120`

**Interfaces:**
- Produces:
  - `pub const IGNORED_LOCK_AND_ARTIFACT_FILES: &[&str]` in `crates/ci-core/src/analysis/scope.rs`
  - `pub const IGNORED_FILE_SUFFIXES: &[&str]` in `crates/ci-core/src/analysis/scope.rs`
  - `pub fn is_ignored_filename(name: &str) -> bool` in `crates/ci-core/src/analysis/scope.rs`
  - `pub fn is_likely_binary(path: &Path) -> bool` in `crates/ci-core/src/analysis/scan.rs`

- [ ] **Step 1: Write the failing unit tests in `crates/ci-core/src/analysis/scope.rs` and `crates/ci-core/src/analysis/scan.rs`**

```rust
// In crates/ci-core/src/analysis/scope.rs #[cfg(test)] mod tests
#[test]
fn scope_excludes_lockfiles_dot_dirs_and_nested_git_clones() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n").unwrap();
    std::fs::write(root.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();
    std::fs::write(root.join("Cargo.lock"), "version = 3\n").unwrap();
    std::fs::write(root.join("uv.lock"), "version = 1\n").unwrap();
    std::fs::write(root.join("tsconfig.tsbuildinfo"), "{}").unwrap();

    // Unlisted dot-directories (.agent, .changeset, .github)
    std::fs::create_dir_all(root.join(".agent/skills/foo")).unwrap();
    std::fs::write(root.join(".agent/skills/foo/SKILL.md"), "# skill").unwrap();
    std::fs::create_dir_all(root.join(".changeset")).unwrap();
    std::fs::write(root.join(".changeset/config.json"), "{}").unwrap();

    // Nested foreign git clone (docs/OpenViking/.git)
    std::fs::create_dir_all(root.join("docs/OpenViking/.git")).unwrap();
    std::fs::write(root.join("docs/OpenViking/main.py"), "print(1)\n").unwrap();

    let scope = ScopeFilter::build(root, &ScopeConfig::default()).unwrap();
    let files = scope.files();
    assert_eq!(files, &[std::path::PathBuf::from("src/lib.rs")]);
}

// In crates/ci-core/src/analysis/scan.rs #[cfg(test)] mod tests
#[test]
fn scan_project_assigns_zero_tokens_to_large_binaries_and_preserves_cyrillic_utf8() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("installer")).unwrap();
    // 600 KB binary file (> TOKENIZE_MAX_BYTES = 512 KB) with NUL bytes
    let mut bin = vec![0u8; 600 * 1024];
    bin[0..4].copy_from_slice(b"MZ\x90\x00");
    std::fs::write(root.join("installer/setup.exe"), &bin).unwrap();

    // Cyrillic UTF-8 file crossing 8192-byte boundary on a 2-byte sequence
    std::fs::create_dir_all(root.join("src")).unwrap();
    let cyrillic = "Процедура Тест()\n".repeat(600); // > 8192 bytes
    std::fs::write(root.join("src/Module.bsl"), &cyrillic).unwrap();

    let report = scan_project(root, &ScopeConfig::default(), 50_000).unwrap();
    let inst = report.directories.iter().find(|d| d.path == "installer").unwrap();
    assert_eq!(inst.estimated_tokens, 0);
    let src = report.directories.iter().find(|d| d.path == "src").unwrap();
    assert!(src.estimated_tokens > 500);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p ci-core scope_excludes_lockfiles_dot_dirs_and_nested_git_clones scan_project_assigns_zero_tokens_to_large_binaries`
Expected: FAIL (`pnpm-lock.yaml`, `.agent`, `docs/OpenViking` included; `installer` has `~153600` tokens).

- [ ] **Step 3: Implement lockfile/dot-dir/nested-git filtering in `crates/ci-core/src/analysis/scope.rs` and `is_likely_binary` in `crates/ci-core/src/analysis/scan.rs`**

In `crates/ci-core/src/analysis/detector.rs`, expand `IGNORED_DIRS` with `".agent"`, `".claude"`, `".cursor"`, `".windsurf"`, `".github"`, `".changeset"`, `".deploy"`, `".impeccable"`, `".ipynb_checkpoints"`, `".Trash-1000"`, `".cargo-cache"`, `".pnpm-store"`, `".sccache"`, `".uv-cache"`.
In `crates/ci-core/src/analysis/scope.rs`:
- Define `IGNORED_LOCK_AND_ARTIFACT_FILES`, `IGNORED_FILE_SUFFIXES`, and `pub fn is_ignored_filename(name: &str) -> bool`.
- In `ScopeFilter::build` closure `filter_entry`:
  - When `entry.depth() > 0 && is_dir`: return `false` if `(name.starts_with('.') || IGNORED_DIRS.contains(&name)) && !explicitly_included(rel, &include)` OR if `entry.path().join(".git").exists() && !explicitly_included(rel, &include)`.
  - When `!is_dir`: return `false` if `is_ignored_filename(name) && !explicitly_included(rel, &include)`.
In `crates/ci-core/src/analysis/scan.rs`:
- Implement `pub fn is_likely_binary(path: &Path) -> bool`: check extension against known binary extensions (`exe`, `dll`, `so`, `dylib`, `msi`, `cab`, `zip`, `tar`, `gz`, `7z`, `rar`, `png`, `jpg`, `jpeg`, `gif`, `webp`, `ico`, `icns`, `pdf`, `woff`, `woff2`, `ttf`, `otf`, `eot`, `mp3`, `mp4`, `wav`, `ogg`, `wasm`, `pyc`, `class`, `jar`, `db`, `sqlite`, `sqlite3`, `cf`, `epf`, `erf`, `bin`, `pack`, `idx`), then read up to `8192` bytes; return `true` if buffer contains `0x00` or if `std::str::from_utf8(&buf)` fails with `err.error_len().is_some()`.
- Call `is_likely_binary(path)` before `if size > TOKENIZE_MAX_BYTES` in `scan_project`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p ci-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/ci-core/src/analysis/detector.rs crates/ci-core/src/analysis/scope.rs crates/ci-core/src/analysis/scan.rs
rtk git commit -m "fix(core): exclude lockfiles, dot-dirs, nested git repos, and binary token inflation"
```

---

### Task 2: Subsystem Discovery Completeness, Fast Token Estimation & Symmetric Splitting (`ci-core::analysis::detector`)

**Files:**
- Modify: `crates/ci-core/src/analysis/detector.rs:45-225`
- Modify: `crates/ci-core/src/analysis/detector.rs:430-455`
- Modify: `crates/ci-core/src/analysis/detector.rs:695-1285`

**Interfaces:**
- Consumes: `crate::analysis::scope::is_ignored_filename`, `crate::analysis::scan::is_likely_binary`
- Produces:
  - `pub const SOURCE_CODE_EXTENSIONS: &[&str]` and `pub const CONFIG_AND_DOC_EXTENSIONS: &[&str]` in `crates/ci-core/src/analysis/detector.rs`
  - `pub fn estimate_tokens_fast(text: &str) -> usize` in `crates/ci-core/src/analysis/detector.rs`

- [ ] **Step 1: Write the failing unit tests in `crates/ci-core/src/analysis/detector.rs`**

```rust
#[test]
fn detects_deep_containers_astro_gradle_maven_and_avoids_doc_leaf_subsystems() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("settings.gradle.kts"), "rootProject.name = \"mimir\"\n").unwrap();
    std::fs::write(root.join("build.gradle.kts"), "plugins { kotlin(\"jvm\") }\n").unwrap();
    std::fs::write(root.join("README.md"), "<div align=\"center\">\n<img src=\"logo.png\"/>\n</div>\n\nReal project description.\n").unwrap();

    // infra/ has NO files at depth 1, only nested subdirectories up to depth 7
    let deep_stow = root.join("infra/stow/antigravity/config/plugins/rules");
    std::fs::create_dir_all(&deep_stow).unwrap();
    std::fs::write(deep_stow.join("init.lua"), "return { ok = true }\n").unwrap();

    // libs/zkbio-client/src/client.ts
    let lib_src = root.join("libs/zkbio-client/src");
    std::fs::create_dir_all(&lib_src).unwrap();
    std::fs::write(lib_src.join("client.ts"), "export class ZkClient {}\n").unwrap();

    // Astro component in src/components/Card.astro
    std::fs::create_dir_all(root.join("src/components")).unwrap();
    std::fs::write(root.join("src/components/Card.astro"), "---\nconst x = 1;\n---\n<div>{x}</div>\n").unwrap();

    let meta = analyze_project(root, false);
    assert_eq!(meta.project_name, "mimir");
    assert_eq!(meta.description, "Real project description.");
    let ids: Vec<&str> = meta.subsystems.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.iter().any(|id| id.starts_with("infra")), "missing infra in {ids:?}");
    assert!(ids.iter().any(|id| id.starts_with("libs")), "missing libs in {ids:?}");
    assert!(ids.iter().any(|id| id.contains("components")), "missing astro components in {ids:?}");
}

#[test]
fn split_context_promotes_sibling_code_files_and_keeps_readme_pinned() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    let cli_rs = "fn test_case() { assert_eq!(1 + 1, 2); }\n".repeat(1600);
    let bench_rs = "fn bench_case() { assert_eq!(2 + 2, 4); }\n".repeat(900);
    let readme_md = "# Test Documentation\nDetailed explanation of test suite.\n".repeat(400);
    std::fs::write(root.join("tests/cli.rs"), cli_rs).unwrap();
    std::fs::write(root.join("tests/regression_benchmark.rs"), bench_rs).unwrap();
    std::fs::write(root.join("tests/README.md"), readme_md).unwrap();

    let meta = analyze_project_with(root, false, false, 15_000);
    let ids: Vec<&str> = meta.subsystems.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"tests_cli"), "ids: {ids:?}");
    assert!(ids.contains(&"tests_regression_benchmark"), "ids: {ids:?}");
    assert!(!ids.contains(&"tests_readme"), "README must never become standalone subsystem: {ids:?}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p ci-core detects_deep_containers_astro_gradle_maven split_context_promotes_sibling_code_files`
Expected: FAIL.

- [ ] **Step 3: Implement the detector improvements in `crates/ci-core/src/analysis/detector.rs`**

1. Define `SOURCE_CODE_EXTENSIONS` (`rs`, `py`, `ts`, `tsx`, `js`, `jsx`, `mjs`, `cjs`, `mts`, `cts`, `astro`, `vue`, `svelte`, `go`, `bsl`, `os`, `mdo`, `java`, `kt`, `kts`, `cs`, `c`, `cpp`, `h`, `hpp`, `sh`, `bash`, `zsh`, `fish`, `ps1`, `psm1`, `lua`, `nu`, `sql`, `prisma`, `graphql`, `proto`, `css`, `scss`) and `CONFIG_AND_DOC_EXTENSIONS` (`toml`, `yaml`, `yml`, `json`, `xml`, `md`, `mdc`, `kdl`, `just`). Keep `CODE_EXTENSIONS` as the union of both, and exclude `is_ignored_filename(name)` in `subsystem_file`.
2. In `detect_type_and_name`: check `settings.gradle.kts` / `build.gradle.kts` / `build.gradle` (parse `rootProject.name`) and `pom.xml` (`ProjectType::Java`).
3. In `extract_description`: skip lines starting with `<` (and HTML blocks `<!-- ... -->`), `[![`, or `---`.
4. Replace `contains_relevant_files(dir)` with a recursive check up to depth 8 (respecting `IGNORED_DIRS`, dot-directories unless in `ScopeFilter`, and `is_ignored_filename`), and increase `max_depth` in `collect_files_recursive` (line 859 and line 1071) from `4` to `12`.
5. Expand `common_parent` (line 698) with `"libs"`, `"plugins"`, `"profiles"`, `"infra"`, `"home"`, `"linked"`, `"dot_config"`, `"submodules"`. For non-`src` container directories (`infra`, `libs`, `plugins`, `home`, `linked`, `dot_config`), if they have subdirectories with relevant files, add their subdirectories to `dirs_to_inspect`; otherwise add the container directory itself.
6. In `module_entries` and `SplitContext::split`:
   - Mark any file entry whose extension is NOT in `SOURCE_CODE_EXTENSIONS` as `pinned = true` (so `.md`, `.json`, `.yaml` files never split into standalone file subsystems).
   - Do not split pure documentation/report directories (`id == "docs" || id == "reports" || id == "references"`) that have zero `SOURCE_CODE_EXTENSIONS` files.
   - In `SplitContext::split`: if at least one file-only entry (`entry.dir.is_none()`) is promoted and the directory has no pinned code entrypoint, promote all remaining unpinned `SOURCE_CODE_EXTENSIONS` file entries with `entry.tokens >= 300`.
7. In `tokens_if_over` and `file_tokens`: implement `pub fn estimate_tokens_fast(text: &str) -> usize` (`let ascii = text.bytes().filter(|b| b.is_ascii()).count(); let non_ascii = text.len() - ascii; ascii / 4 + non_ascii / 3 + 1`), skip binary files (`is_likely_binary`), and pre-check total file byte sizes (`if total_bytes < (threshold * 3) as u64 { return None; }`) before reading files so `tiktoken_rs` is not initialized on `analyze_project` / `audit` cold start.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p ci-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/ci-core/src/analysis/detector.rs
rtk git commit -m "feat(core): support deep subsystem trees, astro/gradle/maven, fast token estimate, and symmetric split"
```

---

### Task 3: Polyglot Lexical AST Compression & Bilingual Identifier-Aware BM25 Search (`ci-core::analysis::lang` & `ci-core::search`)

**Files:**
- Create: `crates/ci-core/src/analysis/lang/brace.rs`
- Modify: `crates/ci-core/src/analysis/lang/mod.rs:1-50`
- Modify: `crates/ci-core/src/analysis/lang/python.rs:1-140`
- Modify: `crates/ci-core/src/search.rs:85-235`

**Interfaces:**
- Produces:
  - `pub struct BraceLangAdapter` implementing `LanguageAdapter` in `crates/ci-core/src/analysis/lang/brace.rs`
  - Updated `PythonAdapter::compress_source` in `crates/ci-core/src/analysis/lang/python.rs`
  - Conditional `TESTS_SCORE_FACTOR` and `id_stems` match boost in `Bm25Retriever` (`crates/ci-core/src/search.rs`)

- [ ] **Step 1: Write the failing unit tests in `crates/ci-core/src/analysis/lang/mod.rs` and `crates/ci-core/src/search.rs`**

```rust
// In crates/ci-core/src/analysis/lang/mod.rs #[cfg(test)] mod tests
#[test]
fn typescript_and_python_adapters_compress_bodies_over_65_percent() {
    let ts_adapter = adapter_for_ext("ts").expect("ts adapter");
    let mut ts_src = String::from("export interface User {\n  id: string;\n  name: string;\n}\n\nexport async function processUsers(users: User[]): Promise<number> {\n");
    for i in 0..40 {
        ts_src.push_str(&format!("  const v{i} = users.length + {i};\n  if (v{i} > 10) {{ console.log('item', v{i}); }}\n"));
    }
    ts_src.push_str("  return users.length;\n}\n");
    let ts_compressed = ts_adapter.compress_source(&ts_src);
    assert!(ts_compressed.contains("export interface User"));
    assert!(ts_compressed.contains("id: string;"));
    assert!(ts_compressed.contains("export async function processUsers"));
    assert!(ts_compressed.contains(ELIDED_BODY));
    let (ts_before, ts_after) = (count_tokens(&ts_src), count_tokens(&ts_compressed));
    assert!((ts_before - ts_after) * 100 / ts_before >= 65);

    let py_adapter = adapter_for_ext("py").expect("py adapter");
    let mut py_src = String::from("class Processor:\n    def run(self, items: list[int]) -> int:\n        \"\"\"Run processor.\"\"\"\n");
    for i in 0..40 {
        py_src.push_str(&format!("        val_{i} = len(items) + {i}\n        print(val_{i})\n"));
    }
    py_src.push_str("        return len(items)\n");
    let py_compressed = py_adapter.compress_source(&py_src);
    assert!(py_compressed.contains("def run(self, items: list[int]) -> int:"));
    assert!(py_compressed.contains(ELIDED_BODY));
    let (py_before, py_after) = (count_tokens(&py_src), count_tokens(&py_compressed));
    assert!((py_before - py_after) * 100 / py_before >= 65);
}

// In crates/ci-core/src/search.rs #[cfg(test)] mod tests
#[test]
fn search_ranks_test_subsystem_first_when_query_matches_its_id_or_path() {
    let mut index = L0Index::default();
    let mut rep = sub("reports", "Отчёты бенчмарков", "BENCHMARK_REPORT.md — результаты регрессионного бенчмарка", &[]);
    rep.path = "reports".into();
    let mut bench = sub("tests_regression_benchmark", "Tests regression_benchmark", "Регрессионные тесты бенчмарка", &["bench_search_quality_and_latency"]);
    bench.path = "tests/regression_benchmark.rs".into();
    bench.tags = vec!["tests".into()];
    index.subsystems = vec![rep, bench];

    let tmp = tempfile::tempdir().unwrap();
    let retriever = Bm25Retriever::build(&index, tmp.path()).unwrap();
    let hits = retriever.search("benchmark regression", 5).unwrap();
    assert_eq!(hits[0].subsystem_id, "tests_regression_benchmark");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p ci-core typescript_and_python_adapters_compress_bodies_over_65_percent search_ranks_test_subsystem_first_when_query_matches_its_id_or_path`
Expected: FAIL (`adapter_for_ext("ts")` returns `None`; `reports` outranks `tests_regression_benchmark` because `TESTS_SCORE_FACTOR = 0.35` demotes `tests_regression_benchmark`).

- [ ] **Step 3: Implement `BraceLangAdapter`, `PythonAdapter::compress_source`, and conditional `TESTS_SCORE_FACTOR` + `id_stems` boost**

1. Create `crates/ci-core/src/analysis/lang/brace.rs` with `pub struct BraceLangAdapter`:
   - `extensions`: `&["ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "astro", "vue", "svelte", "kt", "kts", "java", "cs", "go"]`.
   - `extract_symbols`: extract exported/public function, class, interface, type, and enum declarations.
   - `compress_source`: track brace depth while ignoring braces inside string/template literals and comments; keep `interface` and `type` blocks intact; when entering a function/method body `{` at depth 1 (or depth 2 inside a `class`/`object`), emit `    ⋮----` and skip lines until the matching closing `}`.
2. Update `crates/ci-core/src/analysis/lang/python.rs`:
   - In `compress_source`, after a `def ` or `async def ` signature (and optional docstring), replace the indented body lines with `<body_indent>⋮----` until indentation returns to `<= def_indent`.
3. Register `static BRACE: BraceLangAdapter = BraceLangAdapter;` in `crates/ci-core/src/analysis/lang/mod.rs` (`all() -> [&'static dyn LanguageAdapter; 4]`).
4. In `crates/ci-core/src/search.rs`:
   - Add `id_stems: HashSet<String>` to `Entry` populated from `stems(&format!("{} {}", sub.id, sub.path))`.
   - In `Bm25Retriever::search`: count `let id_matches = wanted.iter().filter(|t| entry.id_stems.contains(**t)).count();`.
   - Only apply `TESTS_SCORE_FACTOR` when `entry.tags.iter().any(|t| t == TESTS_TAG)` AND `id_matches == 0` AND no query stem is a test/bench term.
   - Multiply `score` by `1.0 + 0.6 * (id_matches as f32)` so subsystems whose `id`/`path` directly match query stems rank at the top.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p ci-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/ci-core/src/analysis/lang/ crates/ci-core/src/search.rs
rtk git commit -m "feat(core): add TS/JS/Kotlin/Java and Python AST compression and calibrate BM25 id/test ranking"
```

---

### Task 4: Zero-Fork Fast-Path Git (`head_commit`, `remote_url`, `.git` gitfile) & Batched `RepoGitSnapshot` (`ci-core::git` & `ci-mcp::pipeline`)

**Files:**
- Modify: `crates/ci-core/src/git.rs:78-475`
- Modify: `crates/ci-mcp/src/pipeline.rs:125-225`

**Interfaces:**
- Produces:
  - `pub fn resolve_git_dir(repo_root: &Path) -> Option<PathBuf>` in `crates/ci-core/src/git.rs`
  - In-process `head_commit` and `remote_url` in `SystemGitBackend` (`crates/ci-core/src/git.rs`)
  - `pub struct RepoGitSnapshot` and `fn repo_snapshot(&self, root: &Path) -> Result<RepoGitSnapshot>` in `crates/ci-core/src/git.rs`
  - `pub fn check_subsystem_staleness_with_snapshot(...) -> Result<StalenessReport>` in `crates/ci-core/src/git.rs`

- [ ] **Step 1: Write the failing unit tests in `crates/ci-core/src/git.rs`**

```rust
#[test]
fn in_process_head_commit_and_remote_url_work_without_git_binary_and_support_gitfile() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let real_git = root.join(".actual_git");
    std::fs::create_dir_all(real_git.join("refs/heads")).unwrap();
    // .git is a gitfile pointing to .actual_git (worktree/submodule style)
    std::fs::write(root.join(".git"), "gitdir: .actual_git\n").unwrap();
    let fake_sha = "0123456789abcdef0123456789abcdef01234567";
    std::fs::write(real_git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::write(real_git.join("refs/heads/main"), format!("{fake_sha}\n")).unwrap();
    std::fs::write(
        real_git.join("config"),
        "[core]\n\trepositoryformatversion = 0\n[remote \"origin\"]\n\turl = git@github.com:org/repo.git\n",
    ).unwrap();

    // Use a non-existent git binary to prove zero-fork in-process parsing!
    let git = SystemGitBackend::with_binary("/nonexistent/git-binary");
    assert_eq!(git.head_commit(root).unwrap(), fake_sha);
    assert_eq!(
        git.remote_url(root).unwrap().as_deref(),
        Some("git@github.com:org/repo.git")
    );
}

#[test]
fn repo_snapshot_matches_individual_staleness_checks() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    run_git(root, &["init"]);
    run_git(root, &["config", "user.email", "t@example.com"]);
    run_git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
    run_git(root, &["add", "."]);
    run_git(root, &["commit", "-m", "init"]);

    let git = SystemGitBackend::new();
    let snap = git.repo_snapshot(root).unwrap();
    let paths = vec!["src".to_string()];
    let sha = git.paths_tree_sha(root, &paths).unwrap();
    let head = git.head_commit(root).unwrap();

    let report = check_subsystem_staleness_with_snapshot(&git, root, &snap, &paths, &head, &sha).unwrap();
    assert_eq!(report.state, StalenessState::Fresh);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p ci-core in_process_head_commit_and_remote_url repo_snapshot_matches_individual`
Expected: FAIL.

- [ ] **Step 3: Implement zero-fork `head_commit`/`remote_url` and batched `RepoGitSnapshot` in `crates/ci-core/src/git.rs` and `crates/ci-mcp/src/pipeline.rs`**

1. In `crates/ci-core/src/git.rs`:
   - Implement `pub fn resolve_git_dir(repo_root: &Path) -> Option<PathBuf>` supporting both `.git/` directory and `.git` file (`gitdir: ...`), plus `resolve_common_git_dir` (checking `<git_dir>/commondir` for worktrees).
   - Implement `read_head_commit_in_process(repo_root: &Path) -> Option<String>` reading `HEAD`, loose ref `<common_dir>/<ref>`, or `<common_dir>/packed-refs`, called first in `SystemGitBackend::head_commit` before falling back to `self.run_git`.
   - Implement `read_remote_url_in_process(repo_root: &Path) -> Option<String>` parsing `[remote "origin"]` (or first `[remote "..."]`) `url = ...` from `<common_dir>/config`, called first in `SystemGitBackend::remote_url` before falling back to `self.run_git`.
   - Implement `RepoGitSnapshot`, `repo_snapshot(&self, repo_root: &Path)`, and `check_subsystem_staleness_with_snapshot`.
2. In `crates/ci-mcp/src/pipeline.rs`:
   - Wire `git.repo_snapshot(root)` into `run_check` so checking all cards uses a single snapshot instead of $3 \times N$ spawns.
   - Update `storage_mismatch_hint` to detect `.agents/knowledge` legacy bases.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p ci-core -p ci-mcp`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/ci-core/src/git.rs crates/ci-mcp/src/pipeline.rs
rtk git commit -m "perf(git): add zero-fork HEAD/remote_url parser and batched RepoGitSnapshot staleness check"
```

---

### Task 5: MCP Multi-Project Root Resolution, Parallel Symlink Workspace & Schema/Stale-Binary Diagnostics (`ci-core` & `ci-mcp`)

**Files:**
- Modify: `crates/ci-core/src/config.rs:24-120`
- Modify: `crates/ci-core/src/storage.rs:470-505`
- Modify: `crates/ci-core/src/analysis/workspace.rs:85-155`
- Modify: `crates/ci-mcp/src/server.rs:135-355`

**Interfaces:**
- Produces:
  - `pub workspace_roots: Vec<PathBuf>` on `Config` in `crates/ci-core/src/config.rs`
  - Forward-compatible `L0Index::from_json_str` in `crates/ci-core/src/storage.rs`
  - Parallel (`std::thread::scope`), symlink-aware, and nested-repo-aware `discover_workspace` in `crates/ci-core/src/analysis/workspace.rs`
  - Multi-workspace `resolve_project_root`, `child_repo`, and error-path `stale_binary_footer` in `crates/ci-mcp/src/server.rs`

- [ ] **Step 1: Write the failing unit tests in `crates/ci-core/src/storage.rs` and `crates/ci-mcp/src/server.rs`**

```rust
// In crates/ci-core/src/storage.rs #[cfg(test)] mod tests
#[test]
fn l0_index_reads_newer_schema_version_in_forward_compatible_mode() {
    let mut idx = L0Index::default();
    idx.project = "demo".into();
    let mut val: serde_json::Value = serde_json::to_value(&idx).unwrap();
    val["schema_version"] = serde_json::json!(CURRENT_L0_SCHEMA_VERSION + 1);
    let parsed = L0Index::from_json_str(&val.to_string()).expect("forward-compatible load");
    assert_eq!(parsed.project, "demo");
}

// In crates/ci-mcp/src/server.rs #[cfg(test)] mod tests
#[tokio::test]
async fn resolve_project_root_finds_sibling_and_workspace_root_projects_without_false_upward_match() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path();
    let repo_a = ws.join("context-init");
    let repo_b = ws.join("walle");
    std::fs::create_dir_all(repo_a.join(".git")).unwrap();
    std::fs::create_dir_all(repo_b.join(".git")).unwrap();

    let server = ContextInitServer::new(Some(repo_a.clone()));
    let resolved_by_project = server.resolve_project_root(None, Some("walle")).await;
    assert_eq!(resolved_by_project, repo_b);
    let resolved_by_rel_path = server.resolve_project_root(Some("walle"), None).await;
    assert_eq!(resolved_by_rel_path, repo_b);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p ci-core l0_index_reads_newer_schema_version -p ci-mcp resolve_project_root_finds_sibling`
Expected: FAIL.

- [ ] **Step 3: Implement `workspace_roots`, forward-compatible `L0Index`, parallel symlink `discover_workspace`, and fixed `resolve_project_root` / `instrumented`**

1. In `crates/ci-core/src/config.rs`: add `#[serde(default = "default_workspace_roots")] pub workspace_roots: Vec<PathBuf>` to `Config` with `default_workspace_roots()` returning `/data/projects` and canonicalized `$HOME/projects` when they exist.
2. In `crates/ci-core/src/storage.rs` (`L0Index::from_json_str`): only `bail!` when `found < u64::from(MIN_MIGRATABLE_L0_SCHEMA_VERSION)`. When `found > u64::from(CURRENT_L0_SCHEMA_VERSION)`, log `tracing::warn!` and deserialize in forward-compatible mode.
3. In `crates/ci-core/src/analysis/workspace.rs` (`discover_workspace`):
   - Replace `e.file_type().is_ok_and(|t| t.is_dir())` with `e.path().is_dir()` (follows directory symlinks like `~/projects -> /data/projects`) and skip hidden directories (`name.starts_with('.')`).
   - If a top-level directory does not have `.git`, inspect its immediate subdirectories for `.git` (`wasmbridge-projects/*`).
   - Run `inspect_repo` across all discovered candidate directories in parallel using `std::thread::scope`.
4. In `crates/ci-mcp/src/server.rs`:
   - In `resolve_project_root`: when `candidate_arg` is relative, only call `find_project_root_upward(&candidate)` if `candidate.exists()`; if `!candidate.exists()`, try `self.child_repo(candidate_arg).await`.
   - In `child_repo`: search `<roots>/<name>`, `<default_root>/<name>`, `<parent_of_roots>/<name>`, and `default_workspace_roots()/<name>`.
   - In `instrumented`: also append `stale_binary_footer()` to the error message in the `Err(e)` branch so stale MCP processes always surface the reload warning.

- [ ] **Step 4: Run all workspace and MCP tests to verify they pass**

Run: `rtk cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/ci-core/src/config.rs crates/ci-core/src/storage.rs crates/ci-core/src/analysis/workspace.rs crates/ci-mcp/src/server.rs
rtk git commit -m "fix(mcp): resolve sibling and /data/projects repos, parallelize workspace scan, and allow newer L0 schema"
```

---

### Task 6: Production Rollout & Verification Across `/data/projects`

**Files:**
- Modify: `tests/regression_benchmark.rs`
- Modify: `.context-init/knowledge/` (via `context-init update`)
- Modify: `AGENTS.md` (via `context-init update`)

**Interfaces:**
- Consumes: All deliverables from Tasks 1–5.
- Produces: Installed `context-init` binary in `~/.cargo/bin/context-init`, updated `.context-init/knowledge/` in `context-init`, migrated bases in `ai-dotfiles`, `dotfiles`, `obsidian-orbit`, and verified SLOs across `/data/projects`.

- [ ] **Step 1: Add polyglot AST compression and `/data/projects` regression assertions to `tests/regression_benchmark.rs`**

Add assertions in `tests/regression_benchmark.rs` verifying:
- `recall@5 == 1.0` on `bench_search_quality_and_latency` (including `"benchmark regression"` -> `tests_regression_benchmark`).
- Cold-start first iteration of `bench_drift_audit_latency` $< 25.0\text{ ms}$.
- TypeScript and Python AST compression savings $\ge 65\%$.

- [ ] **Step 2: Refresh `context-init` knowledge base and run the full test and benchmark suite**

Run:
```bash
rtk cargo run -- update . --role update=none
rtk cargo test --workspace
rtk cargo test --test regression_benchmark -- --nocapture
```
Expected: All unit, integration, and benchmark regression tests PASS with `recall@5 = 1.00`.

- [ ] **Step 3: Install updated binary and verify `/data/projects` targets**

Run:
```bash
rtk cargo install --path .
rtk context-init check /data/projects/context-init --compact
rtk context-init workspace /data/projects
rtk context-init init /data/projects/walle --dry-run --role init=none
rtk context-init init /data/projects/wasmbridge-projects --dry-run --role init=none
rtk context-init init /data/projects/ai-dotfiles --dry-run --role init=none
rtk context-init init /data/projects/nestjs-zkbio-cvsecurity --dry-run --role init=none
rtk context-init init /data/projects/resume_1c --dry-run --role init=none
rtk context-init init /data/projects/mimir --dry-run --role init=none
```
Expected:
- `walle` subsystems $\le 18$ (zero `.agent` subsystems).
- `wasmbridge-projects` subsystems $\le 15$ (zero `docs/OpenViking` subsystems, zero `14.5M` binary tokens).
- `ai-dotfiles` includes `infra_*` subsystems.
- `nestjs-zkbio-cvsecurity` includes `libs_zkbio-client`.
- `resume_1c` includes `src_components` / `src_pages`.
- `mimir` detected as `Kotlin`.

- [ ] **Step 4: Commit updated knowledge base and plan artifacts**

```bash
rtk git status
rtk git add .context-init/knowledge AGENTS.md tests/regression_benchmark.rs docs/superpowers/
rtk git commit -m "chore(kb): update knowledge base and regression benchmarks for /data/projects readiness"
```
