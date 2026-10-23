//! OpenViking OKF Sidecar card format (`---` YAML frontmatter + Markdown body) and L0 abstract extraction (`openviking_spec.md` §2).

use crate::storage::{normalize_line_endings, normalize_path};
use anyhow::{bail, Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Maximum character budget for L0 abstract (`openviking_spec.md` §1: ~256 chars).
pub const L0_ABSTRACT_MAX_CHARS: usize = 256;

/// Source provenance metadata in an OKF card frontmatter (`source:`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CardSource {
    /// Source VCS kind (default `"git"`).
    pub kind: String,
    /// Git blob or tree SHA of the source at generation time.
    pub blob_sha: String,
    /// Git commit SHA at generation time.
    pub commit_sha: String,
}

impl Default for CardSource {
    fn default() -> Self {
        Self {
            kind: "git".to_string(),
            blob_sha: String::new(),
            commit_sha: String::new(),
        }
    }
}

/// Generator metadata in an OKF card frontmatter (`generated_by:`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CardGeneratedBy {
    /// Generating component identifier (default `"context-init"`).
    pub component: String,
    /// Execution role (`"init"`, `"update"`, `"task"`).
    pub role: String,
    /// Model alias used for generation (e.g. `"local"`, `"none"`).
    pub model: String,
}

impl Default for CardGeneratedBy {
    fn default() -> Self {
        Self {
            component: "context-init".to_string(),
            role: "init".to_string(),
            model: "none".to_string(),
        }
    }
}

/// Freshness counters in an OKF card frontmatter (`freshness:`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CardFreshness {
    /// Total entries in the directory/subsystem.
    pub total_entries: usize,
    /// Number of entries sampled into this card.
    pub sampled_entries: usize,
    /// Number of entries not sampled.
    pub unsampled_entries: usize,
    /// Number of pending child changes awaiting bubbling.
    pub pending_child_changes: usize,
}

/// Structured YAML frontmatter of an OpenViking OKF Sidecar card (`openviking_spec.md` §2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CardFrontmatter {
    /// Normalized relative path or URI of the directory/object (always `/` separators).
    pub directory: String,
    /// Source git SHA tracking metadata.
    pub source: CardSource,
    /// Generator component, role, and model metadata.
    pub generated_by: CardGeneratedBy,
    /// Entry sampling and bubbling freshness metrics.
    pub freshness: CardFreshness,
}

impl CardFrontmatter {
    /// Serializes the frontmatter fields into YAML text (without `---` delimiters).
    pub fn to_yaml(&self) -> String {
        let norm_dir = normalize_path(&self.directory);
        format!(
            "directory: {}\nsource:\n  kind: {}\n  blob_sha: {}\n  commit_sha: {}\ngenerated_by:\n  component: {}\n  role: {}\n  model: {}\nfreshness:\n  total_entries: {}\n  sampled_entries: {}\n  unsampled_entries: {}\n  pending_child_changes: {}\n",
            format_yaml_scalar(&norm_dir),
            format_yaml_scalar(&self.source.kind),
            format_yaml_scalar(&self.source.blob_sha),
            format_yaml_scalar(&self.source.commit_sha),
            format_yaml_scalar(&self.generated_by.component),
            format_yaml_scalar(&self.generated_by.role),
            format_yaml_scalar(&self.generated_by.model),
            self.freshness.total_entries,
            self.freshness.sampled_entries,
            self.freshness.unsampled_entries,
            self.freshness.pending_child_changes,
        )
    }

