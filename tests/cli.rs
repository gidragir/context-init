//! End-to-end tests of the `context-init` binary with an isolated HOME/XDG environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Temporary sandbox holding a fake HOME (with XDG dirs) and a project directory.
struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("ci_cli_{tag}_{}_{nanos}_{seq}", std::process::id()));
        fs::create_dir_all(dir.join("home")).unwrap();
        fs::create_dir_all(dir.join("project")).unwrap();
        Self { dir }
    }

    fn home(&self) -> PathBuf {
        self.dir.join("home")
    }

    fn project(&self) -> PathBuf {
        self.dir.join("project")
    }

    fn data_home(&self) -> PathBuf {
        self.home().join(".local/share")
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_context-init"));
        cmd.args(args)
            .current_dir(self.project())
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("XDG_DATA_HOME", self.data_home())
            .env("NO_COLOR", "1");
        for (k, _) in std::env::vars() {
            if k.starts_with("CONTEXT_INIT_") {
                cmd.env_remove(k);
            }
        }
        cmd.output().expect("run context-init")
    }

    /// Writes a small Rust project with three modules.
    fn write_fixture(&self) {
        let root = self.project();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/main.rs"),
            "mod api;\nmod engine;\nmod store;\nfn main() {}\n",
        )
        .unwrap();
        for m in ["api", "engine", "store"] {
            fs::create_dir_all(root.join("src").join(m)).unwrap();
            fs::write(
                root.join("src").join(m).join("mod.rs"),
                format!("pub struct Thing;\npub fn run_{m}() -> u32 {{\n    1\n}}\n"),
            )
            .unwrap();
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn central_indexes(data_home: &Path) -> Vec<PathBuf> {
    let projects = data_home.join("context-init/projects");
    fs::read_dir(&projects)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path().join("L0_index.json"))
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn cli_version_matches_cargo() {
    let sb = Sandbox::new("version");
    let out = sb.run(&["--version"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn cli_init_writes_central_storage() {
    let sb = Sandbox::new("init");
    sb.write_fixture();
    let out = sb.run(&["init", ".", "--compact"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert_eq!(
        central_indexes(&sb.data_home()).len(),
        1,
        "{}",
        stdout(&out)
    );
    assert!(!sb.project().join(".agents/knowledge").exists());
}

#[test]
fn cli_check_reads_legacy_in_repo_knowledge() {
    let sb = Sandbox::new("legacy");
    sb.write_fixture();
    let analysis =
        ci_core::analysis::detector::analyze_project(&sb.project(), false, false).unwrap();
    let subs = ci_core::analysis::subsystems::build_deterministic(&analysis);
    ci_core::analysis::generator::generate(
        &analysis,
        &subs,
        &sb.project().join(".agents/knowledge"),
        &ci_core::analysis::generator::GenerateOptions::default(),
    )
    .unwrap();

    let out = sb.run(&["check", "."]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    let all = format!("{}{}", stdout(&out), stderr(&out));
    assert!(all.contains("migrate"), "{all}");
}

#[test]
fn cli_removed_ai_flag_errors() {
    let sb = Sandbox::new("ai_flag");
    sb.write_fixture();
    let out = sb.run(&["init", ".", "--ai"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("--ai"), "{}", stderr(&out));
}

#[test]
fn cli_role_flag_parses() {
    let sb = Sandbox::new("role");
    sb.write_fixture();
    let ok = sb.run(&["--role", "init=none", "init", ".", "--compact"]);
    assert!(ok.status.success(), "{}{}", stdout(&ok), stderr(&ok));
    let bad = sb.run(&["--role", "bogus=x", "init", ".", "--compact", "--force"]);
    assert!(!bad.status.success());
    assert!(
        stderr(&bad).to_lowercase().contains("role"),
        "{}",
        stderr(&bad)
    );
}

#[test]
fn cli_pack_compress_outputs_xml() {
    let sb = Sandbox::new("pack");
    sb.write_fixture();
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    let index_path = central_indexes(&sb.data_home()).remove(0);
    let index = ci_core::storage::L0Index::load_from_file(&index_path).unwrap();
    let id = index.subsystems[0].id.clone();
    let out = sb.run(&["pack", &id, ".", "--compress"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(
        stdout(&out).trim_start().starts_with('<'),
        "{}",
        stdout(&out)
    );
}

#[test]
fn cli_check_exit_code_tracks_drift() {
    let sb = Sandbox::new("drift");
    sb.write_fixture();
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    assert!(sb
        .run(&["check", ".", "--exit-code", "--compact"])
        .status
        .success());

    let extra = sb.project().join("src/extra");
    fs::create_dir_all(&extra).unwrap();
    fs::write(extra.join("mod.rs"), "pub fn extra() {}\n").unwrap();
    let drift = sb.run(&["check", ".", "--exit-code", "--compact"]);
    assert_eq!(drift.status.code(), Some(1), "{}", stdout(&drift));
    assert!(stdout(&drift).contains("extra"), "{}", stdout(&drift));

    fs::remove_dir_all(&extra).unwrap();
    assert!(sb
        .run(&["check", ".", "--exit-code", "--compact"])
        .status
        .success());
}

#[test]
fn cli_config_validate_exit_codes() {
    let sb = Sandbox::new("cfg_validate");
    let ok = sb.run(&["config", "validate"]);
    assert!(ok.status.success(), "{}{}", stdout(&ok), stderr(&ok));

    fs::write(
        sb.project().join(".context-init.toml"),
        "max_response_tokens = 10\n",
    )
    .unwrap();
    let bad = sb.run(&["config", "validate"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(stdout(&bad).contains("64"), "{}", stdout(&bad));
}

#[test]
fn cli_config_init_refuses_overwrite_without_force() {
    let sb = Sandbox::new("cfg_init");
    assert!(sb.run(&["config", "init"]).status.success());
    let global = sb.home().join(".config/context-init/config.toml");
    assert!(global.is_file());
    let again = sb.run(&["config", "init"]);
    assert!(!again.status.success());
    assert!(stderr(&again).contains("--force"), "{}", stderr(&again));
    assert!(sb.run(&["config", "init", "--force"]).status.success());

    assert!(sb.run(&["config", "init", "--project"]).status.success());
    assert!(sb.project().join(".context-init.toml").is_file());
}

#[test]
fn cli_config_show_lists_sources() {
    let sb = Sandbox::new("cfg_show");
    fs::write(
        sb.project().join(".context-init.toml"),
        "max_response_tokens = 3000\n",
    )
    .unwrap();
    let out = sb.run(&["config", "show"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains(".context-init.toml"), "{text}");
    assert!(text.contains("max_response_tokens = 3000"), "{text}");

    let json = sb.run(&["config", "show", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(v["config"]["max_response_tokens"], 3000);
    assert_eq!(v["sources"].as_array().unwrap().len(), 1);
}

#[test]
fn cli_config_path_lists_storage() {
    let sb = Sandbox::new("cfg_path");
    let out = sb.run(&["config", "path"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("project_storage_dir:"),
        "{}",
        stdout(&out)
    );
}

/// Seeds a legacy in-repo knowledge base for the fixture project.
fn seed_legacy(sb: &Sandbox) {
    let analysis =
        ci_core::analysis::detector::analyze_project(&sb.project(), false, false).unwrap();
    let subs = ci_core::analysis::subsystems::build_deterministic(&analysis);
    ci_core::analysis::generator::generate(
        &analysis,
        &subs,
        &sb.project().join(".agents/knowledge"),
        &ci_core::analysis::generator::GenerateOptions::default(),
    )
    .unwrap();
}

#[test]
fn cli_migrate_copies_and_keeps_source() {
    let sb = Sandbox::new("mig_copy");
    sb.write_fixture();
    seed_legacy(&sb);
    let out = sb.run(&["migrate", "."]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert_eq!(central_indexes(&sb.data_home()).len(), 1);
    assert!(sb
        .project()
        .join(".agents/knowledge/L0_index.json")
        .is_file());
}

#[test]
fn cli_migrate_dry_run_writes_nothing() {
    let sb = Sandbox::new("mig_dry");
    sb.write_fixture();
    seed_legacy(&sb);
    let out = sb.run(&["migrate", ".", "--dry-run"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("L0_index.json"), "{}", stdout(&out));
    assert!(central_indexes(&sb.data_home()).is_empty());
}

#[test]
fn cli_migrate_remove_source_deletes_after_copy() {
    let sb = Sandbox::new("mig_rm");
    sb.write_fixture();
    seed_legacy(&sb);
    let out = sb.run(&["migrate", ".", "--remove-source"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(central_indexes(&sb.data_home()).len(), 1);
    assert!(!sb.project().join(".agents/knowledge").exists());
}

#[test]
fn cli_migrate_twice_requires_force() {
    let sb = Sandbox::new("mig_twice");
    sb.write_fixture();
    seed_legacy(&sb);
    assert!(sb.run(&["migrate", "."]).status.success());
    let again = sb.run(&["migrate", "."]);
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("--force"), "{}", stderr(&again));
    assert!(sb.run(&["migrate", ".", "--force"]).status.success());
}
