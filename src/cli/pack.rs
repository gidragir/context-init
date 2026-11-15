//! `pack`: compressed AI context for a subsystem (built-in AST packer or repomix).

use anyhow::{bail, Context, Result};
use clap::Args;
use colored::Colorize;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use ci_core::analysis::detector::count_tokens;
use ci_core::analysis::pack::{
    pack_subsystem_with, CompressOptions, PackMode, PackRequest, PackStyle,
};
use ci_core::analysis::subsystems::subsystem_tokens_cached;
use ci_core::config::is_binary_available;
use ci_mcp::pagination::{compute_query_hash, decode_cursor, encode_cursor, CursorData};

use super::{telemetry_scope, GlobalArgs};

/// Prefix of a page degraded to the compact style.
const AUTO_COMPACT_PREFIX: &str = "[auto-compressed: exceeded budget -> compact]";
/// Prefix of a page degraded to the table of contents.
const AUTO_TOC_PREFIX: &str = "[auto-toc: drill down via --files <f> --no-compress]";

#[derive(Args, Debug, Clone)]
pub struct PackArgs {
    /// Subsystem id from the L0 index, or `all`
    pub subsystem_id: String,
    /// Project directory
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Compress (strip Rust function bodies, normalize whitespace) [default]
    #[arg(long, conflicts_with = "no_compress")]
    pub compress: bool,
    /// Keep full source text
    #[arg(long)]
    pub no_compress: bool,
    /// Use the external `repomix` tool instead of the built-in packer
    #[arg(long)]
    pub repomix: bool,
    /// Output format: compact, xml or markdown
    #[arg(long, default_value = "compact")]
    pub style: String,
    /// Only pack files matching these globs / short file names (repeatable or comma-separated)
    #[arg(long = "files", alias = "filter", value_delimiter = ',')]
    pub files: Vec<String>,
    /// Only pack these symbols (repeatable or comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub symbols: Vec<String>,
    /// Pack mode: full, compressed or toc (overrides --compress/--no-compress)
    #[arg(long)]
    pub mode: Option<String>,
    /// Table of contents only (same as `--mode toc`)
    #[arg(long)]
    pub toc: bool,
    /// Signatures with line spans only (alias of `--toc`)
    #[arg(long)]
    pub signatures_only: bool,
    /// Keep only public items
    #[arg(long)]
    pub public_only: bool,
    /// Keep `use`/import lines in the compact style
    #[arg(long)]
    pub keep_imports: bool,
    /// Keep test code in the compact style
    #[arg(long)]
    pub include_tests: bool,
    /// Output line budget: degrade full -> compact -> toc, then paginate
    #[arg(long)]
    pub max_lines: Option<usize>,
    /// Output token budget: degrade full -> compact -> toc, then paginate
    #[arg(long)]
    pub max_tokens: Option<usize>,
    /// Continue a paginated output (`cur_...` from the previous page hint)
    #[arg(long)]
    pub cursor: Option<String>,
}

/// Maps a built-in pack style to the style name understood by `repomix`.
fn repomix_style(style: PackStyle) -> &'static str {
    match style {
        PackStyle::Compact => "plain",
        PackStyle::Markdown => "markdown",
        PackStyle::Xml => "xml",
    }
}

/// Compression options for a style/mode pair, adjusted by the user flags.
fn compress_options(style: PackStyle, mode: PackMode, args: &PackArgs) -> CompressOptions {
    let mut opts = if style == PackStyle::Compact && mode == PackMode::Compressed {
        CompressOptions::compact()
    } else {
        CompressOptions::legacy()
    };
    if args.public_only {
        opts.public_only = true;
    }
    if args.keep_imports {
        opts.strip_imports = false;
    }
    if args.include_tests {
        opts.exclude_tests = false;
    }
    opts
}

/// One rung of the degradation ladder: what to render and the note prepended to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetLevel {
    pub style: PackStyle,
    pub mode: PackMode,
    pub options: CompressOptions,
    pub prefix: Option<&'static str>,
}

/// Output budget for stdout (`None` = unlimited).
#[derive(Debug, Clone, Copy, Default)]
pub struct CliBudget<'a> {
    pub max_lines: Option<usize>,
    pub max_tokens: Option<usize>,
    pub cursor: Option<&'a str>,
}