    /// Parses `CardFrontmatter` from YAML text (inside `--- ... ---`), resilient to CRLF line endings.
    pub fn parse_yaml(yaml_text: &str) -> Result<Self> {
        let normalized = normalize_line_endings(yaml_text);

        let mut directory: Option<String> = None;
        let mut source_kind: Option<String> = None;
        let mut blob_sha: Option<String> = None;
        let mut commit_sha: Option<String> = None;

        let mut component: Option<String> = None;
        let mut role: Option<String> = None;
        let mut model: Option<String> = None;

        let mut total_entries: Option<usize> = None;
        let mut sampled_entries: Option<usize> = None;
        let mut unsampled_entries: Option<usize> = None;
        let mut pending_child_changes: Option<usize> = None;

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Section {
            Root,
            Source,
            GeneratedBy,
            Freshness,
        }

        let mut section = Section::Root;

        for (line_idx, raw_line) in normalized.lines().enumerate() {
            let line_no_comment = strip_yaml_inline_comment(raw_line);
            let trimmed = line_no_comment.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let is_indented = line_no_comment.starts_with(' ') || line_no_comment.starts_with('\t');
            if !is_indented {
                section = Section::Root;
            }

            let Some((raw_key, raw_val)) = trimmed.split_once(':') else {
                bail!(
                    "Malformed OKF frontmatter line {}: expected 'key: value', got '{}'",
                    line_idx + 1,
                    trimmed
                );
            };

            let key = raw_key.trim();
            let val_trimmed = raw_val.trim();
            let val = unquote_yaml_value(val_trimmed);

            let mut apply_section_kv =
                |sec: Section, k: &str, v: String, lno: usize| -> Result<()> {
                    match (sec, k) {
                        (Section::Source, "kind") => source_kind = Some(v),
                        (Section::Source, "blob_sha") => blob_sha = Some(v),
                        (Section::Source, "commit_sha") => commit_sha = Some(v),
                        (Section::GeneratedBy, "component") => component = Some(v),
                        (Section::GeneratedBy, "role") => role = Some(v),
                        (Section::GeneratedBy, "model") => model = Some(v),
                        (Section::Freshness, "total_entries") => {
                            total_entries = Some(v.parse().with_context(|| {
                                format!("Invalid freshness.total_entries '{}'", v)
                            })?);
                        }
                        (Section::Freshness, "sampled_entries") => {
                            sampled_entries = Some(v.parse().with_context(|| {
                                format!("Invalid freshness.sampled_entries '{}'", v)
                            })?);
                        }
                        (Section::Freshness, "unsampled_entries") => {
                            unsampled_entries = Some(v.parse().with_context(|| {
                                format!("Invalid freshness.unsampled_entries '{}'", v)
                            })?);
                        }
                        (Section::Freshness, "pending_child_changes") => {
                            pending_child_changes = Some(v.parse().with_context(|| {
                                format!("Invalid freshness.pending_child_changes '{}'", v)
                            })?);
                        }
                        (_, other) => {
                            bail!(
                                "Unknown nested OKF frontmatter key '{}' at line {}",
                                other,
                                lno
                            );
                        }
                    }
                    Ok(())
                };

            if section == Section::Root {
                match key {
                    "directory" => {
                        if val.is_empty() {
                            bail!("OKF frontmatter 'directory' must not be empty");
                        }
                        directory = Some(normalize_path(&val));
                    }
                    "source" | "generated_by" | "freshness" => {
                        let target_sec = match key {
                            "source" => Section::Source,
                            "generated_by" => Section::GeneratedBy,
                            _ => Section::Freshness,
                        };
                        if val_trimmed.is_empty() {
                            section = target_sec;
                        } else if let Some(flow_inner) = val_trimmed
                            .strip_prefix('{')
                            .and_then(|s| s.strip_suffix('}'))
                        {
                            for pair in flow_inner.split(',') {
                                let pair_trimmed = pair.trim();
                                if pair_trimmed.is_empty() {
                                    continue;
                                }
                                let Some((fk, fv)) = pair_trimmed.split_once(':') else {
                                    bail!(
                                        "Malformed flow map entry '{}' at line {}",
                                        pair_trimmed,
                                        line_idx + 1
                                    );
                                };
                                apply_section_kv(
                                    target_sec,
                                    fk.trim(),
                                    unquote_yaml_value(fv.trim()),
                                    line_idx + 1,
                                )?;
                            }
                        } else {
                            bail!(
                                "Unexpected scalar value for section '{}' at line {}",
                                key,
                                line_idx + 1
                            );
                        }
                    }
                    other => {
                        bail!("Unknown top-level OKF frontmatter key '{}'", other);
                    }
                }
            } else {
                apply_section_kv(section, key, val, line_idx + 1)?;
            }
        }

        let directory = directory.context("Missing required OKF frontmatter field 'directory'")?;
        let blob_sha =
            blob_sha.context("Missing required OKF frontmatter field 'source.blob_sha'")?;
        let commit_sha =
            commit_sha.context("Missing required OKF frontmatter field 'source.commit_sha'")?;
        let role = role.context("Missing required OKF frontmatter field 'generated_by.role'")?;
        let model = model.context("Missing required OKF frontmatter field 'generated_by.model'")?;

        Ok(Self {
            directory,
            source: CardSource {
                kind: source_kind.unwrap_or_else(|| "git".to_string()),
                blob_sha,
                commit_sha,
            },
            generated_by: CardGeneratedBy {
                component: component.unwrap_or_else(|| "context-init".to_string()),
                role,
                model,
            },
            freshness: CardFreshness {
                total_entries: total_entries.unwrap_or(0),
                sampled_entries: sampled_entries.unwrap_or(0),
                unsampled_entries: unsampled_entries.unwrap_or(0),
                pending_child_changes: pending_child_changes.unwrap_or(0),
            },
        })
    }
}

