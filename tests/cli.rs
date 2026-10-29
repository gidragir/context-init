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
fn cli_init_writes_repo_storage_by_default() {
    let sb = Sandbox::new("init");
    sb.write_fixture();
    let out = sb.run(&["init", ".", "--compact"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(sb
        .project()
        .join(".context-init/knowledge/L0_index.json")
        .is_file());
    assert!(central_indexes(&sb.data_home()).is_empty());
    assert!(!sb.project().join(".agents").exists());

    let full = sb.run(&["init", ".", "--force"]);
    assert!(full.status.success(), "{}{}", stdout(&full), stderr(&full));
    let all = format!("{}{}", stdout(&full), stderr(&full));
    assert!(all.contains("linguist-generated"), "{all}");
}

#[test]
fn cli_init_central_mode_writes_central() {
    let sb = Sandbox::new("init_central");
    sb.write_fixture();
    fs::write(
        sb.project().join(".context-init.toml"),
        "storage = \"central\"\n",
    )
    .unwrap();
    let out = sb.run(&["init", ".", "--compact"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert_eq!(central_indexes(&sb.data_home()).len(), 1);
    assert!(!sb.project().join(".context-init/knowledge").exists());
}

#[test]
fn cli_check_hints_storage_mismatch() {
    let sb = Sandbox::new("mismatch");
    sb.write_fixture();
    let toml = sb.project().join(".context-init.toml");
    fs::write(&toml, "storage = \"central\"\n").unwrap();
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    fs::remove_file(&toml).unwrap();
    let out = sb.run(&["check", "."]);
    let all = format!("{}{}", stdout(&out), stderr(&out));
    assert!(all.contains("migrate --to central"), "{all}");
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap().flatten() {
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

#[test]
fn cli_clone_at_other_path_works_without_init() {
    let sb = Sandbox::new("clone");
    sb.write_fixture();
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    let index = ci_core::storage::L0Index::load_from_file(
        &sb.project().join(".context-init/knowledge/L0_index.json"),
    )
    .unwrap();
    let id = index.subsystems[0].id.clone();

    let clone = sb.dir.join("clone");
    copy_dir(&sb.project(), &clone);
    let run_in_clone = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_context-init"))
            .args(args)
            .current_dir(&clone)
            .env("HOME", sb.home())
            .env("XDG_CONFIG_HOME", sb.home().join(".config"))
            .env("XDG_DATA_HOME", sb.data_home())
            .env("NO_COLOR", "1")
            .output()
            .unwrap()
    };
    let pack = run_in_clone(&["pack", &id, ".", "--compress"]);
    assert!(pack.status.success(), "{}{}", stdout(&pack), stderr(&pack));
    let check = run_in_clone(&["check", ".", "--exit-code"]);
    assert!(
        check.status.success(),
        "{}{}",
        stdout(&check),
        stderr(&check)
    );
    let all = format!("{}{}", stdout(&check), stderr(&check));
    assert!(!all.contains("init"), "{all}");
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
    let index_path = sb.project().join(".context-init/knowledge/L0_index.json");
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

fn exe_path() -> PathBuf {
    fs::canonicalize(env!("CARGO_BIN_EXE_context-init")).unwrap()
}

#[test]
fn cli_setup_dry_run_prints_diff_and_writes_nothing() {
    let sb = Sandbox::new("setup_dry");
    let out = sb.run(&["setup", "claude-code"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains(".claude.json") && text.contains("+"),
        "{text}"
    );
    assert!(text.contains("--write"), "{text}");
    assert!(!sb.home().join(".claude.json").exists());
}

#[test]
fn cli_setup_write_claude_code() {
    let sb = Sandbox::new("setup_cc");
    fs::write(sb.home().join(".claude.json"), "{\"numStartups\": 3}\n").unwrap();
    let out = sb.run(&["setup", "claude-code", "--write"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    let cfg: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sb.home().join(".claude.json")).unwrap()).unwrap();
    assert_eq!(cfg["numStartups"], 3);
    let cmd = cfg["mcpServers"]["context-init"]["command"]
        .as_str()
        .unwrap();
    assert_eq!(fs::canonicalize(cmd).unwrap(), exe_path());
    assert!(sb
        .home()
        .join(".claude/skills/context-init/SKILL.md")
        .is_file());
}

#[test]
fn cli_setup_all_skips_absent_harnesses() {
    let sb = Sandbox::new("setup_all");
    fs::create_dir_all(sb.home().join(".cursor")).unwrap();
    let out = sb.run(&["setup", "all", "--write"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(sb.home().join(".cursor/mcp.json").is_file());
    assert!(!sb.home().join(".codex").exists());
    assert!(!sb.home().join(".gemini").exists());
    // `$HOME` always exists; claude-code counts as installed only with `~/.claude`.
    assert!(!sb.home().join(".claude.json").exists());
    assert!(stdout(&out).contains("User Rules"), "{}", stdout(&out));
}

#[test]
fn cli_setup_all_detects_claude_code_by_its_dir() {
    let sb = Sandbox::new("setup_all_claude");
    fs::create_dir_all(sb.home().join(".claude")).unwrap();
    let out = sb.run(&["setup", "all", "--write"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(sb.home().join(".claude.json").is_file());
}

#[test]
fn cli_setup_write_twice_no_second_backup() {
    let sb = Sandbox::new("setup_twice");
    fs::create_dir_all(sb.home().join(".codex")).unwrap();
    fs::write(
        sb.home().join(".codex/config.toml"),
        "# mine\nmodel = \"o3\"\n",
    )
    .unwrap();
    assert!(sb.run(&["setup", "codex", "--write"]).status.success());
    let backups = |sb: &Sandbox| {
        fs::read_dir(sb.home().join(".codex"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".bak."))
            .count()
    };
    assert_eq!(backups(&sb), 1);
    let again = sb.run(&["setup", "codex", "--write"]);
    assert!(again.status.success());
    assert_eq!(backups(&sb), 1, "{}", stdout(&again));
    let agents = fs::read_to_string(sb.home().join(".codex/AGENTS.md")).unwrap();
    assert_eq!(agents.matches("<!-- context-init:begin -->").count(), 1);
    assert!(fs::read_to_string(sb.home().join(".codex/config.toml"))
        .unwrap()
        .contains("# mine"));
}

impl Sandbox {
    fn run_with_path(&self, args: &[&str], path: &str) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_context-init"));
        cmd.args(args)
            .current_dir(self.project())
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("XDG_DATA_HOME", self.data_home())
            .env("NO_COLOR", "1")
            .env("PATH", path);
        for (k, _) in std::env::vars() {
            if k.starts_with("CONTEXT_INIT_") {
                cmd.env_remove(k);
            }
        }
        cmd.output().expect("run context-init")
    }

    fn enable_telemetry(&self) {
        let dir = self.home().join(".config/context-init");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), "[telemetry]\nenabled = true\n").unwrap();
    }

    fn seed_events(&self) {
        use ci_core::telemetry::{
            FeedbackSignal, JsonlTelemetrySink, TelemetryEvent, TelemetrySignal, TelemetrySink,
        };
        let sink = JsonlTelemetrySink::new(self.data_home(), true);
        for verdict in ["helpful", "stale"] {
            let signal = TelemetrySignal::Feedback(FeedbackSignal {
                tool: "context_pack".to_string(),
                verdict: verdict.to_string(),
                note: None,
            });
            sink.record(&TelemetryEvent::new("demo-project", signal))
                .unwrap();
        }
    }

    /// A directory holding a fake `gh` that logs its arguments.
    fn fake_gh_dir(&self) -> PathBuf {
        let bin = self.dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let log = self.dir.join("gh.log");
        let script = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display());
        let gh = bin.join("gh");
        fs::write(&gh, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
        }
        bin
    }
}

#[test]
fn cli_telemetry_disabled_prints_hint() {
    let sb = Sandbox::new("tele_off");
    let out = sb.run(&["telemetry", "report"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("[telemetry]") && stdout(&out).contains("enabled = true"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn cli_telemetry_report_renders_front_matter() {
    let sb = Sandbox::new("tele_report");
    sb.enable_telemetry();
    sb.seed_events();
    let out = sb.run(&["telemetry", "report"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).starts_with("---"), "{}", stdout(&out));
}

#[cfg(unix)]
#[test]
fn cli_telemetry_submit_uses_gh_when_available() {
    let sb = Sandbox::new("tele_gh");
    sb.enable_telemetry();
    sb.seed_events();
    let bin = sb.fake_gh_dir();
    let out = sb.run_with_path(
        &["telemetry", "report", "--submit", "--yes"],
        &bin.to_string_lossy(),
    );
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    let log = fs::read_to_string(sb.dir.join("gh.log")).unwrap();
    assert!(log.contains("issue create"), "{log}");
    assert!(log.contains("--label telemetry"), "{log}");
    assert!(log.contains("gidragir/context-init"), "{log}");
}

#[test]
fn cli_telemetry_submit_without_gh_prints_url() {
    let sb = Sandbox::new("tele_url");
    sb.enable_telemetry();
    sb.seed_events();
    let empty = sb.dir.join("empty-bin");
    fs::create_dir_all(&empty).unwrap();
    let out = sb.run_with_path(
        &["telemetry", "report", "--submit", "--yes"],
        &empty.to_string_lossy(),
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out)
            .contains("https://github.com/gidragir/context-init/issues/new?labels=telemetry"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn cli_telemetry_submit_non_tty_without_yes_aborts() {
    let sb = Sandbox::new("tele_notty");
    sb.enable_telemetry();
    sb.seed_events();
    let out = sb.run(&["telemetry", "report", "--submit"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("--yes"), "{}", stderr(&out));
}

#[test]
fn cli_feedback_records_when_enabled() {
    let sb = Sandbox::new("fb_on");
    sb.enable_telemetry();
    let out = sb.run(&["feedback", "context_pack", "helpful", "--note", "great"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("recorded"), "{}", stdout(&out));
    let dir = sb.data_home().join("context-init/telemetry");
    let jsonl: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(jsonl.len(), 1);
}

#[test]
fn cli_feedback_disabled_reports_not_recorded() {
    let sb = Sandbox::new("fb_off");
    let out = sb.run(&["feedback", "context_pack", "stale"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("not recorded"), "{}", stdout(&out));
    assert!(!sb.data_home().join("context-init/telemetry").exists());
}

#[test]
fn cli_update_twice_leaves_git_clean() {
    let sb = Sandbox::new("git_clean");
    sb.write_fixture();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(sb.project())
            .env("HOME", sb.home())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", stderr(&out));
        stdout(&out)
    };
    git(&["init", "-q"]);
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "kb"]);
    // The first update after the first commit stamps cards with real Git SHAs (they were
    // created before any commit existed); from then on updates are no-ops.
    assert!(sb.run(&["update", ".", "--compact"]).status.success());
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "kb stamped"]);
    let out = sb.run(&["update", ".", "--compact"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert_eq!(git(&["status", "--porcelain"]), "");
}

fn kb_index(sb: &Sandbox) -> PathBuf {
    sb.project().join(".context-init/knowledge/L0_index.json")
}

#[test]
fn cli_migrate_round_trip() {
    let sb = Sandbox::new("mig_round");
    sb.write_fixture();
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    let index = ci_core::storage::L0Index::load_from_file(&kb_index(&sb)).unwrap();
    let id = index.subsystems[0].id.clone();

    let out = sb.run(&["migrate", ".", "--to", "central", "--remove-source"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert_eq!(central_indexes(&sb.data_home()).len(), 1);
    assert!(!kb_index(&sb).exists());
    let toml = fs::read_to_string(sb.project().join(".context-init.toml")).unwrap();
    assert!(toml.contains("storage = \"central\""), "{toml}");
    let pack = sb.run(&["pack", &id, ".", "--compress"]);
    assert!(pack.status.success(), "{}{}", stdout(&pack), stderr(&pack));

    let back = sb.run(&["migrate", ".", "--to", "repo", "--remove-source"]);
    assert!(back.status.success(), "{}{}", stdout(&back), stderr(&back));
    assert!(kb_index(&sb).is_file());
    assert!(central_indexes(&sb.data_home()).is_empty());
    let toml = fs::read_to_string(sb.project().join(".context-init.toml")).unwrap();
    assert!(toml.contains("storage = \"repo\""), "{toml}");
}

#[test]
fn cli_migrate_keeps_toml_comments() {
    let sb = Sandbox::new("mig_toml");
    sb.write_fixture();
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    fs::write(
        sb.project().join(".context-init.toml"),
        "# keep me\nlanguage = \"en\"\n",
    )
    .unwrap();
    let out = sb.run(&["migrate", ".", "--to", "central"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    let toml = fs::read_to_string(sb.project().join(".context-init.toml")).unwrap();
    assert!(toml.contains("# keep me"), "{toml}");
    assert!(toml.contains("language = \"en\""), "{toml}");
    assert!(toml.contains("storage = \"central\""), "{toml}");
}

#[test]
fn cli_migrate_refuses_existing_target_without_force() {
    let sb = Sandbox::new("mig_force");
    sb.write_fixture();
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    assert!(sb
        .run(&["migrate", ".", "--to", "central"])
        .status
        .success());
    let toml = sb.project().join(".context-init.toml");
    fs::write(&toml, "storage = \"repo\"\n").unwrap();
    let again = sb.run(&["migrate", ".", "--to", "central"]);
    assert!(!again.status.success());
    assert!(stderr(&again).contains("--force"), "{}", stderr(&again));
    assert!(sb
        .run(&["migrate", ".", "--to", "central", "--force"])
        .status
        .success());
}

#[test]
fn cli_migrate_noop_when_already_in_mode() {
    let sb = Sandbox::new("mig_noop");
    sb.write_fixture();
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    let out = sb.run(&["migrate", ".", "--to", "repo"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(
        stdout(&out).contains("already using storage = \"repo\""),
        "{}",
        stdout(&out)
    );
}

#[test]
fn cli_migrate_dry_run_writes_nothing() {
    let sb = Sandbox::new("mig_dry");
    sb.write_fixture();
    assert!(sb.run(&["init", ".", "--compact"]).status.success());
    let out = sb.run(&["migrate", ".", "--to", "central", "--dry-run"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(stdout(&out).contains("L0_index.json"), "{}", stdout(&out));
    assert!(central_indexes(&sb.data_home()).is_empty());
    assert!(!sb.project().join(".context-init.toml").exists());
}

#[test]
fn cli_init_rejects_unknown_model_alias() {
    let sb = Sandbox::new("bad_model");
    sb.write_fixture();
    let out = sb.run(&["init", ".", "--model", "nosuch"]);
    assert!(!out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(
        stderr(&out).contains("Unknown model alias 'nosuch'"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn cli_agents_md_contains_rtk_fallback_commands() {
    let sb = Sandbox::new("agents_rtk");
    sb.write_fixture();
    let out = sb.run(&["init", ".", "--compact"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    let agents = fs::read_to_string(sb.project().join("AGENTS.md")).unwrap();
    assert!(
        agents.contains("rtk context-init pack <subsystem_id> --compress"),
        "{agents}"
    );
    assert!(
        agents.contains("rtk context-init check --compact"),
        "{agents}"
    );
}

#[test]
fn cli_setup_all_write_aborts_before_any_write_if_one_harness_config_is_invalid() {
    let sb = Sandbox::new("setup_all_abort");
    let good = sb.home().join(".gemini/config/mcp_config.json");
    let bad = sb.home().join(".cursor/mcp.json");
    fs::create_dir_all(good.parent().unwrap()).unwrap();
    fs::create_dir_all(bad.parent().unwrap()).unwrap();
    fs::write(&good, "{}\n").unwrap();
    fs::write(&bad, "{bad").unwrap();
    let out = sb.run(&["setup", "all", "--write"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    assert_eq!(fs::read_to_string(&good).unwrap(), "{}\n");
    let siblings: Vec<_> = fs::read_dir(good.parent().unwrap())
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(siblings.len(), 1, "unexpected files: {siblings:?}");
}

#[test]
fn workspace_task_runner_and_packaging_metadata_invariants() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mise: toml::Value = fs::read_to_string(root.join("mise.toml"))
        .expect("mise.toml must exist")
        .parse()
        .expect("mise.toml is valid TOML");
    for task in ["build", "test", "lint", "fmt", "check", "dist", "audit"] {
        assert!(
            mise["tasks"].get(task).is_some(),
            "missing mise task {task}"
        );
    }
    let lefthook = fs::read_to_string(root.join("lefthook.yml")).unwrap();
    assert!(!lefthook.contains("sync-binary"), "{lefthook}");

    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("[package.metadata.binstall]"));
    assert!(manifest.contains("license = \"MIT\""));
    for krate in ["ci-core", "ci-llm", "ci-mcp"] {
        let m = fs::read_to_string(root.join("crates").join(krate).join("Cargo.toml")).unwrap();
        assert!(m.contains("license.workspace = true"), "{krate}");
        assert!(m.contains("repository.workspace = true"), "{krate}");
    }
}

#[test]
fn ci_and_release_configuration_files_exist_and_cover_d26_d27() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("ci.yml");
    for needle in [
        "ubuntu-latest",
        "macos-latest",
        "windows-latest",
        "Swatinem/rust-cache",
        "clippy",
        "nextest",
        "regression_benchmark",
    ] {
        assert!(ci.contains(needle), "ci.yml lacks {needle}");
    }
    let dist: toml::Value = fs::read_to_string(root.join("dist-workspace.toml"))
        .expect("dist-workspace.toml")
        .parse()
        .unwrap();
    let text = dist.to_string();
    for needle in [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "gidragir/homebrew-tap",
    ] {
        assert!(text.contains(needle), "dist-workspace.toml lacks {needle}");
    }
    fs::read_to_string(root.join("release-plz.toml"))
        .expect("release-plz.toml")
        .parse::<toml::Value>()
        .unwrap();
    for wf in ["release-plz.yml", "release.yml"] {
        assert!(root.join(".github/workflows").join(wf).is_file(), "{wf}");
    }
}