impl CliBudget<'_> {
    fn is_active(&self) -> bool {
        self.max_lines.is_some() || self.max_tokens.is_some() || self.cursor.is_some()
    }

    fn fits(&self, text: &str) -> bool {
        self.max_lines.is_none_or(|m| text.lines().count() <= m)
            && self.max_tokens.is_none_or(|m| count_tokens(text) <= m)
    }
}

/// Applies the `full -> compact -> toc -> paginate` ladder.
///
/// Renders `levels` in order and stops at the first one that fits the budget; when none fits
/// (or a cursor is given) the last rendered text is paginated line by line. `query` binds
/// cursors to the request parameters.
pub fn apply_cli_budget(
    levels: &[BudgetLevel],
    budget: &CliBudget<'_>,
    query: &[&str],
    render: &mut dyn FnMut(&BudgetLevel) -> Result<String>,
) -> Result<String> {
    let mut text = String::new();
    for level in levels {
        let body = render(level)?;
        text = match level.prefix {
            Some(p) => format!("{p}\n{body}"),
            None => body,
        };
        if budget.fits(&text) {
            break;
        }
    }
    if budget.cursor.is_none() && budget.fits(&text) {
        return Ok(text);
    }
    paginate_lines(&text, budget, compute_query_hash(query, &text))
}

/// Opening fence line if `text` ends inside a ```` ``` ```` code block.
fn open_fence(text: &str) -> Option<&str> {
    let mut open: Option<&str> = None;
    for line in text.lines() {
        let t = line.trim_start();
        if !t.starts_with("```") {
            continue;
        }
        match open {
            None => open = Some(line),
            Some(o) => {
                let ticks = o.trim_start().chars().take_while(|&c| c == '`').count();
                let t = t.trim_end();
                if t.chars().all(|c| c == '`') && t.len() >= ticks {
                    open = None;
                }
            }
        }
    }
    open
}

/// One page of `text` from the cursor, strictly within `max_lines`/`max_tokens`.
///
/// Before cutting the body, one line is reserved for a closing fence (when the cut lands
/// inside a code block) and one for the `--cursor "cur_..."` hint; a page that starts inside
/// a code block reopens it with the original fence line.
fn paginate_lines(text: &str, budget: &CliBudget<'_>, query_hash: u64) -> Result<String> {
    let (start, page) = match budget.cursor {
        None => (0, 0),
        Some(c) => {
            let cur = decode_cursor(c).context("Invalid --cursor value")?;
            if cur.query_hash.is_some_and(|h| h != query_hash) {
                bail!(
                    "--cursor does not match this request (arguments or content changed); restart without --cursor"
                );
            }
            let at_line_start =
                cur.byte_offset == 0 || text.as_bytes().get(cur.byte_offset - 1) == Some(&b'\n');
            if cur.byte_offset > text.len() || !at_line_start {
                bail!("Invalid --cursor value: offset out of range");
            }
            (cur.byte_offset, cur.page)
        }
    };
    let reopen = open_fence(&text[..start]);
    let lines: Vec<&str> = text[start..].split_inclusive('\n').collect();
    let render = |n: usize, hint: bool| -> String {
        let mut out = String::new();
        if let Some(f) = reopen {
            out.push_str(f);
            out.push('\n');
        }
        let body: String = lines[..n].concat();
        out.push_str(&body);
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if open_fence(&out).is_some() {
            out.push_str("```\n");
        }
        if hint {
            let cursor = encode_cursor(&CursorData {
                byte_offset: start + body.len(),
                page: page + 1,
                query_hash: Some(query_hash),
            });
            out.push_str(&format!(
                "[page {}: truncated, rerun with the same arguments and --cursor \"{cursor}\"]\n",
                page + 1
            ));
        }
        out
    };
    let whole = render(lines.len(), false);
    if budget.fits(&whole) {
        return Ok(whole);
    }
    // Largest body that fits together with the reserved fence and hint lines.
    let (mut lo, mut hi) = (0usize, lines.len().saturating_sub(1));
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if budget.fits(&render(mid, true)) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    while lo > 0 && !budget.fits(&render(lo, true)) {
        lo -= 1;
    }
    if lo == 0 {
        bail!("Output budget too small for a single line: increase --max-lines/--max-tokens");
    }
    Ok(render(lo, true))
}