/// Complete OpenViking OKF Sidecar card (`CardFrontmatter` + Markdown body).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OkfCard {
    /// Structured metadata frontmatter.
    pub frontmatter: CardFrontmatter,
    /// Markdown body following the closing `---` delimiter.
    pub body: String,
}

impl OkfCard {
    /// Creates a new `OkfCard` with normalized directory path and line endings.
    pub fn new(mut frontmatter: CardFrontmatter, body: impl Into<String>) -> Self {
        frontmatter.directory = normalize_path(&frontmatter.directory);
        let normalized_body = normalize_line_endings(&body.into()).trim().to_string();
        Self {
            frontmatter,
            body: normalized_body,
        }
    }

    /// Parses an `OkfCard` from full Markdown text containing `--- ... ---` frontmatter and a Markdown body.
    pub fn parse(markdown: &str) -> Result<Self> {
        let stripped = markdown.strip_prefix('\u{feff}').unwrap_or(markdown);
        let normalized = normalize_line_endings(stripped);
        let trimmed_start = normalized.trim_start_matches('\n');

        let Some(after_open) = strip_opening_frontmatter_delimiter(trimmed_start) else {
            bail!("OKF card must start with '---' YAML frontmatter delimiter");
        };

        let (yaml_part, body_part) = split_frontmatter_and_body(after_open)?;
        let frontmatter = CardFrontmatter::parse_yaml(yaml_part)?;
        let body = body_part.trim().to_string();

        Ok(Self { frontmatter, body })
    }

    /// Serializes the `OkfCard` into standard OKF Markdown (`--- ... ---` + blank line + body + trailing `\n`).
    pub fn to_markdown(&self) -> String {
        let yaml = self.frontmatter.to_yaml();
        let body_trimmed = normalize_line_endings(&self.body).trim().to_string();
        if body_trimmed.is_empty() {
            format!("---\n{}---\n", yaml)
        } else {
            format!("---\n{}---\n\n{}\n", yaml, body_trimmed)
        }
    }

    /// Extracts the L0 abstract (first paragraph after `# H1`, capped at `L0_ABSTRACT_MAX_CHARS`).
    pub fn l0_abstract(&self) -> String {
        extract_l0_abstract(&self.body)
    }
}

impl fmt::Display for OkfCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_markdown())
    }
}

