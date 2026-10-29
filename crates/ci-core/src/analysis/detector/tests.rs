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
    fs::write(src.join("storage.rs"), rust_source("storage", 900)).unwrap();
    fs::write(src.join("analysis").join("mod.rs"), "pub mod inner;\n").unwrap();
    fs::write(src.join("analysis").join("a.rs"), rust_source("a", 600)).unwrap();
    fs::write(src.join("analysis").join("b.rs"), rust_source("b", 600)).unwrap();
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
    let threshold = 1000;
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
    let outside = base.join("outside");
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
    std::os::unix::fs::symlink(outside.join("leak"), root.join("linked")).unwrap();

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