pub fn run(global: &GlobalArgs, args: &PackArgs) -> Result<ExitCode> {
    let project = global.load_project(&args.path, None)?;
    let index = project.load_l0_index()?;
    let compress = !args.no_compress;
    let style = PackStyle::try_parse(&args.style)?;
    let mode = match &args.mode {
        Some(m) => PackMode::try_parse(m)?,
        None if args.toc || args.signatures_only => PackMode::Toc,
        None if compress => PackMode::Compressed,
        None => PackMode::Full,
    };
    let options = compress_options(style, mode, args);
    let mut req = PackRequest::new(&project.root, &args.subsystem_id);
    req.files = &args.files;
    req.symbols = &args.symbols;
    req.mode = mode;
    req.style = style;
    req.options = options;
    let packed = pack_subsystem_with(&project.root, &project.knowledge_dir, &index, &req)?;

    let tel = telemetry_scope(&project);
    if tel.is_active() {
        let cache = project.metrics_cache();
        let source_tokens: usize = packed
            .subsystems
            .iter()
            .map(|s| subsystem_tokens_cached(&cache, s))
            .sum();
        let _ = cache.save();
        tel.efficiency(
            "context_pack",
            source_tokens,
            &packed.content,
            false,
            0,
            false,
            false,
        );
    }

    if args.repomix && is_binary_available("repomix") {
        let mut cmd = Command::new("repomix");
        cmd.arg("--include")
            .arg(packed.files.join(","))
            .arg("--style")
            .arg(repomix_style(style))
            .arg("--no-security-check")
            .arg("-o")
            .arg(args.output.as_deref().unwrap_or("-".as_ref()))
            .current_dir(&project.root);
        if compress {
            cmd.arg("--compress");
        }
        if cmd.status().map(|s| s.success()).unwrap_or(false) {
            return Ok(ExitCode::SUCCESS);
        }
        eprintln!("repomix failed; falling back to the built-in packer");
    }

    let budget = CliBudget {
        max_lines: args.max_lines,
        max_tokens: args.max_tokens,
        cursor: args.cursor.as_deref(),
    };
    let content = if budget.is_active() {
        let levels = ladder(style, mode, options, args);
        let mode_name = format!("{mode:?}");
        let style_name = format!("{style:?}");
        let mut query: Vec<&str> = vec![&args.subsystem_id, &style_name, &mode_name];
        query.extend(args.files.iter().map(String::as_str));
        query.push("--symbols");
        query.extend(args.symbols.iter().map(String::as_str));
        let mut first = Some(packed.content.clone());
        apply_cli_budget(&levels, &budget, &query, &mut |level| {
            if let Some(text) = first.take() {
                return Ok(text);
            }
            let mut r = req;
            r.style = level.style;
            r.mode = level.mode;
            r.options = level.options;
            Ok(pack_subsystem_with(&project.root, &project.knowledge_dir, &index, &r)?.content)
        })?
    } else {
        packed.content.clone()
    };

    match &args.output {
        Some(path) => {
            std::fs::write(path, &content)
                .with_context(|| format!("Failed to write {:?}", path))?;
            eprintln!(
                "{}",
                format!(
                    "Packed {} file(s), ~{} tokens into {}",
                    packed.files.len(),
                    count_tokens(&content),
                    path.display()
                )
                .green()
                .bold()
            );
        }
        None => std::io::stdout().write_all(content.as_bytes())?,
    }
    Ok(ExitCode::SUCCESS)
}