impl FromStr for OkfCard {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

/// Extracts the L0 abstract (first paragraph after `# H1`, up to 256 characters) from Markdown content (`openviking_spec.md` §1–2).
pub fn extract_l0_abstract(markdown: &str) -> String {
    extract_l0_abstract_with_limit(markdown, L0_ABSTRACT_MAX_CHARS)
}

/// Extracts the L0 abstract (first paragraph after `# H1`) with a configurable character limit.
pub fn extract_l0_abstract_with_limit(markdown: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let stripped = markdown.strip_prefix('\u{feff}').unwrap_or(markdown);
    let normalized = normalize_line_endings(stripped);

    // Strip YAML frontmatter if the caller passed a full OKF card instead of just the body.
    let body = if let Some(after_open) =
        strip_opening_frontmatter_delimiter(normalized.trim_start_matches('\n'))
    {
        if let Ok((_, b)) = split_frontmatter_and_body(after_open) {
            b
        } else {
            normalized.as_str()
        }
    } else {
        normalized.as_str()
    };

    let lines: Vec<&str> = body.lines().collect();

    // Find the first `# H1` line (single `#` followed by whitespace).
    let h1_index = lines.iter().position(|line| {
        let t = line.trim_start();
        t.starts_with("# ") || t == "#"
    });

    let start_idx = h1_index.map(|i| i + 1).unwrap_or(0);
    let mut paragraph_lines: Vec<&str> = Vec::new();

    for &line in &lines[start_idx..] {
        let t = line.trim();
        if t.is_empty() {
            if !paragraph_lines.is_empty() {
                break;
            }
            continue;
        }
        // Stop if we hit another Markdown heading or horizontal rule.
        if t.starts_with('#') || t == "---" {
            if h1_index.is_some() || !paragraph_lines.is_empty() {
                break;
            }
            continue;
        }
        // Stop at bullet list or code fence boundaries.
        if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("```") {
            break;
        }
        paragraph_lines.push(t);
    }

    let joined = paragraph_lines.join(" ");
    truncate_chars(&joined, max_chars)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    if max_chars <= 3 {
        return text.chars().take(max_chars).collect();
    }
    let prefix: String = text.chars().take(max_chars - 3).collect();
    format!("{}...", prefix.trim_end())
}

fn strip_opening_frontmatter_delimiter(input: &str) -> Option<&str> {
    let first_line_end = input.find('\n').unwrap_or(input.len());
    let first_line = input[..first_line_end].trim_end_matches([' ', '\t', '\r']);
    if first_line == "---" {
        if first_line_end < input.len() {
            Some(&input[first_line_end + 1..])
        } else {
            Some("")
        }
    } else {
        None
    }
}

fn split_frontmatter_and_body(after_open: &str) -> Result<(&str, &str)> {
    let mut offset = 0usize;
    for line in after_open.split_inclusive('\n') {
        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r').trim();
        if trimmed == "---" {
            let yaml_part = &after_open[..offset];
            let body_part = &after_open[offset + line.len()..];
            return Ok((yaml_part, body_part));
        }
        offset += line.len();
    }
    bail!("Unclosed OKF frontmatter: missing closing '---' delimiter")
}

fn format_yaml_scalar(val: &str) -> String {
    let needs_quotes = val.contains('#')
        || val.contains(':')
        || val.starts_with('{')
        || val.contains('"')
        || val.contains('\'')
        || val.contains('\\')
        || val.trim() != val;
    if needs_quotes {
        let escaped = val.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        val.to_string()
    }
}

fn strip_yaml_inline_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut prev_whitespace = true;

    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            prev_whitespace = false;
            continue;
        }
        match ch {
            '\\' if in_double => {
                escaped = true;
                prev_whitespace = false;
                continue;
            }
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double && prev_whitespace => {
                return line[..idx].trim_end();
            }
            _ => {}
        }
        prev_whitespace = ch.is_whitespace();
    }
    line
}

fn unquote_yaml_value(val: &str) -> String {
    if val.len() >= 2 && val.starts_with('"') && val.ends_with('"') {
        let inner = &val[1..val.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                } else {
                    out.push('\\');
                }
            } else {
                out.push(ch);
            }
        }
        out
    } else if val.len() >= 2 && val.starts_with('\'') && val.ends_with('\'') {
        val[1..val.len() - 1].to_string()
    } else {
        val.to_string()
    }
}
