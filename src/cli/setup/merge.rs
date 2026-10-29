//! Non-destructive merging of the `context-init` MCP entry into harness config files.

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// MCP server name used in every harness config.
pub const SERVER_NAME: &str = "context-init";

/// Merges `entry` into `mcpServers.context-init` of a JSON config, keeping every other key,
/// key order and any extra keys of an existing entry (e.g. `env`).
pub fn merge_json(existing: Option<&str>, entry: Value) -> Result<String> {
    let mut root = match existing.map(str::trim) {
        None | Some("") => Value::Object(Map::new()),
        Some(text) => serde_json::from_str(text).context("existing config is not valid JSON")?,
    };
    let root_obj = root
        .as_object_mut()
        .context("existing config is not a JSON object")?;
    let servers = root_obj
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()));
    let servers = match servers.as_object_mut() {
        Some(s) => s,
        None => bail!("`mcpServers` in the existing config is not an object"),
    };
    let slot = servers
        .entry(SERVER_NAME)
        .or_insert_with(|| Value::Object(Map::new()));
    match (slot.as_object_mut(), entry) {
        (Some(slot_obj), Value::Object(new)) => {
            for (k, v) in new {
                slot_obj.insert(k, v);
            }
        }
        (_, new) => *slot = new,
    }
    Ok(format!("{}\n", serde_json::to_string_pretty(&root)?))
}

/// Turns an inline table (`key = { .. }`) into a regular table in place.
fn inline_to_table(item: &mut toml_edit::Item) {
    if let toml_edit::Item::Value(toml_edit::Value::InlineTable(_)) = item {
        let taken = std::mem::take(item);
        if let toml_edit::Item::Value(toml_edit::Value::InlineTable(it)) = taken {
            *item = toml_edit::Item::Table(it.into_table());
        }
    }
}

/// Sets `[mcp_servers.context-init] command/args` in a Codex-style TOML config,
/// preserving comments and formatting of everything else.
pub fn merge_toml(existing: Option<&str>, exe: &Path) -> Result<String> {
    let mut doc: toml_edit::DocumentMut = existing
        .unwrap_or("")
        .parse()
        .context("existing config is not valid TOML")?;
    let servers = doc.entry("mcp_servers").or_insert_with(|| {
        let mut t = toml_edit::Table::new();
        t.set_implicit(true);
        toml_edit::Item::Table(t)
    });
    inline_to_table(servers);
    let servers = servers
        .as_table_mut()
        .context("`mcp_servers` in the existing config is not a table")?;
    let entry = servers
        .entry(SERVER_NAME)
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
    inline_to_table(entry);
    let entry = entry
        .as_table_mut()
        .context("`mcp_servers.context-init` is not a table")?;
    entry["command"] = toml_edit::value(exe.to_string_lossy().as_ref());
    let mut args = toml_edit::Array::new();
    args.push("mcp");
    entry["args"] = toml_edit::value(args);
    Ok(doc.to_string())
}

/// Caps for the preview diff, so a huge or minified config (e.g. `~/.claude.json`)
/// does not flood the terminal.
pub const DIFF_MAX_LINES: usize = 200;
pub const DIFF_MAX_LINE_CHARS: usize = 200;

fn clip_line(line: &str) -> String {
    match line.char_indices().nth(DIFF_MAX_LINE_CHARS) {
        Some((cut, _)) => format!("{}… (+{} chars)", &line[..cut], line[cut..].chars().count()),
        None => line.to_string(),
    }
}

/// A file write computed in memory; applied only after the user asked for `--write`.
#[derive(Debug, Clone)]
pub struct PlannedWrite {
    pub path: PathBuf,
    pub old: Option<String>,
    pub new: String,
}