/// Ladder rungs for the requested style/mode.
///
/// An explicit full mode (`--no-compress` / `--mode full`) asks for bodies, so it is never
/// degraded and goes straight to pagination; `toc` is the last rung.
fn ladder(
    style: PackStyle,
    mode: PackMode,
    options: CompressOptions,
    args: &PackArgs,
) -> Vec<BudgetLevel> {
    let mut levels = vec![BudgetLevel {
        style,
        mode,
        options,
        prefix: None,
    }];
    if mode != PackMode::Compressed {
        return levels;
    }
    if style != PackStyle::Compact {
        levels.push(BudgetLevel {
            style: PackStyle::Compact,
            mode: PackMode::Compressed,
            options: compress_options(PackStyle::Compact, PackMode::Compressed, args),
            prefix: Some(AUTO_COMPACT_PREFIX),
        });
    }
    levels.push(BudgetLevel {
        style: PackStyle::Compact,
        mode: PackMode::Toc,
        options: compress_options(PackStyle::Compact, PackMode::Toc, args),
        prefix: Some(AUTO_TOC_PREFIX),
    });
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repomix_style_maps_all_styles() {
        assert_eq!(repomix_style(PackStyle::Compact), "plain");
        assert_eq!(repomix_style(PackStyle::Markdown), "markdown");
        assert_eq!(repomix_style(PackStyle::Xml), "xml");
    }

    fn level(prefix: Option<&'static str>) -> BudgetLevel {
        BudgetLevel {
            style: PackStyle::Markdown,
            mode: PackMode::Full,
            options: CompressOptions::legacy(),
            prefix,
        }
    }

    fn fenced_doc() -> String {
        let mut s = String::from("## File: `a.rs`\n```rust\n");
        for i in 0..40 {
            s.push_str(&format!("let x{i} = {i};\n"));
        }
        s.push_str("```\n");
        s
    }

    #[test]
    fn open_fence_tracks_nested_markers() {
        assert_eq!(open_fence("a\n```rust\nb\n"), Some("```rust"));
        assert_eq!(open_fence("```rust\nb\n```\n"), None);
        assert_eq!(open_fence("````md\n```\n"), Some("````md"));
    }

    #[test]
    fn apply_cli_budget_paginates_with_balanced_fences() {
        let doc = fenced_doc();
        let budget = CliBudget {
            max_lines: Some(10),
            ..Default::default()
        };
        let levels = [level(None)];
        let mut cursor: Option<String> = None;
        let mut seen = String::new();
        for _ in 0..20 {
            let b = CliBudget {
                cursor: cursor.as_deref(),
                ..budget
            };
            let page = apply_cli_budget(&levels, &b, &["q"], &mut |_| Ok(doc.clone())).unwrap();
            assert!(page.lines().count() <= 10, "{page}");
            assert_eq!(
                page.lines().filter(|l| l.starts_with("```")).count() % 2,
                0,
                "{page}"
            );
            seen.push_str(&page);
            cursor = page
                .split("--cursor \"")
                .nth(1)
                .map(|r| r.split('"').next().unwrap().to_string());
            if cursor.is_none() {
                break;
            }
        }
        assert!(cursor.is_none(), "pagination did not finish");
        for i in 0..40 {
            assert!(seen.contains(&format!("let x{i} = {i};")), "lost line {i}");
        }
    }

    #[test]
    fn apply_cli_budget_stops_at_first_fitting_level() {
        let levels = [level(None), level(Some("[auto-toc]"))];
        let budget = CliBudget {
            max_lines: Some(5),
            ..Default::default()
        };
        let mut calls = 0;
        let out = apply_cli_budget(&levels, &budget, &[], &mut |l| {
            calls += 1;
            Ok(if l.prefix.is_some() {
                "toc\n".into()
            } else {
                fenced_doc()
            })
        })
        .unwrap();
        assert_eq!(out, "[auto-toc]\ntoc\n");
        assert_eq!(calls, 2);
    }

    #[test]
    fn apply_cli_budget_respects_token_budget_and_rejects_foreign_cursor() {
        let doc = fenced_doc();
        let budget = CliBudget {
            max_tokens: Some(60),
            ..Default::default()
        };
        let page =
            apply_cli_budget(&[level(None)], &budget, &["a"], &mut |_| Ok(doc.clone())).unwrap();
        assert!(count_tokens(&page) <= 60, "{page}");
        let cur = page
            .split("--cursor \"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let foreign = CliBudget {
            cursor: Some(cur),
            ..budget
        };
        let err = apply_cli_budget(&[level(None)], &foreign, &["b"], &mut |_| Ok(doc.clone()))
            .unwrap_err();
        assert!(err.to_string().contains("cursor"), "{err}");
    }
}
