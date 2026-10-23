//! Where each supported AI harness keeps its MCP config and rules (spec §5.2).

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Home and per-user config roots, injectable for tests.
#[derive(Debug, Clone)]
pub struct HomeDirs {
    pub home: PathBuf,
    /// `$XDG_CONFIG_HOME` or `~/.config`.
    pub config: PathBuf,
    /// `%APPDATA%` on Windows.
    pub app_data: Option<PathBuf>,
}

impl HomeDirs {
    pub fn from_env() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .context("Cannot determine the home directory (HOME is not set)")?;
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let app_data = std::env::var_os("APPDATA").map(PathBuf::from);
        Ok(Self {
            home,
            config,
            app_data,
        })
    }
}

/// Supported AI harnesses (D22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Harness {
    Antigravity,
    ClaudeCode,
    ClaudeDesktop,
    Cursor,
    GeminiCli,
    Codex,
}

impl Harness {
    pub const ALL: [Harness; 6] = [
        Harness::Antigravity,
        Harness::ClaudeCode,
        Harness::ClaudeDesktop,
        Harness::Cursor,
        Harness::GeminiCli,
        Harness::Codex,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Harness::Antigravity => "antigravity",
            Harness::ClaudeCode => "claude-code",
            Harness::ClaudeDesktop => "claude-desktop",
            Harness::Cursor => "cursor",
            Harness::GeminiCli => "gemini-cli",
            Harness::Codex => "codex",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Json,
    Toml,
}

/// Where the short usage rule for agents goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleTarget {
    /// A whole skill file owned by context-init.
    SkillFile(PathBuf),
    /// A marker-delimited block inside a shared rules file.
    MarkedBlock(PathBuf),
    /// No rules file: print the text for the user to paste (Cursor keeps rules in its UI).
    PrintOnly,
    /// The harness has no rule mechanism.
    None,
}

#[derive(Debug, Clone)]
pub struct HarnessSpec {
    pub mcp_file: PathBuf,
    pub format: ConfigFormat,
    pub rule: RuleTarget,
}

/// User-level config locations per harness.
pub fn spec(h: Harness, dirs: &HomeDirs) -> HarnessSpec {
    let home = &dirs.home;
    let gemini_rules = RuleTarget::MarkedBlock(home.join(".gemini/GEMINI.md"));
    let (mcp_file, format, rule) = match h {
        Harness::ClaudeCode => (
            home.join(".claude.json"),
            ConfigFormat::Json,
            RuleTarget::SkillFile(home.join(".claude/skills/context-init/SKILL.md")),
        ),
        Harness::ClaudeDesktop => {
            let base = if cfg!(target_os = "macos") {
                home.join("Library/Application Support")
            } else if cfg!(windows) {
                dirs.app_data
                    .clone()
                    .unwrap_or_else(|| home.join("AppData/Roaming"))
            } else {
                dirs.config.clone()
            };
            (
                base.join("Claude/claude_desktop_config.json"),
                ConfigFormat::Json,
                RuleTarget::None,
            )
        }
        Harness::Cursor => (
            home.join(".cursor/mcp.json"),
            ConfigFormat::Json,
            RuleTarget::PrintOnly,
        ),
        Harness::GeminiCli => (
            home.join(".gemini/settings.json"),
            ConfigFormat::Json,
            gemini_rules,
        ),
        Harness::Antigravity => (
            home.join(".gemini/config/mcp_config.json"),
            ConfigFormat::Json,
            gemini_rules,
        ),
        Harness::Codex => (
            home.join(".codex/config.toml"),
            ConfigFormat::Toml,
            RuleTarget::MarkedBlock(home.join(".codex/AGENTS.md")),
        ),
    };
    HarnessSpec {
        mcp_file,
        format,
        rule,
    }
}

/// The JSON `mcpServers.context-init` entry for `h`.
pub fn server_entry(h: Harness, exe: &Path) -> Value {
    let mut entry = json!({ "command": exe.to_string_lossy(), "args": ["mcp"] });
    if h == Harness::ClaudeCode {
        entry["type"] = json!("stdio");
    }
    entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn dirs() -> HomeDirs {
        HomeDirs {
            home: PathBuf::from("/h"),
            config: PathBuf::from("/h/.config"),
            app_data: Some(PathBuf::from("/h/AppData/Roaming")),
        }
    }

    #[test]
    fn spec_paths_match_table() {
        let d = dirs();
        let cases: [(Harness, &str); 5] = [
            (Harness::ClaudeCode, "/h/.claude.json"),
            (Harness::Cursor, "/h/.cursor/mcp.json"),
            (Harness::GeminiCli, "/h/.gemini/settings.json"),
            (Harness::Antigravity, "/h/.gemini/config/mcp_config.json"),
            (Harness::Codex, "/h/.codex/config.toml"),
        ];
        for (h, path) in cases {
            assert_eq!(spec(h, &d).mcp_file, PathBuf::from(path), "{h:?}");
        }
        let desktop = spec(Harness::ClaudeDesktop, &d).mcp_file;
        assert!(
            desktop.ends_with("Claude/claude_desktop_config.json"),
            "{desktop:?}"
        );
        if cfg!(target_os = "linux") {
            assert!(desktop.starts_with("/h/.config"));
        }
        assert!(matches!(
            spec(Harness::Codex, &d).format,
            ConfigFormat::Toml
        ));
        assert!(matches!(
            spec(Harness::ClaudeCode, &d).rule,
            RuleTarget::SkillFile(ref p) if p == Path::new("/h/.claude/skills/context-init/SKILL.md")
        ));
        assert!(matches!(
            spec(Harness::Cursor, &d).rule,
            RuleTarget::PrintOnly
        ));
        assert!(matches!(
            spec(Harness::ClaudeDesktop, &d).rule,
            RuleTarget::None
        ));
    }

    #[test]
    fn server_entry_shape() {
        let exe = Path::new("/bin/context-init");
        let cc = server_entry(Harness::ClaudeCode, exe);
        assert_eq!(cc["type"], "stdio");
        assert_eq!(cc["args"][0], "mcp");
        assert!(server_entry(Harness::Cursor, exe).get("type").is_none());
    }
}