impl PlannedWrite {
    /// Reads the current content of `path` (if any) to compare against `new`.
    pub fn new(path: PathBuf, new: String) -> Result<Self> {
        let old = match fs::read_to_string(&path) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("Failed to read {:?}", path)),
        };
        Ok(Self { path, old, new })
    }

    pub fn is_noop(&self) -> bool {
        self.old.as_deref() == Some(self.new.as_str())
    }

    /// A compact unified-style diff: the changed middle between the common leading and
    /// trailing lines (enough for a single-key change, linear in file size).
    pub fn unified_diff(&self) -> String {
        let old: Vec<&str> = self.old.as_deref().unwrap_or("").lines().collect();
        let new: Vec<&str> = self.new.lines().collect();
        let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let max_suffix = old.len().min(new.len()) - prefix;
        let suffix = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(max_suffix)
            .take_while(|(a, b)| a == b)
            .count();
        let mut out = format!(
            "--- {}\n+++ {}\n@@ line {} @@\n",
            if self.old.is_some() {
                self.path.display().to_string()
            } else {
                "/dev/null".to_string()
            },
            self.path.display(),
            prefix + 1
        );
        let changed = old[prefix..old.len() - suffix]
            .iter()
            .map(|l| ('-', *l))
            .chain(new[prefix..new.len() - suffix].iter().map(|l| ('+', *l)));
        let total = old.len() - suffix - prefix + new.len() - suffix - prefix;
        for (sign, line) in changed.take(DIFF_MAX_LINES) {
            out.push(sign);
            out.push_str(&clip_line(line));
            out.push('\n');
        }
        if total > DIFF_MAX_LINES {
            out.push_str(&format!("… ({} more lines)\n", total - DIFF_MAX_LINES));
        }
        out
    }

    /// Writes `new` (creating parent dirs). An existing file is first copied to
    /// `<path>.bak.<stamp>`, whose path is returned. Unchanged files are not touched.
    pub fn apply(&self, stamp: &str) -> Result<Option<PathBuf>> {
        if self.is_noop() {
            return Ok(None);
        }
        // Re-check the disk: another planned write may have already produced this content
        // (a file shared between targets, e.g. `GEMINI.md`).
        if fs::read_to_string(&self.path).ok().as_deref() == Some(self.new.as_str()) {
            return Ok(None);
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("Failed to create {:?}", parent))?;
        }
        let backup = match &self.old {
            Some(_) => {
                let b = PathBuf::from(format!("{}.bak.{stamp}", self.path.display()));
                if b.exists() {
                    // The backup of the original is already there; never overwrite it.
                    None
                } else {
                    // `copy` keeps the original's permission bits (configs may hold secrets).
                    fs::copy(&self.path, &b)
                        .with_context(|| format!("Failed to write backup {:?}", b))?;
                    Some(b)
                }
            }
            None => None,
        };
        atomic_write_str(&self.path, &self.new)?;
        Ok(backup)
    }
}

