use super::*;

#[test]
fn test_parse_cargo_workspace() {
    let toml_sample = r#"
[workspace]
members = [
"crates/core",
"crates/cli",
]

[package]
name = "my-workspace"
version = "0.1.0"
"#;
    let temp_dir = std::env::temp_dir().join("test_cargo_ws");
    let _ = fs::create_dir_all(&temp_dir);
    let file_path = temp_dir.join("Cargo.toml");
    fs::write(&file_path, toml_sample).unwrap();

    let (name, members) = parse_cargo_toml(&file_path);
    assert_eq!(name, Some("my-workspace".to_string()));
    assert_eq!(members, vec!["crates/core", "crates/cli"]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_parse_package_json() {
    let pkg_sample = r#"{
  "name": "@my-org/web",
  "workspaces": ["packages/*", "apps/*"]
}"#;
    let temp_dir = std::env::temp_dir().join("test_pkg_ws");
    let _ = fs::create_dir_all(&temp_dir);
    let file_path = temp_dir.join("package.json");
    fs::write(&file_path, pkg_sample).unwrap();

    let (name, members) = parse_package_json(&file_path);
    assert_eq!(name, Some("@my-org/web".to_string()));
    assert_eq!(members, vec!["packages/*", "apps/*"]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_parse_pnpm_workspace_yaml() {
    let pnpm_sample = r#"# PNPM Workspace Configuration
packages:
  # Internal plugins
  - 'plugins/*' # inline comment
  - "packages/*"
  - apps/web
  - '!**/tests/**'
# Column 0 comment
  - modules/legacy

catalog:
  typescript: 7.0.2
"#;
    let temp_dir = std::env::temp_dir().join("test_pnpm_ws");
    let _ = fs::create_dir_all(&temp_dir);
    let file_path = temp_dir.join("pnpm-workspace.yaml");
    fs::write(&file_path, pnpm_sample).unwrap();

    let members = parse_pnpm_workspace_yaml(&file_path);
    assert_eq!(
        members,
        vec!["plugins/*", "packages/*", "apps/web", "modules/legacy"]
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_clean_symbol_line() {
    assert_eq!(
        clean_symbol_line("pub fn process_data(input: &str) -> Result<()> {"),
        "pub fn process_data(input: &str) -> Result<()>"
    );
    assert_eq!(
        clean_symbol_line("export class ContextEngine {"),
        "export class ContextEngine"
    );
}

#[test]
fn test_rust_syn_extraction() {
    let code = r#"
pub struct Engine<T: Clone> {
data: T,
}

pub enum Status {
Active,
Idle,
}

pub async fn run_task<T>(id: &str, payload: T) -> Result<Status, Error> {
Ok(Status::Active)
}
"#;
    let symbols = extract_rust_symbols(code, true);
    assert!(symbols.iter().any(
        |s| s.contains("struct Engine < T : Clone >") || s.contains("struct Engine<T: Clone>")
    ));
    assert!(symbols.iter().any(|s| s.contains("enum Status")));
    assert!(symbols.iter().any(|s| s.contains("run_task")
        && s.contains("Result < Status , Error >")
        || s.contains("Result<Status, Error>")));
}

#[test]
fn test_multiline_ts_extraction() {
    let code = r#"
export async function initializeClient<T>(
endpoint: string,
timeoutMs: number
): Promise<Client<T>> {
return new Client();
}
"#;
    let symbols = extract_multiline_symbols(code, "ts", true);
    assert_eq!(symbols.len(), 1);
    assert!(symbols[0].contains("initializeClient<T>"));
    assert!(symbols[0].contains("endpoint: string"));
    assert!(symbols[0].contains("timeoutMs: number"));
    assert!(symbols[0].contains("Promise<Client<T>>"));
}

#[test]
fn test_multiline_python_extraction() {
    let code = r#"
def execute_pipeline(
config: PipelineConfig,
debug: bool = False,
) -> ExecutionResult:
pass
"#;
    let symbols = extract_multiline_symbols(code, "py", true);
    assert_eq!(symbols.len(), 1);
    assert!(symbols[0].contains("execute_pipeline"));
    assert!(symbols[0].contains("config: PipelineConfig"));
    assert!(symbols[0].contains("ExecutionResult"));
}

#[test]
fn test_secret_sanitizer() {
    assert_eq!(
        sanitize_symbol("pub const API_KEY: &str = \"sk-1234567890abcdef123456\";"),
        Some("pub const API_KEY: &str = \"[REDACTED]\"".to_string())
    );
    assert_eq!(
        sanitize_symbol("export const GITHUB_TOKEN = \"ghp_1234567890abcdef1234567890\";"),
        Some("export const GITHUB_TOKEN = \"[REDACTED]\"".to_string())
    );
    assert_eq!(
        sanitize_symbol("export const AUTH = \"Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.xyz\";"),
        Some("export const AUTH = \"[REDACTED]\"".to_string())
    );
    assert_eq!(
        sanitize_symbol(
            "pub fn set_header(val: &str = \"Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.xyz\")"
        ),
        Some("pub fn set_header(val: &str = \"Bearer [REDACTED]\")".to_string())
    );
    assert_eq!(sanitize_symbol("-----BEGIN RSA PRIVATE KEY-----"), None);
}

#[test]
fn test_rtk_prefixing() {
    assert_eq!(prefix_with_rtk("cargo test"), "rtk cargo test");
    assert_eq!(prefix_with_rtk("rtk cargo test"), "rtk cargo test");
    assert_eq!(prefix_with_rtk(""), "");
}

#[test]
fn test_count_tokens() {
    let tokens = count_tokens("Hello world! Context-init BPE tokenometry.");
    assert!(tokens > 0);
}

#[test]
fn test_multiline_go_extraction() {
    let code = r#"
func ProcessRequest(
ctx context.Context,
req *DataRequest,
) (*DataResponse, error) {
return nil, nil
}
"#;
    let symbols = extract_multiline_symbols(code, "go", true);
    assert_eq!(symbols.len(), 1);
    assert!(symbols[0].contains("func ProcessRequest"));
    assert!(symbols[0].contains("ctx context.Context"));
    assert!(symbols[0].contains("(*DataResponse, error)"));
}

#[test]
fn test_rust_impl_and_trait_extraction() {
    let code = r#"
pub trait Service {
fn handle(&self, req: Request) -> Response;
}

pub struct MyService;

impl MyService {
pub fn new() -> Self {
    Self
}
fn private_helper(&self) {}
}
"#;
    let symbols = extract_rust_symbols(code, true);
    assert!(symbols.iter().any(|s| s.contains("trait Service")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("fn handle") && s.contains("&self") && s.contains("Response")));
    assert!(symbols.iter().any(|s| s.contains("struct MyService")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("pub fn new") && s.contains("-> Self")));
    // private_helper should not be extracted
    assert!(!symbols.iter().any(|s| s.contains("private_helper")));
}

#[test]
fn test_secret_sanitizer_edge_cases() {
    // SSH key
    assert_eq!(
        sanitize_symbol("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI..."),
        None
    );
    // PGP Private
    assert_eq!(
        sanitize_symbol("-----BEGIN PGP PRIVATE KEY BLOCK-----"),
        None
    );
    // Slack token
    let slack = "pub const SLACK_TOKEN: &str = \"xoxb-1234567890-1234567890123-abcdef\";";
    assert_eq!(
        sanitize_symbol(slack),
        Some("pub const SLACK_TOKEN: &str = \"[REDACTED]\"".to_string())
    );
    // Regular harmless const
    let normal = "pub const MAX_RETRIES: usize = 5;";
    assert_eq!(
        sanitize_symbol(normal),
        Some("pub const MAX_RETRIES: usize = 5;".to_string())
    );
}

#[test]
fn test_normalize_signature_whitespace() {
    let sig = "pub fn count_tokens ( text : & str ) -> usize";
    assert_eq!(
        normalize_signature_whitespace(sig),
        "pub fn count_tokens(text: &str) -> usize"
    );

    let sig_method = "pub fn parse < 'a > ( input : & 'a str ) -> ( & 'a str , usize )";
    assert_eq!(
        normalize_signature_whitespace(sig_method),
        "pub fn parse<'a>(input: &'a str) -> (&'a str, usize)"
    );

    let sig_mut = "pub fn mutate ( & mut self , val : & mut String ) -> Result < ( ) , Error >";
    assert_eq!(
        normalize_signature_whitespace(sig_mut),
        "pub fn mutate(&mut self, val: &mut String) -> Result<(), Error>"
    );

    let sig_trait = "pub fn bound ( ) -> impl Trait + ( Send )";
    assert_eq!(
        normalize_signature_whitespace(sig_trait),
        "pub fn bound() -> impl Trait + (Send)"
    );
}

#[test]
fn test_clean_multiline_signature_truncation() {
    let long_ts = format!(
        "export async function processData(param1: {}, param2: String): Promise<Result>",
        "A".repeat(300)
    );
    let cleaned = clean_multiline_signature(&long_ts, "ts");
    assert_eq!(
        cleaned,
        "export async function processData(...): Promise<Result>"
    );

    let long_py = format!(
        "async def handle_request(arg1: {}, arg2: int) -> bool:",
        "B".repeat(300)
    );
    let cleaned_py = clean_multiline_signature(&long_py, "py");
    assert_eq!(cleaned_py, "async def handle_request(...) -> bool");
}

#[test]
fn test_ignored_dirs_in_subsystem_discovery() {
    let temp_dir = std::env::temp_dir().join(format!(
        "test_ignored_dirs_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&temp_dir);

    // Create ignored directories with files in them
    let ignored_test_dirs = [
        ".vault",
        ".turbo",
        "out",
        ".svelte-kit",
        ".astro",
        ".parcel-cache",
        ".context-init",
        "node_modules",
        "target",
    ];
    for dir_name in &ignored_test_dirs {
        let dir_path = temp_dir.join(dir_name);
        let _ = fs::create_dir_all(&dir_path);
        let _ = fs::write(dir_path.join("file.ts"), "export const x = 1;");
        let _ = fs::write(dir_path.join("main.rs"), "fn main() {}");

        // Also test nested directories inside ignored dirs
        let nested_dir = dir_path.join("nested_sub");
        let _ = fs::create_dir_all(&nested_dir);
        let _ = fs::write(nested_dir.join("sub.rs"), "pub fn sub() {}");
    }

    // Test ignored directories inside packages/
    let pkg_vault = temp_dir.join("packages").join(".vault");
    let _ = fs::create_dir_all(&pkg_vault);
    let _ = fs::write(pkg_vault.join("plugin.ts"), "export const p = 1;");

    // Create a legitimate subsystem directory
    let legit_dir = temp_dir.join("src").join("core");
    let _ = fs::create_dir_all(&legit_dir);
    let _ = fs::write(legit_dir.join("mod.rs"), "pub fn run() {}");

    // Also test root-level file with workspace members pointing into ignored directories
    let _ = fs::write(
        temp_dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\".vault/nested_sub\", \"packages/.vault\"]\n\n[package]\nname = \"dummy\"\nversion = \"0.1.0\"\n",
    );

    let analysis = analyze_project(&temp_dir, false, false).expect("Analysis should succeed");

    assert!(
        !analysis.detected_modules.is_empty(),
        "Legitimate subsystems must be discovered"
    );
    assert!(
        analysis
            .detected_modules
            .iter()
            .any(|m| m.name == "core" || m.dir_path == "src/core"),
        "Module 'core' should have been discovered"
    );

    for m in &analysis.detected_modules {
        for ignored in &ignored_test_dirs {
            assert!(
                !m.dir_path.split('/').any(|seg| seg == *ignored) && !m.name.contains(ignored),
                "Module {} (path: {}) should not have matched ignored dir {}",
                m.name,
                m.dir_path,
                ignored
            );
        }
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_workspace_root_dot_member_does_not_create_empty_subsystem() {
    let temp_dir = std::env::temp_dir().join(format!(
        "test_ws_dot_member_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let crate_dir = temp_dir.join("crates").join("ci-core").join("src");
    let root_src = temp_dir.join("src");
    let _ = fs::create_dir_all(&crate_dir);
    let _ = fs::create_dir_all(&root_src);

    let _ = fs::write(
        temp_dir.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/ci-core\", \".\"]\n\n[package]\nname = \"context-init\"\nversion = \"0.3.0\"\n",
    );
    let _ = fs::write(
        temp_dir.join("crates").join("ci-core").join("Cargo.toml"),
        "[package]\nname = \"ci-core\"\nversion = \"0.3.0\"\n",
    );
    let _ = fs::write(crate_dir.join("lib.rs"), "pub fn core_fn() {}");
    let _ = fs::write(root_src.join("main.rs"), "fn main() {}");

    let analysis = analyze_project(&temp_dir, false, false).expect("Analysis should succeed");
    assert!(
        analysis
            .detected_modules
            .iter()
            .all(|m| !m.name.is_empty() && !m.dir_path.is_empty() && m.dir_path != "."),
        "No module should have an empty name or '.' dir_path: {:?}",
        analysis.detected_modules
    );
    assert!(
        analysis
            .detected_modules
            .iter()
            .any(|m| m.dir_path == "crates/ci-core"),
        "crates/ci-core must be discovered"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_flat_src_merges_main_and_lib_into_single_core_module() {
    let temp_dir = std::env::temp_dir().join(format!(
        "test_flat_src_core_merge_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let root_src = temp_dir.join("src");
    let _ = fs::create_dir_all(&root_src);

    let _ = fs::write(
        temp_dir.join("Cargo.toml"),
        "[package]\nname = \"dual-target\"\nversion = \"0.3.0\"\n",
    );
    let _ = fs::write(
        root_src.join("main.rs"),
        "pub fn run_cli() {}\nfn main() {}",
    );
    let _ = fs::write(root_src.join("lib.rs"), "pub fn run_lib() {}");
    let _ = fs::write(root_src.join("detector.rs"), "pub fn detect() {}");
    let _ = fs::write(root_src.join("service.rs"), "pub fn serve() {}");

    let analysis = analyze_project(&temp_dir, false, false).expect("Analysis should succeed");
    let core_modules: Vec<_> = analysis
        .detected_modules
        .iter()
        .filter(|m| m.dir_path == "src/core")
        .collect();

    assert_eq!(
        core_modules.len(),
        1,
        "main.rs and lib.rs must be merged into a single src/core module, got: {:?}",
        core_modules
    );
    let core = core_modules[0];
    assert_eq!(core.name, "core");
    assert_eq!(core.files.len(), 2);
    assert!(core.files.iter().any(|f| f.path == "src/lib.rs"));
    assert!(core.files.iter().any(|f| f.path == "src/main.rs"));
    assert!(core.entrypoints.contains(&"src/lib.rs".to_string()));
    assert!(core.entrypoints.contains(&"src/main.rs".to_string()));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_parse_cargo_toml_workspace_inheritance_and_sections() {
    let temp_dir = std::env::temp_dir().join(format!(
        "test_parse_cargo_ws_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let subcrate_dir = temp_dir.join("crates").join("ci-core");
    let _ = fs::create_dir_all(subcrate_dir.join("src"));

    let root_toml = temp_dir.join("Cargo.toml");
    let _ = fs::write(
        &root_toml,
        "[workspace]\nresolver = \"2\"\nmembers = [\n    \"crates/ci-core\", # core crate\n    \".\",\n]\n\n[workspace.package]\nversion = \"0.3.0\"\nedition = \"2021\"\n\n[workspace.metadata.custom]\nname = \"should-not-match-metadata-name\"\n\n[package]\nname = \"context-init\"\nversion.workspace = true\nedition.workspace = true\n",
    );

    let subcrate_toml = subcrate_dir.join("Cargo.toml");
    let _ = fs::write(
        &subcrate_toml,
        "[package]\nname = \"ci-core\"\nversion.workspace = true\nedition.workspace = true\n",
    );
    let _ = fs::write(
        subcrate_dir.join("src").join("lib.rs"),
        "pub const V: &str = \"0.3.0\";",
    );

    let (root_name, members) = parse_cargo_toml(&root_toml);
    assert_eq!(root_name.as_deref(), Some("context-init"));
    assert_eq!(members, vec!["crates/ci-core".to_string(), ".".to_string()]);

    let (sub_name, sub_members) = parse_cargo_toml(&subcrate_toml);
    assert_eq!(sub_name.as_deref(), Some("ci-core"));
    assert!(sub_members.is_empty());

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Rust source of about `n` BPE tokens.
fn rust_source(prefix: &str, n: usize) -> String {
    let mut out = String::new();
    let mut tokens = 0;
    let mut i = 0;
    while tokens < n {
        let line = format!("pub fn {prefix}_{i}() {{}}\n");
        tokens += count_tokens(&line);
        out.push_str(&line);
        i += 1;
    }
    out
}

/// Workspace with one crate `crates/foo`: big `analysis/` dir (with a big `inner/`),
/// big `storage.rs`, small `tiny.rs` and `lib.rs`. Returns the temp root.
fn split_fixture(tag: &str) -> PathBuf {
    let temp_dir = std::env::temp_dir().join(format!(
        "test_crate_split_{tag}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let src = temp_dir.join("crates").join("foo").join("src");
    let inner = src.join("analysis").join("inner");
    fs::create_dir_all(&inner).unwrap();
    fs::write(
        temp_dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/foo\"]\n",
    )
    .unwrap();
    fs::write(
        temp_dir.join("crates").join("foo").join("Cargo.toml"),
        "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(src.join("lib.rs"), "pub mod analysis;\npub mod storage;\n").unwrap();
    fs::write(src.join("tiny.rs"), rust_source("tiny", 300)).unwrap();
    fs::write(src.join("storage.rs"), rust_source("storage", 1000)).unwrap();
    fs::write(src.join("analysis").join("mod.rs"), "pub mod inner;\n").unwrap();
    fs::write(src.join("analysis").join("a.rs"), rust_source("a", 400)).unwrap();
    fs::write(src.join("analysis").join("b.rs"), rust_source("b", 400)).unwrap();
    fs::write(inner.join("mod.rs"), rust_source("inner", 700)).unwrap();
    temp_dir
}

fn analyze_with_threshold(root: &Path, threshold: usize) -> ProjectAnalysis {
    analyze_project_with(
        root,
        &AnalyzeOptions {
            deep: false,
            with_rtk: false,
            split_threshold_tokens: threshold,
            scope: Default::default(),
        },
    )
    .expect("Analysis should succeed")
}

fn module_tokens(root: &Path, m: &DiscoveredModule) -> usize {
    m.files
        .iter()
        .map(|f| count_tokens(&fs::read_to_string(root.join(&f.path)).unwrap()))
        .sum()
}

#[test]
fn test_crate_under_threshold_keeps_single_stable_subsystem() {
    let temp_dir = split_fixture("small");
    let analysis = analyze_with_threshold(&temp_dir, 1_000_000);
    let crate_modules: Vec<&DiscoveredModule> = analysis
        .detected_modules
        .iter()
        .filter(|m| m.dir_path.starts_with("crates/foo"))
        .collect();
    assert_eq!(crate_modules.len(), 1, "small crate must not be split");
    assert_eq!(crate_modules[0].id, "crates_foo");
    assert_eq!(crate_modules[0].dir_path, "crates/foo");
    assert_eq!(crate_modules[0].parent, None);

    // The default entry point matches the default threshold and keeps the same id.
    let default = analyze_project(&temp_dir, false, false).unwrap();
    assert!(default
        .detected_modules
        .iter()
        .any(|m| m.id == "crates_foo"));
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_crate_over_threshold_is_split_into_module_subsystems() {
    let temp_dir = split_fixture("big");
    let threshold = 1100;
    let analysis = analyze_with_threshold(&temp_dir, threshold);
    let ids: Vec<&str> = analysis
        .detected_modules
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    for expected in [
        "crates_foo",
        "crates_foo_storage",
        "crates_foo_analysis",
        "crates_foo_analysis_inner",
    ] {
        assert!(ids.contains(&expected), "missing {expected} in {ids:?}");
    }
    assert!(
        !ids.contains(&"crates_foo_tiny"),
        "small files stay in the crate root subsystem: {ids:?}"
    );

    let root = analysis
        .detected_modules
        .iter()
        .find(|m| m.id == "crates_foo")
        .unwrap();
    assert_eq!(root.parent, None, "root keeps the crate's stable id");
    let root_paths: Vec<&str> = root.files.iter().map(|f| f.path.as_str()).collect();
    assert!(root_paths.contains(&"crates/foo/src/lib.rs"));
    assert!(root_paths.contains(&"crates/foo/src/tiny.rs"));
    assert!(root_paths.contains(&"crates/foo/Cargo.toml"));

    let analysis_mod = analysis
        .detected_modules
        .iter()
        .find(|m| m.id == "crates_foo_analysis")
        .unwrap();
    assert_eq!(analysis_mod.parent.as_deref(), Some("crates_foo"));
    assert!(analysis_mod
        .files
        .iter()
        .any(|f| f.path == "crates/foo/src/analysis/mod.rs"));
    let inner = analysis
        .detected_modules
        .iter()
        .find(|m| m.id == "crates_foo_analysis_inner")
        .unwrap();
    assert_eq!(inner.parent.as_deref(), Some("crates_foo_analysis"));

    // Every file lands in exactly one subsystem; none exceeds the threshold.
    let mut seen = std::collections::BTreeSet::new();
    for m in analysis
        .detected_modules
        .iter()
        .filter(|m| m.dir_path.starts_with("crates/foo"))
    {
        crate::storage::validate_subsystem_id(&m.id).unwrap();
        assert!(
            module_tokens(&temp_dir, m) <= threshold,
            "{} exceeds the threshold",
            m.id
        );
        for f in &m.files {
            assert!(seen.insert(f.path.clone()), "{} packed twice", f.path);
        }
    }
    assert_eq!(seen.len(), 8);

    // Deterministic: same input, same output.
    let again = analyze_with_threshold(&temp_dir, threshold);
    let again_ids: Vec<&str> = again
        .detected_modules
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(ids, again_ids);
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_split_merges_file_and_dir_with_same_stem() {
    let temp_dir = split_fixture("stem");
    let src = temp_dir.join("crates").join("foo").join("src");
    fs::write(src.join("analysis.rs"), "pub fn glue() {}\n").unwrap();
    let analysis = analyze_with_threshold(&temp_dir, 1000);
    let m = analysis
        .detected_modules
        .iter()
        .find(|m| m.id == "crates_foo_analysis")
        .unwrap();
    assert!(m
        .files
        .iter()
        .any(|f| f.path == "crates/foo/src/analysis.rs"));
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_split_threshold_zero_disables_splitting() {
    let temp_dir = split_fixture("off");
    let analysis = analyze_with_threshold(&temp_dir, 0);
    assert!(analysis.detected_modules.iter().all(|m| m.parent.is_none()));
    assert!(analysis
        .detected_modules
        .iter()
        .any(|m| m.id == "crates_foo"));
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn representative_files_sorted_by_path() {
    let file = |path: &str, lines: usize, is_entrypoint: bool| SubsystemFile {
        path: path.to_string(),
        is_entrypoint,
        lines,
    };
    let paths =
        |files: Vec<SubsystemFile>| -> Vec<String> { files.into_iter().map(|f| f.path).collect() };
    let order = vec!["main.rs", "a.rs", "b.rs"];
    let before = representative_files(vec![
        file("b.rs", 10, false),
        file("a.rs", 500, false),
        file("main.rs", 1, true),
    ]);
    assert_eq!(paths(before), order);
    let after = representative_files(vec![
        file("b.rs", 900, false),
        file("a.rs", 500, false),
        file("main.rs", 1, true),
    ]);
    assert_eq!(paths(after), order);
}

#[test]
fn detector_honors_gitignore_scope_and_symlinks() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("det_scope_{}_{nanos}", std::process::id()));
    let root = base.join("proj");
    for (rel, text) in [
        ("proj/.gitignore", "generated/\n"),
        ("proj/src/api/mod.rs", "pub fn api() {}\n"),
        ("proj/src/store/mod.rs", "pub fn store() {}\n"),
        ("proj/generated/mod.rs", "pub fn generated() {}\n"),
        ("outside/leak/mod.rs", "pub fn leak() {}\n"),
    ] {
        let p = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(base.join("outside/leak"), root.join("linked")).unwrap();

    let analysis = analyze_project(&root, false, false).unwrap();
    let ids: Vec<&str> = analysis
        .detected_modules
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert!(
        !ids.iter()
            .any(|i| i.contains("generated") || i.contains("linked")),
        "{ids:?}"
    );
    assert!(ids.iter().any(|i| i.contains("api")), "{ids:?}");

    let scoped = analyze_project_with(
        &root,
        &AnalyzeOptions {
            scope: ScopeConfig {
                exclude: vec!["src/store/**".into()],
                ..ScopeConfig::default()
            },
            ..AnalyzeOptions::default()
        },
    )
    .unwrap();
    let ids: Vec<&str> = scoped
        .detected_modules
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert!(!ids.iter().any(|i| i.contains("store")), "{ids:?}");
    let _ = fs::remove_dir_all(&base);
}

fn write_tree(files: &[(&str, &str)]) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("det_tests_{}_{nanos}", std::process::id()));
    for (rel, text) in files {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    root
}

#[test]
fn is_test_path_matches_known_shapes() {
    for p in [
        "tests/cli.rs",
        "crates/a/tests/it.rs",
        "src/a/tests.rs",
        "tests.rs",
        "src/x_test.rs",
        "src/x_tests.rs",
        "pkg/x_test.py",
        "pkg/test_x.py",
        "src\\a\\tests.rs",
        "tests\\it.rs",
        "attest_test.rs",
        "core/src/commonTest/kotlin/MainTest.kt",
        "core/src/jvmTest/kotlin/MainTests.kt",
        "core/src/jsTest/kotlin/MainSpec.kt",
        "core/src/iosTest/kotlin/Fakes.kt",
        "core/src/androidTest/kotlin/Helper.kt",
        "app/src/test/java/ExampleTest.java",
        "app/src/test/java/ExampleTestCase.java",
        "src/components/Button.test.tsx",
        "src/components/Button.test.ts",
        "src/components/Button.spec.js",
        "pkg/server_test.go",
        "tests/e2e/login.spec.ts",
        "__tests__/utils.js",
        "specs/feature.js",
    ] {
        assert!(is_test_path(p), "{p}");
    }
    for p in [
        "src/a.rs",
        "src/contest.rs",
        "src/latest.py",
        "src/testing/mod.rs",
        "src/contests/x.rs",
        "conftest.py",
        "test_utils.rs",
    ] {
        assert!(!is_test_path(p), "{p}");
    }
}

#[test]
fn test_files_contribute_no_symbols() {
    let root = write_tree(&[
        (
            "Cargo.toml",
            "[package]\nname = \"p\"\nversion = \"0.1.0\"\n",
        ),
        ("src/a/mod.rs", "pub fn real() {}\n"),
        ("src/a/tests.rs", "pub struct Engine;\n"),
    ]);
    let analysis = analyze_project(&root, false, false).unwrap();
    let all: Vec<String> = analysis
        .detected_modules
        .iter()
        .flat_map(|m| m.symbols.clone())
        .collect();
    assert!(!all.iter().any(|s| s.contains("Engine")), "{all:?}");
    assert!(all.iter().any(|s| s.contains("real")), "{all:?}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cfg_test_module_symbols_skipped() {
    let src = "pub fn real() {}\n#[cfg(test)]\nmod t {\n    pub struct Fake;\n}\n\
               #[cfg(test)]\npub struct Fake;\n#[cfg(test)]\npub fn fake_helper() {}\n\
               #[cfg(all(test, unix))]\npub fn fake_unix() {}\n";
    let syms = extract_rust_symbols(src, false);
    assert_eq!(syms.len(), 1, "{syms:?}");
    assert!(syms[0].contains("real"));
}

#[test]
fn subsystem_without_files_is_not_tagged_tests() {
    let root = write_tree(&[
        (
            "Cargo.toml",
            "[package]\nname = \"p\"\nversion = \"0.1.0\"\n",
        ),
        ("src/api/mod.rs", "pub fn api() {}\n"),
    ]);
    let mut analysis = analyze_project(&root, false, false).unwrap();
    for m in &mut analysis.detected_modules {
        m.files.clear();
    }
    let subs = crate::analysis::subsystems::build_deterministic(&analysis);
    assert!(!subs.is_empty());
    assert!(subs.iter().all(|s| !s.tags.iter().any(|t| t == "tests")));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_only_subsystem_tagged() {
    let root = write_tree(&[
        (
            "Cargo.toml",
            "[package]\nname = \"p\"\nversion = \"0.1.0\"\n",
        ),
        ("src/api/mod.rs", "pub fn api() {}\n"),
        ("src/only/tests.rs", "pub struct Engine;\n"),
        ("src/big/mod.rs", "pub fn big() {}\n"),
        (
            "src/big/tests/mod.rs",
            "pub struct Fixture;\npub fn helper_one() {}\npub fn helper_two() {}\n",
        ),
    ]);
    let opts = AnalyzeOptions {
        split_threshold_tokens: 8,
        ..AnalyzeOptions::default()
    };
    let analysis = analyze_project_with(&root, &opts).unwrap();
    let subs = crate::analysis::subsystems::build_deterministic(&analysis);
    let ids: Vec<&str> = subs.iter().map(|s| s.id.as_str()).collect();
    // No parent: project-level wording.
    let only = subs
        .iter()
        .find(|s| s.id.contains("only"))
        .unwrap_or_else(|| panic!("{ids:?}"));
    assert!(only.tags.iter().any(|t| t == "tests"), "{only:?}");
    assert_eq!(only.summary, "Тесты проекта");
    assert!(only.key_symbols.is_empty(), "{only:?}");
    // Split-off tests child: names its parent.
    let child = subs
        .iter()
        .find(|s| s.parent.is_some() && s.files.iter().all(|f| is_test_path(&f.path)))
        .unwrap_or_else(|| panic!("{ids:?}"));
    assert!(child.tags.iter().any(|t| t == "tests"), "{child:?}");
    assert!(child.summary.starts_with("Тесты для "), "{}", child.summary);
    let api = subs.iter().find(|s| s.id.contains("api")).unwrap();
    assert!(!api.tags.iter().any(|t| t == "tests"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn detects_deep_containers_astro_gradle_maven_and_avoids_doc_leaf_subsystems() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("settings.gradle.kts"),
        "rootProject.name = \"mimir\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("build.gradle.kts"),
        "plugins { kotlin(\"jvm\") }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("README.md"),
        "<div align=\"center\">\n<img src=\"logo.png\"/>\n</div>\n\nReal project description.\n",
    )
    .unwrap();

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
    std::fs::write(
        root.join("src/components/Card.astro"),
        "---\nconst x = 1;\n---\n<div>{x}</div>\n",
    )
    .unwrap();

    let meta = analyze_project(root, false, false).unwrap();
    assert_eq!(meta.name, "mimir");
    assert_eq!(meta.description, "Real project description.");
    let ids: Vec<&str> = meta
        .detected_modules
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    assert!(
        ids.iter().any(|id| id.starts_with("infra")),
        "missing infra in {ids:?}"
    );
    assert!(
        ids.iter().any(|id| id.starts_with("libs")),
        "missing libs in {ids:?}"
    );
    assert!(
        ids.iter().any(|id| id.contains("components")),
        "missing astro components in {ids:?}"
    );
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

    let opts = AnalyzeOptions {
        split_threshold_tokens: 15_000,
        ..Default::default()
    };
    let meta = analyze_project_with(root, &opts).unwrap();
    let ids: Vec<&str> = meta
        .detected_modules
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    assert!(
        !ids.contains(&"tests_cli"),
        "single test file must not become standalone subsystem: {ids:?}"
    );
    assert!(
        !ids.contains(&"tests_regression_benchmark"),
        "single test file must not become standalone subsystem: {ids:?}"
    );
    assert!(
        ids.contains(&"tests"),
        "root tests module must be kept: {ids:?}"
    );
    assert!(
        !ids.contains(&"tests_readme"),
        "README must never become standalone subsystem: {ids:?}"
    );
}

#[test]
fn test_truncate_to_boundary_sentence_priority() {
    let text = "Один открытый инструмент синхронизации Obsidian-хранилища на всех платформах (Linux, Windows, Android). Он ощущается как Google Drive Desktop: один раз указал папку vault и пароль, дальше всё синхронизируется само. Локально лежат обычные .md.";
    let res = crate::openviking::truncate_to_boundary(text, 220);
    assert_eq!(
        res,
        "Один открытый инструмент синхронизации Obsidian-хранилища на всех платформах (Linux, Windows, Android). Он ощущается как Google Drive Desktop: один раз указал папку vault и пароль, дальше всё синхронизируется само."
    );
}

#[test]
fn test_truncate_to_boundary_unbroken_url() {
    let url = "https://example.com/very/long/unbroken/path/without/spaces/that/exceeds/the/character/limit/completely/and/keeps/going/until/it/is/truncated/safely";
    let res = crate::openviking::truncate_to_boundary(url, 50);
    assert!(res.ends_with("..."));
    assert!(res.chars().count() <= 50);
}

#[test]
fn test_truncate_to_boundary_trailing_punctuation() {
    let text = "Первая часть текста: вторая часть текста, которая обрезается.";
    let res = crate::openviking::truncate_to_boundary(text, 25);
    assert!(!res.contains(":,"));
    assert!(res.ends_with("..."));
}

#[test]
fn test_is_file_entrypoint_gradle_maven() {
    assert!(super::is_file_entrypoint("build.gradle.kts"));
    assert!(super::is_file_entrypoint("build.gradle"));
    assert!(super::is_file_entrypoint("pom.xml"));
    assert!(super::is_file_entrypoint("CMakeLists.txt"));
}

#[test]
fn test_detect_root_entrypoints_gradle_maven() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
    std::fs::write(dir.path().join("settings.gradle.kts"), "").unwrap();
    std::fs::write(dir.path().join("pom.xml"), "").unwrap();
    let entrypoints = super::detect_root_entrypoints(dir.path());
    assert!(entrypoints.contains(&"build.gradle.kts".to_string()));
    assert!(entrypoints.contains(&"settings.gradle.kts".to_string()));
    assert!(entrypoints.contains(&"pom.xml".to_string()));
}

#[test]
fn test_parse_mise_tasks_multiline_and_depends() {
    let dir = tempfile::tempdir().unwrap();
    let toml = r#"
[tasks.build]
run = "./gradlew build"

[tasks.test]
description = "Run all tests"
run = "./gradlew allTests"

[tasks.desktop]
run = "./gradlew :app-desktop:run"

[tasks.check]
depends = ["build", "test"]
"#;
    std::fs::write(dir.path().join("mise.toml"), toml).unwrap();
    let tasks = super::mise::parse_mise_tasks(dir.path()).expect("mise tasks must be parsed");
    assert_eq!(tasks.build.as_deref(), Some("mise run build"));
    assert_eq!(tasks.test.as_deref(), Some("mise run test"));
    assert_eq!(
        tasks.custom.get("desktop").map(String::as_str),
        Some("mise run desktop")
    );
    assert_eq!(
        tasks.custom.get("check").map(String::as_str),
        Some("mise run check")
    );
}

#[test]
fn test_parse_mise_tasks_runner() {
    let dir = tempfile::tempdir().unwrap();
    let toml = r#"
[tasks.check]
run = "cargo check"

[tasks.clippy]
run = "cargo clippy --all-targets"

[tasks.format]
run = "cargo fmt --check"

[tasks.dev]
run = "cargo run --example dev"

[tasks.release]
run = "cargo build --release"
"#;
    std::fs::write(dir.path().join("mise.toml"), toml).unwrap();
    let tasks = super::mise::parse_mise_tasks(dir.path()).expect("mise tasks must be parsed");
    // build is None because no build task
    assert_eq!(tasks.build, None);
    // test falls back to check
    assert_eq!(tasks.test.as_deref(), Some("mise run check"));
    // lint falls back to clippy
    assert_eq!(tasks.lint.as_deref(), Some("mise run clippy"));
    // fmt falls back to format
    assert_eq!(tasks.fmt.as_deref(), Some("mise run format"));
    // run falls back to dev
    assert_eq!(tasks.run.as_deref(), Some("mise run dev"));
    // custom contains non-primary-slot tasks: check, clippy, format, dev, release
    assert_eq!(
        tasks.custom.get("check").map(String::as_str),
        Some("mise run check")
    );
    assert_eq!(
        tasks.custom.get("clippy").map(String::as_str),
        Some("mise run clippy")
    );
    assert_eq!(
        tasks.custom.get("format").map(String::as_str),
        Some("mise run format")
    );
    assert_eq!(
        tasks.custom.get("dev").map(String::as_str),
        Some("mise run dev")
    );
    assert_eq!(
        tasks.custom.get("release").map(String::as_str),
        Some("mise run release")
    );
}

#[test]
fn test_parse_mise_tasks_missing_or_no_tasks() {
    let dir = tempfile::tempdir().unwrap();
    // No mise.toml exists
    assert!(super::mise::parse_mise_tasks(dir.path()).is_none());

    // mise.toml without [tasks]
    std::fs::write(dir.path().join("mise.toml"), "[tools]\nnode = '20'\n").unwrap();
    assert!(super::mise::parse_mise_tasks(dir.path()).is_none());
}

#[test]
fn test_detect_commands_with_mise_merging_and_rtk() {
    let dir = tempfile::tempdir().unwrap();
    let toml = r#"
[tasks.build]
run = "./gradlew assemble"

[tasks.test]
run = "./gradlew allTests"

[tasks.extra]
run = "./gradlew extraTask"
"#;
    std::fs::write(dir.path().join("mise.toml"), toml).unwrap();

    // With rtk
    let cmds = super::detect_commands(dir.path(), &ProjectType::Rust, true);
    assert_eq!(cmds.build.as_deref(), Some("rtk mise run build"));
    assert_eq!(cmds.test.as_deref(), Some("rtk mise run test"));
    // Language default for Rust lint and fmt preserved, but prefixed with rtk
    assert_eq!(cmds.lint.as_deref(), Some("rtk cargo clippy"));
    assert_eq!(cmds.fmt.as_deref(), Some("rtk cargo fmt --check"));
    // Custom task extra present and prefixed with rtk
    assert_eq!(
        cmds.custom.get("extra").map(String::as_str),
        Some("rtk mise run extra")
    );
    // No fake "runner" command exists
    assert!(!cmds.custom.contains_key("runner"));

    // Without rtk
    let cmds_no_rtk = super::detect_commands(dir.path(), &ProjectType::Rust, false);
    assert_eq!(cmds_no_rtk.build.as_deref(), Some("mise run build"));
    assert_eq!(cmds_no_rtk.test.as_deref(), Some("mise run test"));
    assert_eq!(cmds_no_rtk.lint.as_deref(), Some("cargo clippy"));
    assert_eq!(cmds_no_rtk.fmt.as_deref(), Some("cargo fmt --check"));
    assert_eq!(
        cmds_no_rtk.custom.get("extra").map(String::as_str),
        Some("mise run extra")
    );
    assert!(!cmds_no_rtk.custom.contains_key("runner"));
}

#[test]
fn test_parse_gradle_workspace_groovy_and_nested() {
    let dir = tempfile::tempdir().unwrap();
    let kts = r#"
rootProject.name = "demo"
include(":core", ":drive")
include(":features:auth")
includeBuild("build-logic")
"#;
    std::fs::write(dir.path().join("settings.gradle.kts"), kts).unwrap();
    std::fs::create_dir_all(dir.path().join("core")).unwrap();
    std::fs::create_dir_all(dir.path().join("drive")).unwrap();
    std::fs::create_dir_all(dir.path().join("features/auth")).unwrap();
    std::fs::create_dir_all(dir.path().join("build-logic")).unwrap();

    let (name, members) = super::gradle::parse_gradle_workspace(dir.path());
    assert_eq!(name.as_deref(), Some("demo"));
    assert!(members.contains(&"core".to_string()));
    assert!(members.contains(&"drive".to_string()));
    assert!(members.contains(&"features/auth".to_string()));
    assert!(members.contains(&"build-logic".to_string()));
}

#[test]
fn test_parse_gradle_workspace_groovy_dsl() {
    let dir = tempfile::tempdir().unwrap();
    let groovy = r#"
// This is a comment
/* Multi-line
   comment */
rootProject.name = 'groovy-app'

include ':app', ':common'
include ':modules:auth'
includeBuild 'build-logic'

// include ':commented-out'
"#;
    std::fs::write(dir.path().join("settings.gradle"), groovy).unwrap();
    std::fs::create_dir_all(dir.path().join("app")).unwrap();
    std::fs::create_dir_all(dir.path().join("common")).unwrap();
    std::fs::create_dir_all(dir.path().join("modules/auth")).unwrap();
    std::fs::create_dir_all(dir.path().join("build-logic")).unwrap();
    // Non-existent directory should not be in members
    // e.g. if we had include ':missing' without creating dir

    let (name, members) = super::gradle::parse_gradle_workspace(dir.path());
    assert_eq!(name.as_deref(), Some("groovy-app"));
    assert_eq!(members.len(), 4);
    assert!(members.contains(&"app".to_string()));
    assert!(members.contains(&"common".to_string()));
    assert!(members.contains(&"modules/auth".to_string()));
    assert!(members.contains(&"build-logic".to_string()));
    assert!(!members.contains(&"commented-out".to_string()));
}

#[test]
fn test_parse_gradle_workspace_security_escaping() {
    let dir = tempfile::tempdir().unwrap();
    let kts = r#"
rootProject.name = "safe-app"
include(":safe-mod")
include(":../escaped")
includeBuild("../outside")
"#;
    std::fs::write(dir.path().join("settings.gradle.kts"), kts).unwrap();
    std::fs::create_dir_all(dir.path().join("safe-mod")).unwrap();

    let (name, members) = super::gradle::parse_gradle_workspace(dir.path());
    assert_eq!(name.as_deref(), Some("safe-app"));
    assert_eq!(members, vec!["safe-mod".to_string()]);
}

#[test]
fn test_gradle_subsystems_parent_overlap_guard() {
    let dir = tempfile::tempdir().unwrap();
    let kts = r#"
rootProject.name = "parent-guard-test"
include(":features:auth", ":features:billing")
"#;
    std::fs::write(dir.path().join("settings.gradle.kts"), kts).unwrap();
    std::fs::create_dir_all(dir.path().join("features/auth")).unwrap();
    std::fs::create_dir_all(dir.path().join("features/billing")).unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::create_dir_all(dir.path().join("scripts")).unwrap();

    // Put a code file in each to make them relevant
    std::fs::write(dir.path().join("features/auth/Auth.kt"), "class Auth").unwrap();
    std::fs::write(
        dir.path().join("features/billing/Billing.kt"),
        "class Billing",
    )
    .unwrap();
    std::fs::write(dir.path().join("docs/README.md"), "# Documentation").unwrap();
    std::fs::write(
        dir.path().join("scripts/deploy.sh"),
        "#!/bin/bash\necho deploy",
    )
    .unwrap();

    let (ptype, name, ws_members) = super::parse_gradle(dir.path()).unwrap();
    assert_eq!(ptype, ProjectType::Kotlin);
    assert_eq!(name.as_deref(), Some("parent-guard-test"));
    assert!(ws_members.contains(&"features/auth".to_string()));
    assert!(ws_members.contains(&"features/billing".to_string()));

    let modules = super::discover_subsystems(dir.path(), &ws_members, &ptype, false, false, 15000);

    let module_dirs: Vec<String> = modules.iter().map(|m| m.dir_path.clone()).collect();

    // The subproject modules must be present
    assert!(module_dirs.contains(&"features/auth".to_string()));
    assert!(module_dirs.contains(&"features/billing".to_string()));

    // Standalone infrastructure directories must be present
    assert!(module_dirs.contains(&"docs".to_string()));
    assert!(module_dirs.contains(&"scripts".to_string()));

    // Parent directory 'features' MUST NOT be duplicated as a separate subsystem
    assert!(
        !module_dirs.contains(&"features".to_string()),
        "Parent directory 'features' should not be present in modules: {:?}",
        module_dirs
    );
}

#[test]
fn test_prevent_test_file_atomization_flat_dir() {
    let dir = tempfile::tempdir().unwrap();
    // Создаем модуль с кодом и 25 тестами
    let src_dir = dir.path().join("core/src/commonMain/kotlin");
    let test_dir = dir.path().join("core/src/commonTest/kotlin");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(src_dir.join("Main.kt"), "class Main").unwrap();
    for i in 0..25 {
        let content = format!(
            "class Test{} {{\n    fun test() {{ assert(true) }}\n}}\n",
            i
        );
        // Заполняем строками > 300 токенов (всего > 15 000 токенов для вызова сплиттера)
        let padded = format!("{}\n{}", content, "// comment\n".repeat(300));
        std::fs::write(test_dir.join(format!("MyTest{}.kt", i)), padded).unwrap();
    }
    let analysis = super::analyze_project(dir.path(), false, false).unwrap();
    let modules = &analysis.detected_modules;
    // Тесты не должны распасться на 25 независимых подсистем
    let single_file_test_modules: Vec<_> = modules
        .iter()
        .filter(|m| m.files.len() == 1 && super::tests_classifier::is_test_path(&m.files[0].path))
        .collect();
    assert_eq!(
        single_file_test_modules.len(),
        0,
        "No single test file may become an L1 subsystem"
    );
    let test_modules: Vec<_> = modules.iter().filter(|m| m.id.contains("tests")).collect();
    assert_eq!(
        test_modules.len(),
        1,
        "Expected exactly 1 aggregated test module, got {:?}",
        modules.iter().map(|m| &m.id).collect::<Vec<_>>()
    );
    assert_eq!(test_modules[0].id, "core_tests_common");
    assert!(
        test_modules[0].files.len() <= 15,
        "Representative files must be capped at 15"
    );
}

#[test]
fn test_extract_kotlin_symbols_multiline_no_braces() {
    let source = r#"
package dev.mimir.core

sealed interface SyncStatus

class RemoteUnavailableException(message: String) : Exception(message)

internal fun doInternalSync(force: Boolean = false): Boolean

private fun ignoredHelper() {
    println("secret")
}

data class Account(val id: String, val name: String)
"#;
    let adapter =
        crate::analysis::lang::adapter_for_ext("kt").expect("Kotlin adapter must be registered");
    let symbols = adapter.extract_symbols(source);
    assert!(symbols
        .iter()
        .any(|s| s.contains("sealed interface SyncStatus")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("class RemoteUnavailableException")));
    assert!(symbols
        .iter()
        .any(|s| s.contains("internal fun doInternalSync")));
    assert!(symbols.iter().any(|s| s.contains("data class Account")));
    assert!(
        !symbols.iter().any(|s| s.contains("ignoredHelper")),
        "private functions must be ignored"
    );
}

#[test]
fn test_discover_subsystems_disambiguates_colliding_paths() {
    let dir = tempfile::tempdir().unwrap();
    // a_b/c and a/b_c both sanitize to a_b_c without disambiguation
    fs::create_dir_all(dir.path().join("a_b/c/src")).unwrap();
    fs::write(dir.path().join("a_b/c/src/lib.rs"), "pub fn a() {}\n").unwrap();
    fs::create_dir_all(dir.path().join("a/b_c/src")).unwrap();
    fs::write(dir.path().join("a/b_c/src/lib.rs"), "pub fn b() {}\n").unwrap();

    let ws_members = vec!["a/b_c".to_string(), "a_b/c".to_string()];
    let modules = super::discover_subsystems(
        dir.path(),
        &ws_members,
        &ProjectType::Generic,
        false,
        false,
        15000,
    );
    assert_eq!(modules.len(), 2, "Expected 2 modules, got: {:?}", modules);
    let ids: Vec<String> = modules.iter().map(|m| m.id.clone()).collect();
    assert_ne!(ids[0], ids[1], "IDs must differ, got: {:?}", ids);
    let mut unique_ids = ids.clone();
    unique_ids.sort();
    unique_ids.dedup();
    assert_eq!(
        ids.len(),
        unique_ids.len(),
        "Module IDs must be unique: {:?}",
        ids
    );
}

#[test]
fn test_flat_scripts_collection_never_splits_into_single_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("scripts/ci")).unwrap();
    // 20 flat script files of ~1200 tokens each (total > 24000)
    for i in 1..=20 {
        let content = format!(
            "#!/bin/bash\n# Script {i}\n{}\n",
            "echo 'doing work step'\n".repeat(200)
        );
        fs::write(root.join(format!("scripts/job_{i:02}.sh")), content).unwrap();
    }
    // Subdirectory scripts/ci with ~8000 tokens
    let ci_content = format!(
        "#!/bin/bash\n# CI task\n{}\n",
        "echo 'ci step'\n".repeat(1200)
    );
    fs::write(root.join("scripts/ci/build.sh"), ci_content).unwrap();

    let modules = super::discover_subsystems(root, &[], &ProjectType::Generic, false, false, 15000);
    let ids: Vec<String> = modules.iter().map(|m| m.id.clone()).collect();
    assert!(
        ids.contains(&"scripts".to_string()),
        "Must have parent 'scripts' module: {:?}",
        ids
    );
    assert!(
        ids.contains(&"scripts_ci".to_string()),
        "Must have child 'scripts_ci' module: {:?}",
        ids
    );
    // Flat files must NOT be promoted to individual subsystems
    assert_eq!(
        modules.len(),
        2,
        "Expected exactly 2 modules (scripts, scripts_ci), got: {:?}",
        ids
    );
}

#[test]
fn test_root_tests_dir_does_not_promote_single_test_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("tests")).unwrap();
    for i in 1..=5 {
        let content = format!(
            "// Test suite {i}\n#[test]\nfn test_{i}() {{\n{}\n}}\n",
            "    assert!(true);\n".repeat(600)
        );
        fs::write(root.join(format!("tests/e2e_{i}.rs")), content).unwrap();
    }
    let modules = super::discover_subsystems(root, &[], &ProjectType::Generic, false, false, 15000);
    let ids: Vec<String> = modules.iter().map(|m| m.id.clone()).collect();
    assert_eq!(
        modules.len(),
        1,
        "Expected single 'tests' module without single-file promotions, got: {:?}",
        ids
    );
    assert_eq!(ids[0], "tests");
}

#[test]
fn test_container_keeps_direct_files_without_duplicating_child_modules() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("packages/ui/src")).unwrap();
    fs::write(
        root.join("packages/tsconfig.base.json"),
        "{\n  \"compilerOptions\": {}\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("packages/ui/src/index.ts"),
        "export const a = 1;\n",
    )
    .unwrap();

    let modules = super::discover_subsystems(root, &[], &ProjectType::Generic, false, false, 15000);
    let ids: Vec<String> = modules.iter().map(|m| m.id.clone()).collect();
    assert!(
        ids.contains(&"packages".to_string()),
        "Container 'packages' must be kept for direct files: {:?}",
        ids
    );
    assert!(
        ids.contains(&"packages_ui".to_string()),
        "Child module 'packages_ui' must be present: {:?}",
        ids
    );

    let pkg_mod = modules.iter().find(|m| m.id == "packages").unwrap();
    let pkg_files: Vec<String> = pkg_mod.files.iter().map(|f| f.path.clone()).collect();
    assert!(pkg_files.contains(&"packages/tsconfig.base.json".to_string()));
    assert!(
        !pkg_files.iter().any(|f| f.contains("packages/ui/src")),
        "Container must not duplicate child files: {:?}",
        pkg_files
    );
}