/// Writes `content` to a sibling temp file and renames it over `path`, so a crash never
/// leaves a half-written config. Permissions of an existing `path` are kept.
pub fn atomic_write_str(path: &Path, content: &str) -> Result<()> {
    use std::io::Write;
    // Write through a symlinked config (e.g. managed by GNU Stow) instead of replacing the link.
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = resolved.as_path();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}.tmp.{}.{nanos}", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .with_context(|| format!("Failed to create {:?}", tmp))?;
        if let Ok(meta) = fs::metadata(path) {
            fs::set_permissions(&tmp, meta.permissions())?;
        }
        file.write_all(content.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path).with_context(|| format!("Failed to replace {:?}", path))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn entry() -> Value {
        json!({"command": "/opt/ci/context-init", "args": ["mcp"]})
    }

    #[test]
    fn unified_diff_is_capped_for_huge_or_minified_files() {
        let long_line = "é".repeat(100_000);
        let many_lines = (0..5_000)
            .map(|i| format!("line {i}\n"))
            .collect::<String>();
        let plan = PlannedWrite {
            path: PathBuf::from("/h/.claude.json"),
            old: Some(long_line.clone()),
            new: format!("{long_line}x\n{many_lines}"),
        };
        let diff = plan.unified_diff();
        assert!(
            diff.lines().count() <= DIFF_MAX_LINES + 5,
            "{}",
            diff.lines().count()
        );
        assert!(diff
            .lines()
            .all(|l| l.chars().count() <= DIFF_MAX_LINE_CHARS + 32));
        assert!(diff.contains("more lines"), "{diff}");
    }

    #[test]
    fn merge_json_into_absent_file() {
        let out = merge_json(None, entry()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v["mcpServers"]["context-init"]["command"],
            "/opt/ci/context-init"
        );
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn merge_json_preserves_other_servers_and_key_order() {
        let existing = r#"{"zeta": 1, "mcpServers": {"other": {"command": "x"}}, "alpha": true}"#;
        let out = merge_json(Some(existing), entry()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["zeta", "mcpServers", "alpha"]);
    }

    #[test]
    fn merge_json_replaces_stale_entry_keeping_user_keys() {
        let existing = r#"{"mcpServers": {"context-init": {"command": "/old", "args": ["mcp", "."], "env": {"A": "1"}}}}"#;
        let out = merge_json(Some(existing), entry()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let e = &v["mcpServers"]["context-init"];
        assert_eq!(e["command"], "/opt/ci/context-init");
        assert_eq!(e["args"], json!(["mcp"]));
        assert_eq!(e["env"]["A"], "1");
    }

    #[test]
    fn merge_json_invalid_input_errors() {
        assert!(merge_json(Some("{ not json"), entry()).is_err());
        assert!(merge_json(Some("[1, 2]"), entry()).is_err());
        assert!(merge_json(Some(r#"{"mcpServers": 3}"#), entry()).is_err());
    }

    #[test]
    fn merge_toml_preserves_comments() {
        let existing = "# keep me\nmodel = \"o3\"\n\n[mcp_servers.x]\ncommand = \"x\" # inline\n";
        let out = merge_toml(Some(existing), Path::new("/opt/ci/context-init")).unwrap();
        assert!(out.contains("# keep me"), "{out}");
        assert!(out.contains("# inline"), "{out}");
        let v: toml::Value = toml::from_str(&out).unwrap();
        assert_eq!(v["mcp_servers"]["x"]["command"].as_str(), Some("x"));
        assert_eq!(
            v["mcp_servers"]["context-init"]["command"].as_str(),
            Some("/opt/ci/context-init")
        );
        assert_eq!(
            v["mcp_servers"]["context-init"]["args"][0].as_str(),
            Some("mcp")
        );
    }

    #[test]
    fn setup_exe_path_with_spaces_roundtrips() {
        let exe = Path::new("/opt/my tools/\"q\"/context-init");
        let json_out = merge_json(
            None,
            json!({"command": exe.to_string_lossy(), "args": ["mcp"]}),
        )
        .unwrap();
        let v: Value = serde_json::from_str(&json_out).unwrap();
        assert_eq!(
            v["mcpServers"]["context-init"]["command"],
            exe.to_string_lossy().as_ref()
        );
        let toml_out = merge_toml(None, exe).unwrap();
        let t: toml::Value = toml::from_str(&toml_out).unwrap();
        assert_eq!(
            t["mcp_servers"]["context-init"]["command"].as_str(),
            Some(exe.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn planned_write_apply_creates_backup_then_noop_on_rerun() {
        let dir = std::env::temp_dir().join(format!(
            "ci_setup_apply_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("sub/mcp.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{}\n").unwrap();

        let w =
            PlannedWrite::new(path.clone(), merge_json(Some("{}\n"), entry()).unwrap()).unwrap();
        assert!(!w.is_noop());
        assert!(w.unified_diff().contains("+"), "{}", w.unified_diff());
        let backup = w.apply("20261023-120000").unwrap().expect("backup path");
        assert_eq!(
            backup,
            PathBuf::from(format!("{}.bak.20261023-120000", path.display()))
        );
        assert_eq!(fs::read_to_string(&backup).unwrap(), "{}\n");

        let again = PlannedWrite::new(path.clone(), fs::read_to_string(&path).unwrap()).unwrap();
        assert!(again.is_noop());
        assert_eq!(again.apply("20261023-120001").unwrap(), None);
        assert!(!dir.join("sub/mcp.json.bak.20261023-120001").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn backup_keeps_original_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "ci_setup_perm_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, "secret = 1\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let w = PlannedWrite::new(path.clone(), "secret = 2\n".to_string()).unwrap();
        let backup = w.apply("20261023-120000").unwrap().unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&backup), 0o600);
        assert_eq!(mode(&path), 0o600);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_toml_converts_inline_tables() {
        let exe = Path::new("/opt/ci/context-init");
        let out = merge_toml(Some("mcp_servers = { other = { command = \"x\" } }\n"), exe).unwrap();
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("x"));
        assert_eq!(
            doc["mcp_servers"]["context-init"]["command"].as_str(),
            Some("/opt/ci/context-init")
        );

        let out = merge_toml(
            Some("[mcp_servers]\ncontext-init = { command = \"/old\" }\n"),
            exe,
        )
        .unwrap();
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert_eq!(
            doc["mcp_servers"]["context-init"]["command"].as_str(),
            Some("/opt/ci/context-init")
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_str_replaces_file_preserving_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("ci_atomic_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cfg.json");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        atomic_write_str(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let leftovers = fs::read_dir(&dir).unwrap().flatten().count();
        assert_eq!(leftovers, 1, "temp file must be renamed away");
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_str_writes_through_symlink() {
        let dir = std::env::temp_dir().join(format!("ci_atomic_link_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("real.json");
        let link = dir.join("link.json");
        fs::write(&target, "old").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        atomic_write_str(&link, "new").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        let _ = fs::remove_dir_all(&dir);
    }
}
