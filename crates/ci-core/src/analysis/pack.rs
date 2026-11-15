//! Built-in AST context packer (shared by CLI `pack` and MCP `context_pack`).

use std::path::Path;

use anyhow::{bail, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::storage::{safe_join, L0Index, SubsystemInfo};

/// Output format of a context pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackStyle {
    /// Plain `=== path (NL) ===` headers, no XML/Markdown framing.
    #[default]
    Compact,
    Markdown,
    Xml,
}

impl PackStyle {
    /// Strict parser: `compact`, `markdown` or `xml` (case-insensitive).
    pub fn try_parse(style: &str) -> Result<Self> {
        match style.trim().to_ascii_lowercase().as_str() {
            "compact" => Ok(Self::Compact),
            "markdown" => Ok(Self::Markdown),
            "xml" => Ok(Self::Xml),
            other => bail!("Unknown pack style '{other}': expected compact, markdown or xml"),
        }
    }

    /// Lenient parser: unknown values fall back to the default (Compact).
    pub fn parse(style: &str) -> Self {
        Self::try_parse(style).unwrap_or_default()
    }
}

/// How much of each file goes into the pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackMode {
    Full,
    #[default]
    Compressed,
    Toc,
}

/// Fine-grained compression switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressOptions {
    pub strip_imports: bool,
    pub public_only: bool,
    pub exclude_tests: bool,
    pub strip_comments: bool,
    pub inline_elide: bool,
}

impl CompressOptions {
    /// Legacy behavior: everything off, identical to `compress_source`.
    pub fn legacy() -> Self {
        Self {
            strip_imports: false,
            public_only: false,
            exclude_tests: false,
            strip_comments: false,
            inline_elide: false,
        }
    }

    /// Aggressive settings for the compact style.
    pub fn compact() -> Self {
        Self {
            strip_imports: true,
            public_only: false,
            exclude_tests: true,
            strip_comments: true,
            inline_elide: true,
        }
    }
}

impl Default for CompressOptions {
    fn default() -> Self {
        Self::legacy()
    }
}

/// Parameters of a pack operation.
#[derive(Debug, Clone, Copy)]
pub struct PackRequest<'a> {
    pub root: &'a Path,
    /// Subsystem id / name / `all` (used by `pack_subsystem_with`).
    pub subsystem_id: &'a str,
    pub subs: &'a [&'a SubsystemInfo],
    /// Glob / short-name filters over subsystem files (empty = no filtering).
    pub files: &'a [String],
    /// Symbol filters (`name`, `Type.name`, `Type::name`): only the exact source of matching
    /// declarations is packed (in `Toc` mode only matching TOC lines are listed).
    pub symbols: &'a [String],
    pub mode: PackMode,
    pub style: PackStyle,
    pub options: CompressOptions,
}

impl<'a> PackRequest<'a> {
    /// Request with defaults: no filters, compressed mode, compact style.
    pub fn new(root: &'a Path, subsystem_id: &'a str) -> Self {
        Self {
            root,
            subsystem_id,
            subs: &[],
            files: &[],
            symbols: &[],
            mode: PackMode::default(),
            style: PackStyle::default(),
            options: CompressOptions::legacy(),
        }
    }
}

impl PackMode {
    /// Strict parser: `full`, `compressed` or `toc` (case-insensitive).
    pub fn try_parse(mode: &str) -> Result<Self> {
        match mode.trim().to_ascii_lowercase().as_str() {
            "full" => Ok(Self::Full),
            "compressed" => Ok(Self::Compressed),
            "toc" => Ok(Self::Toc),
            other => bail!("Unknown pack mode '{other}': expected full, compressed or toc"),
        }
    }
}

/// Like `compress_source`, with explicit options; `CompressOptions::legacy()` is identical
/// to `compress_source`.
pub fn compress_source_with(content: &str, ext: &str, opts: &CompressOptions) -> String {
    match crate::analysis::lang::adapter_for_ext(ext) {
        Some(adapter) => adapter.compress_with(content, ext, opts),
        None => compress_code(content),
    }
}

fn xml_escape(s: &str) -> String {
    quick_xml::escape::escape(s).into_owned()
}

/// A packed subsystem selection.
#[derive(Debug, Clone)]
pub struct PackedSubsystems {
    /// Packed context text.
    pub content: String,
    /// Relative paths of the packed files.
    pub files: Vec<String>,
    /// Subsystems that matched the requested id.
    pub subsystems: Vec<SubsystemInfo>,
}

/// Selects subsystems by id or name (`all` = every subsystem) and packs their files that
/// exist on disk. A subsystem without file lists (legacy indexes) contributes its L1 card
/// from `knowledge_dir`. Shared by CLI `pack` and MCP `context_pack`.
pub fn pack_subsystem(
    root: &Path,
    knowledge_dir: &Path,
    index: &L0Index,
    subsystem_id: &str,
    compress: bool,
    style: PackStyle,
) -> Result<PackedSubsystems> {
    let mut req = PackRequest::new(root, subsystem_id);
    req.mode = if compress {
        PackMode::Compressed
    } else {
        PackMode::Full
    };
    req.style = style;
    pack_subsystem_with(root, knowledge_dir, index, &req)
}

/// Validates `files` filters and compiles them into a matcher. Each pattern matches the
/// full relative path, any path suffix (`**/{pat}`) and the path relative to the subsystem dir.
pub fn validate_file_patterns(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        let p = pat.trim();
        if p.is_empty() {
            bail!("Empty file pattern");
        }
        if p.contains('\0') {
            bail!("File pattern contains a NUL byte");
        }
        if p.starts_with('/') || p.starts_with('\\') {
            bail!("File pattern '{p}' must be relative (no leading slash)");
        }
        let b = p.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            bail!("File pattern '{p}' must not contain a drive prefix");
        }
        if p.contains('\\') {
            bail!("File pattern '{p}' must use '/' separators");
        }
        if p.split('/').any(|c| c == "..") {
            bail!("File pattern '{p}' must not contain '..'");
        }
        for variant in [p.to_string(), format!("**/{p}")] {
            let glob = GlobBuilder::new(&variant)
                .literal_separator(true)
                .build()
                .map_err(|e| anyhow::anyhow!("Invalid file pattern '{p}': {e}"))?;
            builder.add(glob);
        }
    }
    builder
        .build()
        .map_err(|e| anyhow::anyhow!("Invalid file patterns: {e}"))
}

/// Longest common directory prefix of a subsystem's files (`""` when none).
fn sub_dir(paths: &[&str]) -> String {
    let mut dir: Option<Vec<&str>> = None;
    for p in paths {
        let parts: Vec<&str> = p.split('/').collect();
        let parent = &parts[..parts.len().saturating_sub(1)];
        dir = Some(match dir {
            None => parent.to_vec(),
            Some(cur) => cur
                .iter()
                .zip(parent.iter())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| *a)
                .collect(),
        });
    }
    dir.map(|d| d.join("/")).unwrap_or_default()
}

/// True when `path` is a regular file strictly inside `root` (no secrets, no `..`, no symlinks).
fn is_safe_source(root: &Path, canonical_root: &Path, path: &str) -> bool {
    if path.contains('\0') || is_secret_path(path) {
        return false;
    }
    let Some(full) = safe_join(root, path) else {
        return false;
    };
    match std::fs::symlink_metadata(&full) {
        Ok(m) if !m.file_type().is_symlink() => {}
        _ => return false,
    }
    std::fs::canonicalize(&full).is_ok_and(|c| c.starts_with(canonical_root))
}

/// Request-driven variant of [`pack_subsystem`] with `files` filtering and path protection.
pub fn pack_subsystem_with(
    root: &Path,
    knowledge_dir: &Path,
    index: &L0Index,
    req: &PackRequest<'_>,
) -> Result<PackedSubsystems> {
    let subsystem_id = req.subsystem_id;
    let matcher = if req.files.is_empty() {
        None
    } else {
        Some(validate_file_patterns(req.files)?)
    };
    let wanted = subsystem_id.trim();
    let clean = wanted
        .trim_start_matches("subsystems/")
        .trim_end_matches(".md");
    let matched: Vec<&SubsystemInfo> = if wanted.eq_ignore_ascii_case("all") {
        index.subsystems.iter().collect()
    } else {
        index
            .subsystems
            .iter()
            .filter(|s| {
                [wanted, clean]
                    .iter()
                    .any(|w| s.id.eq_ignore_ascii_case(w) || s.name.eq_ignore_ascii_case(w))
            })
            .collect()
    };
    if matched.is_empty() {
        let available: Vec<&str> = index.subsystems.iter().map(|s| s.id.as_str()).collect();
        bail!(
            "Subsystem '{}' not found in L0 index. Available subsystems: {}",
            subsystem_id,
            available.join(", ")
        );
    }

    let canonical_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut files: Vec<String> = Vec::new();
    let mut cards: Vec<String> = Vec::new();
    let mut listed: Vec<String> = Vec::new();
    for sub in &matched {
        if sub.files.is_empty() && sub.key_files.is_empty() {
            if matcher.is_some() {
                continue;
            }
            let Some(card) = safe_join(knowledge_dir, &sub.l1_path) else {
                continue;
            };
            if card.is_file() {
                // Relative when the card lives in the repo, absolute for central storage.
                let path = card
                    .strip_prefix(root)
                    .unwrap_or(&card)
                    .to_string_lossy()
                    .replace('\\', "/");
                if !files.contains(&path) {
                    cards.push(path.clone());
                    files.push(path);
                }
            }
            continue;
        }
        let candidates: Vec<&String> = sub
            .files
            .iter()
            .map(|f| &f.path)
            .chain(sub.key_files.iter())
            .collect();
        let dir = sub_dir(&candidates.iter().map(|p| p.as_str()).collect::<Vec<_>>());
        for path in candidates {
            // The index is committed and may come from a clone: never pack secrets,
            // databases, symlinks or anything outside the project root.
            if !is_safe_source(root, &canonical_root, path) {
                continue;
            }
            if !listed.contains(path) {
                listed.push(path.clone());
            }
            if let Some(m) = &matcher {
                let rel = if dir.is_empty() {
                    path.as_str()
                } else {
                    path.strip_prefix(&dir)
                        .map(|r| r.trim_start_matches('/'))
                        .unwrap_or(path)
                };
                if !m.is_match(path.as_str()) && !m.is_match(rel) {
                    continue;
                }
            }
            if !files.contains(path) {
                files.push(path.clone());
            }
        }
    }
    if files.is_empty() {
        if matcher.is_some() {
            const MAX_LISTED: usize = 50;
            let more = listed.len().saturating_sub(MAX_LISTED);
            let mut shown = listed.iter().take(MAX_LISTED).cloned().collect::<Vec<_>>();
            if more > 0 {
                shown.push(format!("... and {more} more"));
            }
            bail!(
                "No files matched {:?} in subsystem '{}'. Available files: {}",
                req.files,
                subsystem_id,
                shown.join(", ")
            );
        }
        bail!(
            "No existing source files found on disk for subsystem '{}'",
            subsystem_id
        );
    }

    let rendered = render_pack(root, &matched, &files, req, &cards);
    if !req.symbols.is_empty() && rendered.symbol_hits == 0 {
        const MAX_LISTED: usize = 50;
        let more = rendered.available.len().saturating_sub(MAX_LISTED);
        let mut shown: Vec<String> = rendered
            .available
            .iter()
            .take(MAX_LISTED)
            .cloned()
            .collect();
        if more > 0 {
            shown.push(format!("... and {more} more"));
        }
        bail!(
            "No symbols matched {:?} in subsystem '{}'. Available symbols: {}",
            req.symbols,
            subsystem_id,
            shown.join(", ")
        );
    }
    Ok(PackedSubsystems {
        content: rendered.content,
        files,
        subsystems: matched.into_iter().cloned().collect(),
    })
}

/// Files that must never reach a pack: env files, keys, tokens, local secrets, databases.
fn is_secret_path(path: &str) -> bool {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || name == "mise.local.toml"
        || name == ".npmrc"
        || name == ".netrc"
        || name == "credentials.json"
        || name == "google-services.json"
        || name.starts_with("secrets.")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || [
            ".key",
            ".token",
            ".pem",
            ".db",
            ".sqlite",
            ".sqlite3",
            ".p12",
            ".pfx",
            ".jks",
            ".keystore",
        ]
        .iter()
        .any(|ext| name.ends_with(ext))
}

/// Packs `files` (relative to `root`) of `subs` into one AI-ready context string,
/// optionally AST-compressed.
pub fn pack_files(
    root: &Path,
    subs: &[&SubsystemInfo],
    files: &[String],
    compress: bool,
    style: PackStyle,
) -> String {
    let mut req = PackRequest::new(root, "");
    req.mode = if compress {
        PackMode::Compressed
    } else {
        PackMode::Full
    };
    req.style = style;
    pack_files_with(root, subs, files, &req)
}

/// Request-driven variant of [`pack_files`]: mode, style and compression options come from
/// `req`; secrets, symlinks and paths escaping `root` are skipped.
pub fn pack_files_with(
    root: &Path,
    subs: &[&SubsystemInfo],
    files: &[String],
    req: &PackRequest<'_>,
) -> String {
    render_pack(root, subs, files, req, &[]).content
}

/// `trusted` paths (L1 cards resolved via `safe_join(knowledge_dir, ..)`) skip root checks.
fn render_pack(
    root: &Path,
    subs: &[&SubsystemInfo],
    files: &[String],
    req: &PackRequest<'_>,
    trusted: &[String],
) -> Rendered {
    let canonical_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let compress = req.mode != PackMode::Full;
    // Path safety and secret filtering apply to every mode (full, compressed, TOC, symbols).
    let read_raw = |file_rel: &str| -> Option<(String, String)> {
        if !trusted.iter().any(|t| t == file_rel)
            && !is_safe_source(root, &canonical_root, file_rel)
        {
            return None;
        }
        let full_p = root.join(file_rel);
        let content = std::fs::read_to_string(&full_p).ok()?;
        let ext = full_p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        Some((ext, content))
    };
    let mut rendered = Rendered::default();

    if req.mode == PackMode::Toc {
        let id = if subs.is_empty() {
            req.subsystem_id.to_string()
        } else {
            subs.iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut body = String::new();
        let mut packed = 0usize;
        for file_rel in files {
            let Some((ext, content)) = read_raw(file_rel) else {
                continue;
            };
            let entries = toc_entries(&content, &ext, req, &mut rendered);
            if !req.symbols.is_empty() && entries.is_empty() {
                continue;
            }
            body.push_str(&format!("{} ({}L)\n", file_rel, content.lines().count()));
            for e in entries {
                body.push_str(&e);
                body.push('\n');
            }
            packed += 1;
        }
        rendered.content = format!("[TOC] {id} ({packed} files)\n{body}");
        return rendered;
    }

    let mut entries: Vec<(&String, String, String)> = Vec::new();
    for file_rel in files {
        let Some((ext, content)) = read_raw(file_rel) else {
            continue;
        };
        let processed = if !req.symbols.is_empty() {
            match select_symbols(&content, &ext, req.symbols, &mut rendered) {
                Some(text) => text,
                None => continue,
            }
        } else if compress {
            compress_source_with(&content, &ext, &req.options)
        } else {
            content
        };
        entries.push((file_rel, ext, processed));
    }
    let style = req.style;
    let mut buffer = String::new();

    if style == PackStyle::Compact {
        for (file_rel, _, processed) in &entries {
            buffer.push_str(&format!(
                "=== {} ({}L) ===\n{}\n\n",
                file_rel,
                processed.lines().count(),
                processed
            ));
        }
    } else if style == PackStyle::Markdown {
        buffer.push_str("# 📦 AI Context Pack: ");
        let names: Vec<&str> = subs.iter().map(|s| s.name.as_str()).collect();
        buffer.push_str(&names.join(", "));
        buffer.push_str("\n\n> Сгенерировано context-init (встроенный AST-пакер)\n\n");

        for (file_rel, ext, processed) in &entries {
            buffer.push_str(&format!(
                "## File: `{}`\n```{}\n{}\n```\n\n",
                file_rel, ext, processed
            ));
        }
    } else {
        buffer.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        buffer.push_str("<context_pack generated_by=\"context-init\">\n");
        for sub in subs {
            buffer.push_str(&format!(
                "  <subsystem id=\"{}\" name=\"{}\">\n    <summary>{}</summary>\n  </subsystem>\n",
                xml_escape(&sub.id),
                xml_escape(&sub.name),
                xml_escape(&sub.summary)
            ));
        }
        buffer.push_str("  <files>\n");

        for (file_rel, _, processed) in &entries {
            buffer.push_str(&format!(
                "    <file path=\"{}\">\n<![CDATA[\n{}\n]]>\n    </file>\n",
                xml_escape(file_rel),
                processed.replace("]]>", "]]]]><![CDATA[>")
            ));
        }
        buffer.push_str("  </files>\n</context_pack>\n");
    }

    rendered.content = buffer;
    rendered
}

/// Result of [`render_pack`]: text plus symbol-selection bookkeeping.
#[derive(Default)]
struct Rendered {
    content: String,
    /// Spans selected by `PackRequest::symbols`.
    symbol_hits: usize,
    /// Names of every symbol seen (for "nothing matched" errors).
    available: Vec<String>,
}

const TOC_MAX_CHARS: usize = 160;

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

/// True when `query` (`name`, `Container.name` or `Container::name`) selects `spans[idx]`.
fn symbol_matches(
    query: &str,
    idx: usize,
    spans: &[crate::analysis::lang::SymbolSpan],
    names: &[String],
    ignore_case: bool,
) -> bool {
    let q = query.trim();
    let (container, name) = match q
        .rfind("::")
        .map(|p| (p, 2))
        .or_else(|| q.rfind('.').map(|p| (p, 1)))
    {
        Some((p, w)) => (Some(&q[..p]), &q[p + w..]),
        None => (None, q),
    };
    let eq = |a: &str, b: &str| {
        if ignore_case {
            a.to_lowercase() == b.to_lowercase()
        } else {
            a == b
        }
    };
    if name.is_empty() || !eq(&names[idx], name) {
        return false;
    }
    let Some(container) = container else {
        return true;
    };
    let last = container.rsplit(['.', ':']).next().unwrap_or(container);
    let me = &spans[idx];
    spans.iter().enumerate().any(|(j, s)| {
        j != idx
            && s.line_start <= me.line_start
            && me.line_end <= s.line_end
            && eq(&names[j], last)
    })
}

/// Spans of `content` with their names, selected by `req.symbols` when it is non-empty.
fn selected_spans(
    content: &str,
    ext: &str,
    symbols: &[String],
    rendered: &mut Rendered,
) -> Vec<(crate::analysis::lang::SymbolSpan, String, usize)> {
    use crate::analysis::lang::{adapter_for_ext, symbol_name};
    let Some(adapter) = adapter_for_ext(ext) else {
        return Vec::new();
    };
    let spans = adapter.extract_symbol_spans(content, ext);
    let names: Vec<String> = spans.iter().map(symbol_name).collect();
    for n in &names {
        if !n.is_empty() && !rendered.available.contains(n) {
            rendered.available.push(n.clone());
        }
    }
    let ignore_case = matches!(ext.to_ascii_lowercase().as_str(), "bsl" | "os");
    let mut out = Vec::new();
    for idx in 0..spans.len() {
        let depth = (0..idx)
            .filter(|&j| {
                spans[j].line_start <= spans[idx].line_start
                    && spans[idx].line_end <= spans[j].line_end
            })
            .count();
        if !symbols.is_empty()
            && !symbols
                .iter()
                .any(|q| symbol_matches(q, idx, &spans, &names, ignore_case))
        {
            continue;
        }
        out.push((spans[idx].clone(), names[idx].clone(), depth));
    }
    if !symbols.is_empty() {
        rendered.symbol_hits += out.len();
    }
    out
}

/// Exact source text of the spans selected by `symbols`, or `None` when nothing matched.
fn select_symbols(
    content: &str,
    ext: &str,
    symbols: &[String],
    rendered: &mut Rendered,
) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut picked: Vec<(usize, usize)> = Vec::new();
    let mut blocks = Vec::new();
    for (span, name, _) in selected_spans(content, ext, symbols, rendered) {
        if picked
            .iter()
            .any(|&(s, e)| s <= span.line_start && span.line_end <= e)
        {
            continue; // already inside a selected container
        }
        picked.push((span.line_start, span.line_end));
        let end = span.line_end.min(lines.len());
        let start = span.line_start.saturating_sub(1).min(end);
        blocks.push(format!(
            "[L{}-L{}] {}\n{}",
            span.line_start,
            span.line_end,
            name,
            lines[start..end].join("\n")
        ));
    }
    if blocks.is_empty() {
        None
    } else {
        Some(blocks.join("\n\n"))
    }
}

/// TOC lines of one file: symbol spans for code, headings / keys for known text formats.
fn toc_entries(
    content: &str,
    ext: &str,
    req: &PackRequest<'_>,
    rendered: &mut Rendered,
) -> Vec<String> {
    if crate::analysis::lang::adapter_for_ext(ext).is_some() {
        return selected_spans(content, ext, req.symbols, rendered)
            .into_iter()
            .filter(|(s, _, _)| !req.options.public_only || s.is_public)
            .map(|(s, _, depth)| {
                format!(
                    "{}[L{}-L{}] {}",
                    "  ".repeat(depth + 1),
                    s.line_start,
                    s.line_end,
                    truncate_chars(&s.signature, TOC_MAX_CHARS)
                )
            })
            .collect();
    }
    if !req.symbols.is_empty() {
        return Vec::new();
    }
    non_code_toc(content, ext)
        .into_iter()
        .map(|(line, text)| format!("  [L{line}] {}", truncate_chars(&text, TOC_MAX_CHARS)))
        .collect()
}

/// `(1-based line, label)` outline of non-code files: Markdown headings, TOML tables,
/// top-level YAML / JSON keys, SQL `CREATE`/`ALTER` statements, shell functions.
fn non_code_toc(content: &str, ext: &str) -> Vec<(usize, String)> {
    let lines = content.lines().enumerate().map(|(i, l)| (i + 1, l));
    match ext.to_ascii_lowercase().as_str() {
        "md" | "markdown" | "mdx" => {
            let mut fence = false;
            let mut out = Vec::new();
            for (n, l) in lines {
                let t = l.trim_start();
                if t.starts_with("```") || t.starts_with("~~~") {
                    fence = !fence;
                    continue;
                }
                let hashes = t.chars().take_while(|c| *c == '#').count();
                if !fence && (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
                    out.push((n, t.trim_end().to_string()));
                }
            }
            out
        }
        "toml" => lines
            .filter(|(_, l)| {
                let t = l.trim();
                t.starts_with('[') && t.ends_with(']')
            })
            .map(|(n, l)| (n, l.trim().to_string()))
            .collect(),
        "yaml" | "yml" => lines
            .filter(|(_, l)| {
                !l.starts_with([' ', '\t', '#', '-']) && l.contains(':') && !l.trim().is_empty()
            })
            .map(|(n, l)| (n, format!("{}:", l.split(':').next().unwrap_or("").trim())))
            .collect(),
        "json" => {
            let keys: Vec<(usize, usize, String)> = lines
                .filter_map(|(n, l)| {
                    let t = l.trim_start();
                    let rest = t.strip_prefix('"')?;
                    let end = rest.find('"')?;
                    rest[end + 1..]
                        .trim_start()
                        .starts_with(':')
                        .then(|| (n, l.len() - t.len(), format!("\"{}\"", &rest[..end])))
                })
                .collect();
            let min = keys.iter().map(|k| k.1).min().unwrap_or(0);
            keys.into_iter()
                .filter(|k| k.1 == min)
                .map(|(n, _, k)| (n, k))
                .collect()
        }
        "sql" => lines
            .filter(|(_, l)| {
                let u = l.trim_start().to_ascii_uppercase();
                u.starts_with("CREATE ") || u.starts_with("ALTER ")
            })
            .map(|(n, l)| (n, l.trim().to_string()))
            .collect(),
        "sh" | "bash" | "zsh" => lines
            .filter(|(_, l)| {
                let t = l.trim();
                t.starts_with("function ")
                    || (t.ends_with('{')
                        && t.split_once("()").is_some_and(|(name, _)| {
                            let name = name.trim();
                            !name.is_empty()
                                && name.chars().all(|c| {
                                    c.is_alphanumeric() || c == '_' || c == '-' || c == ':'
                                })
                        }))
            })
            .map(|(n, l)| (n, l.trim().trim_end_matches('{').trim().to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Compresses source text through the language adapter of `ext` (D7: Rust via AST, BSL by
/// signatures, ...); unknown languages get whitespace normalization.
pub fn compress_source(content: &str, ext: &str) -> String {
    match crate::analysis::lang::adapter_for_ext(ext) {
        Some(adapter) => adapter.compress_source(content),
        None => compress_code(content),
    }
}

pub fn compress_rust_ast(content: &str) -> Option<String> {
    compress_rust_ast_with(content, &CompressOptions::legacy())
}

/// [`compress_rust_ast`] with options: `use` items collapse into `// [N use omitted]`,
/// `#[cfg(test)]` / `#[test]` items are dropped (`exclude_tests`), non-`pub` items are
/// dropped (`public_only`), doc comments are removed (`strip_comments`) and bodies become
/// `{ ⋮ }` (`inline_elide`).
pub fn compress_rust_ast_with(content: &str, opts: &CompressOptions) -> Option<String> {
    let mut parsed = syn::parse_file(content).ok()?;
    if opts.strip_comments {
        strip_doc_attrs(&mut parsed.attrs);
    }
    let mut uses = 0usize;
    prune_rust_items(&mut parsed.items, opts, &mut uses);
    let mut file_attrs = Vec::new();
    for attr in &parsed.attrs {
        file_attrs.push(normalize_rust_tokens(&quote::quote!(#attr).to_string()));
    }
    let body = format_rust_items_opts(&parsed.items, opts.inline_elide);
    let mut parts = Vec::new();
    if !file_attrs.is_empty() {
        parts.push(file_attrs.join("\n"));
    }
    if uses > 0 {
        parts.push(format!("// [{uses} use omitted]"));
    }
    parts.push(body);
    Some(parts.join("\n\n"))
}

fn strip_doc_attrs(attrs: &mut Vec<syn::Attribute>) {
    attrs.retain(|a| !a.path().is_ident("doc"));
}

fn is_test_attrs(attrs: &[syn::Attribute]) -> bool {
    crate::analysis::detector::has_cfg_test(attrs)
        || attrs.iter().any(|a| a.path().is_ident("test"))
}

fn rust_item_attrs(item: &mut syn::Item) -> Option<&mut Vec<syn::Attribute>> {
    use syn::Item;
    Some(match item {
        Item::Const(x) => &mut x.attrs,
        Item::Enum(x) => &mut x.attrs,
        Item::ExternCrate(x) => &mut x.attrs,
        Item::Fn(x) => &mut x.attrs,
        Item::ForeignMod(x) => &mut x.attrs,
        Item::Impl(x) => &mut x.attrs,
        Item::Macro(x) => &mut x.attrs,
        Item::Mod(x) => &mut x.attrs,
        Item::Static(x) => &mut x.attrs,
        Item::Struct(x) => &mut x.attrs,
        Item::Trait(x) => &mut x.attrs,
        Item::TraitAlias(x) => &mut x.attrs,
        Item::Type(x) => &mut x.attrs,
        Item::Union(x) => &mut x.attrs,
        Item::Use(x) => &mut x.attrs,
        _ => return None,
    })
}

fn rust_item_is_public(item: &syn::Item) -> bool {
    use crate::analysis::detector::is_syn_pub;
    use syn::Item;
    match item {
        Item::Const(x) => is_syn_pub(&x.vis),
        Item::Enum(x) => is_syn_pub(&x.vis),
        Item::ExternCrate(x) => is_syn_pub(&x.vis),
        Item::Fn(x) => is_syn_pub(&x.vis),
        Item::Mod(x) => is_syn_pub(&x.vis),
        Item::Static(x) => is_syn_pub(&x.vis),
        Item::Struct(x) => is_syn_pub(&x.vis),
        Item::Trait(x) => is_syn_pub(&x.vis),
        Item::TraitAlias(x) => is_syn_pub(&x.vis),
        Item::Type(x) => is_syn_pub(&x.vis),
        Item::Union(x) => is_syn_pub(&x.vis),
        Item::Use(x) => is_syn_pub(&x.vis),
        Item::Macro(x) => x.attrs.iter().any(|a| a.path().is_ident("macro_export")),
        _ => true,
    }
}

/// Applies `strip_imports`, `exclude_tests`, `public_only` and `strip_comments` to an item
/// list in place (recursing into inline modules, impls and traits).
fn prune_rust_items(items: &mut Vec<syn::Item>, opts: &CompressOptions, uses: &mut usize) {
    use crate::analysis::detector::is_syn_pub;
    use syn::{ImplItem, Item, TraitItem};
    items.retain_mut(|item| {
        if opts.strip_imports && matches!(item, Item::Use(_)) {
            *uses += 1;
            return false;
        }
        if opts.exclude_tests && rust_item_attrs(item).is_some_and(|a| is_test_attrs(a)) {
            return false;
        }
        if opts.public_only && !rust_item_is_public(item) {
            return false;
        }
        if opts.strip_comments {
            if let Some(attrs) = rust_item_attrs(item) {
                strip_doc_attrs(attrs);
            }
        }
        match item {
            Item::Mod(m) => {
                if let Some((_, inner)) = &mut m.content {
                    prune_rust_items(inner, opts, uses);
                }
            }
            Item::Impl(imp) => {
                let inherent = imp.trait_.is_none();
                imp.items.retain_mut(|it| {
                    let (attrs, public) = match it {
                        ImplItem::Fn(f) => (&mut f.attrs, is_syn_pub(&f.vis)),
                        ImplItem::Const(c) => (&mut c.attrs, is_syn_pub(&c.vis)),
                        ImplItem::Type(t) => (&mut t.attrs, is_syn_pub(&t.vis)),
                        _ => return true,
                    };
                    if opts.exclude_tests && is_test_attrs(attrs) {
                        return false;
                    }
                    if opts.public_only && inherent && !public {
                        return false;
                    }
                    if opts.strip_comments {
                        strip_doc_attrs(attrs);
                    }
                    true
                });
                if opts.public_only && inherent && imp.items.is_empty() {
                    return false;
                }
            }
            Item::Trait(t) if opts.strip_comments => {
                for it in &mut t.items {
                    match it {
                        TraitItem::Fn(f) => strip_doc_attrs(&mut f.attrs),
                        TraitItem::Const(c) => strip_doc_attrs(&mut c.attrs),
                        TraitItem::Type(ty) => strip_doc_attrs(&mut ty.attrs),
                        _ => {}
                    }
                }
            }
            Item::Struct(s) if opts.strip_comments => {
                for f in s.fields.iter_mut() {
                    strip_doc_attrs(&mut f.attrs);
                }
            }
            Item::Enum(e) if opts.strip_comments => {
                for v in e.variants.iter_mut() {
                    strip_doc_attrs(&mut v.attrs);
                    for f in v.fields.iter_mut() {
                        strip_doc_attrs(&mut f.attrs);
                    }
                }
            }
            _ => {}
        }
        true
    });
}

pub fn format_rust_items(items: &[syn::Item]) -> String {
    format_rust_items_opts(items, false)
}

/// Elided body of a function signature: multi-line `⋮----` block or inline `{ ⋮ }`.
fn elided_fn(indent: &str, sig: &str, inline: bool) -> String {
    use crate::analysis::lang::{ELIDED_BODY, INLINE_ELIDED};
    if inline {
        format!("{indent}{sig} {{ {INLINE_ELIDED} }}")
    } else {
        format!("{indent}{sig} {{\n{indent}    {ELIDED_BODY}\n{indent}}}")
    }
}

fn format_rust_items_opts(items: &[syn::Item], inline: bool) -> String {
    let member_sep = if inline { "\n" } else { "\n\n" };
    let mut formatted_items = Vec::new();
    for item in items {
        match item {
            syn::Item::Fn(f) => {
                let attrs = &f.attrs;
                let vis = &f.vis;
                let sig = &f.sig;
                let sig_str = quote::quote!(#vis #sig).to_string();
                let clean_sig = normalize_rust_tokens(&sig_str);
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let body = elided_fn("", &clean_sig, inline);
                if attr_strs.is_empty() {
                    formatted_items.push(body);
                } else {
                    formatted_items.push(format!("{}\n{}", attr_strs.join("\n"), body));
                }
            }
            syn::Item::Impl(imp) => {
                let attrs = &imp.attrs;
                let defaultness = &imp.defaultness;
                let unsafety = &imp.unsafety;
                let generics = &imp.generics;
                let (impl_generics, _, where_clause) = generics.split_for_impl();
                let self_ty = &imp.self_ty;
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let header_raw = if let Some((bang, path, for_tok)) = &imp.trait_ {
                    quote::quote!(#defaultness #unsafety impl #impl_generics #bang #path #for_tok #self_ty #where_clause).to_string()
                } else {
                    quote::quote!(#defaultness #unsafety impl #impl_generics #self_ty #where_clause)
                        .to_string()
                };
                let clean_header_core = normalize_rust_tokens(&header_raw);
                let clean_header = if attr_strs.is_empty() {
                    clean_header_core
                } else {
                    format!("{}\n{}", attr_strs.join("\n"), clean_header_core)
                };
                let mut item_strs = Vec::new();
                for it in &imp.items {
                    if let syn::ImplItem::Fn(m) = it {
                        let m_attrs: Vec<String> = m
                            .attrs
                            .iter()
                            .map(|a| {
                                format!(
                                    "    {}",
                                    normalize_rust_tokens(&quote::quote!(#a).to_string())
                                )
                            })
                            .collect();
                        let vis = &m.vis;
                        let defaultness = &m.defaultness;
                        let sig = &m.sig;
                        let sig_str = quote::quote!(#vis #defaultness #sig).to_string();
                        let clean_sig = normalize_rust_tokens(&sig_str);
                        let body = elided_fn("    ", &clean_sig, inline);
                        if m_attrs.is_empty() {
                            item_strs.push(body);
                        } else {
                            item_strs.push(format!("{}\n{}", m_attrs.join("\n"), body));
                        }
                    } else {
                        let other_str = quote::quote!(#it).to_string();
                        item_strs.push(format!("    {}", normalize_rust_tokens(&other_str)));
                    }
                }
                if item_strs.is_empty() {
                    formatted_items.push(format!("{} {{}}", clean_header));
                } else {
                    formatted_items.push(format!(
                        "{} {{\n{}\n}}",
                        clean_header,
                        item_strs.join(member_sep)
                    ));
                }
            }
            syn::Item::Trait(t) => {
                let attrs = &t.attrs;
                let vis = &t.vis;
                let ident = &t.ident;
                let generics = &t.generics;
                let (impl_generics, _, where_clause) = generics.split_for_impl();
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let header_raw =
                    quote::quote!(#vis trait #ident #impl_generics #where_clause).to_string();
                let clean_header_core = normalize_rust_tokens(&header_raw);
                let clean_header = if attr_strs.is_empty() {
                    clean_header_core
                } else {
                    format!("{}\n{}", attr_strs.join("\n"), clean_header_core)
                };
                let mut trait_items = Vec::new();
                for it in &t.items {
                    if let syn::TraitItem::Fn(tf) = it {
                        let tf_attrs: Vec<String> = tf
                            .attrs
                            .iter()
                            .map(|a| {
                                format!(
                                    "    {}",
                                    normalize_rust_tokens(&quote::quote!(#a).to_string())
                                )
                            })
                            .collect();
                        let sig = &tf.sig;
                        let sig_str = quote::quote!(#sig).to_string();
                        let clean_sig = normalize_rust_tokens(&sig_str);
                        let body = if tf.default.is_some() {
                            elided_fn("    ", &clean_sig, inline)
                        } else {
                            format!("    {};", clean_sig)
                        };
                        if tf_attrs.is_empty() {
                            trait_items.push(body);
                        } else {
                            trait_items.push(format!("{}\n{}", tf_attrs.join("\n"), body));
                        }
                    } else {
                        let other_str = quote::quote!(#it).to_string();
                        trait_items.push(format!("    {}", normalize_rust_tokens(&other_str)));
                    }
                }
                if trait_items.is_empty() {
                    formatted_items.push(format!("{} {{}}", clean_header));
                } else {
                    formatted_items.push(format!(
                        "{} {{\n{}\n}}",
                        clean_header,
                        trait_items.join(member_sep)
                    ));
                }
            }
            syn::Item::Mod(m) => {
                let attrs = &m.attrs;
                let vis = &m.vis;
                let ident = &m.ident;
                let attr_strs: Vec<String> = attrs
                    .iter()
                    .map(|a| normalize_rust_tokens(&quote::quote!(#a).to_string()))
                    .collect();
                let mod_header_raw = quote::quote!(#vis mod #ident).to_string();
                let clean_header_core = normalize_rust_tokens(&mod_header_raw);
                let clean_header = if attr_strs.is_empty() {
                    clean_header_core
                } else {
                    format!("{}\n{}", attr_strs.join("\n"), clean_header_core)
                };
                if let Some((_, inner_items)) = &m.content {
                    let inner = format_rust_items_opts(inner_items, inline);
                    let indented: Vec<String> = inner
                        .lines()
                        .map(|l| {
                            if l.is_empty() {
                                String::new()
                            } else {
                                format!("    {}", l)
                            }
                        })
                        .collect();
                    formatted_items.push(format!(
                        "{} {{\n{}\n}}",
                        clean_header,
                        indented.join("\n")
                    ));
                } else {
                    formatted_items.push(format!("{};", clean_header));
                }
            }
            other => {
                let s = quote::quote!(#other).to_string();
                formatted_items.push(normalize_rust_tokens(&s));
            }
        }
    }
    formatted_items.join("\n\n")
}

pub fn normalize_rust_tokens(s: &str) -> String {
    let s = s
        .replace("# ! [ ", "#![")
        .replace("# ! [", "#![")
        .replace("#![ ", "#![")
        .replace("# ! ", "#!")
        .replace("# !", "#!")
        .replace("# [ ", "#[")
        .replace("# [", "#[")
        .replace("#[ ", "#[")
        .replace(" < ", "<")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace(" > ", "> ")
        .replace(" ( ", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" : ", ": ")
        .replace(" , ", ", ")
        .replace(" ;", ";")
        .replace(" :: ", "::")
        .replace("( &", "(&")
        .replace("(& ", "(&")
        .replace("& mut ", "&mut ")
        .replace("& '", "&'")
        .replace("& ", "&");

    let words: Vec<&str> = s.split_whitespace().collect();
    let joined = words.join(" ");
    joined
        .replace(" (", "(")
        .replace(":: ", "::")
        .replace(" ::", "::")
        .replace(":(", ": (")
        .replace("->(", "-> (")
        .replace("+(", "+ (")
}

pub fn compress_code(content: &str) -> String {
    let mut result = Vec::new();
    let mut prev_was_empty = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_was_empty {
                result.push("");
                prev_was_empty = true;
            }
        } else {
            prev_was_empty = false;
            result.push(line.trim_end());
        }
    }

    result.join("\n")
}

#[cfg(test)]
mod packer_tests {
    use super::*;

    #[test]
    fn test_compress_code() {
        let raw = "fn hello() {\n\n\n    println!(\"hi\");   \n\n\n}\n";
        let compressed = compress_code(raw);
        assert_eq!(compressed, "fn hello() {\n\n    println!(\"hi\");\n\n}");
    }

    #[test]
    fn test_compress_rust_ast() {
        let code = r#"
use std::collections::HashMap;

pub struct Service {
    pub name: String,
}

impl Service {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

pub fn run() {
    println!("running");
}
"#;
        let compressed = compress_source(code, "rs");
        assert!(compressed.contains("pub fn run() {\n    ⋮----\n}"));
        assert!(compressed.contains("pub fn new(name: String) -> Self {\n        ⋮----\n    }"));
        assert!(compressed.contains("pub struct Service { pub name: String, }"));
    }

    #[test]
    fn test_compress_rust_ast_with_attributes_and_references() {
        let code = r#"
#![allow(unused)]

#[inline]
pub fn process(text: &str) -> &str {
    text
}

impl Service {
    #[inline]
    pub fn handle(&self, query: &str) -> bool {
        query.is_empty()
    }
}

#[cfg(test)]
mod tests {
    pub fn internal() {
        println!("test");
    }
}
"#;
        let compressed = compress_source(code, "rs");
        assert!(compressed.contains("#![allow(unused)]"));
        assert!(
            compressed.contains("#[inline]\npub fn process(text: &str) -> &str {\n    ⋮----\n}")
        );
        assert!(compressed.contains(
            "#[inline]\n    pub fn handle(&self, query: &str) -> bool {\n        ⋮----\n    }"
        ));
        assert!(compressed.contains(
            "#[cfg(test)]\nmod tests {\n    pub fn internal() {\n        ⋮----\n    }\n}"
        ));
    }

    #[test]
    fn test_pack_fallback_xml_and_md() {
        let temp_dir = std::env::temp_dir().join(format!("test_pack_sub_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("lib.rs");
        std::fs::write(&test_file, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        let sub = SubsystemInfo {
            id: "core".to_string(),
            name: "Core".to_string(),
            summary: "Core logic".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec!["core".to_string()],
            entrypoints: vec!["lib.rs".to_string()],
            key_files: vec!["lib.rs".to_string()],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 10,
            parent: None,
            summary_source: crate::storage::SummarySource::Heuristic,
            summary_model: None,
            needs_review: false,
        };

        let files = vec!["lib.rs".to_string()];
        let out_xml = temp_dir.join("pack.xml");
        let out_md = temp_dir.join("pack.md");

        let xml_str = pack_files(&temp_dir, &[&sub], &files, true, PackStyle::parse("xml"));
        std::fs::write(&out_xml, &xml_str).unwrap();
        assert!(out_xml.exists());
        let xml_content = std::fs::read_to_string(&out_xml).unwrap();
        assert!(xml_content.contains("<context_pack"));
        assert!(xml_content.contains("lib.rs"));
        assert!(xml_content.contains("⋮----"));

        let md_str = pack_files(
            &temp_dir,
            &[&sub],
            &files,
            true,
            PackStyle::parse("markdown"),
        );
        std::fs::write(&out_md, &md_str).unwrap();
        assert!(out_md.exists());
        let md_content = std::fs::read_to_string(&out_md).unwrap();
        assert!(md_content.contains("# 📦 AI Context Pack:"));
        assert!(md_content.contains("## File: `lib.rs`"));
        assert!(md_content.contains("⋮----"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pack_builtin_no_compress() {
        let temp_dir = std::env::temp_dir().join(format!("test_pack_nc_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("lib.rs");
        std::fs::write(&test_file, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        let sub = SubsystemInfo {
            id: "core".to_string(),
            name: "Core".to_string(),
            summary: "Core logic".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec!["core".to_string()],
            entrypoints: vec!["lib.rs".to_string()],
            key_files: vec!["lib.rs".to_string()],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 10,
            parent: None,
            summary_source: crate::storage::SummarySource::Heuristic,
            summary_model: None,
            needs_review: false,
        };

        let files = vec!["lib.rs".to_string()];
        let out_xml = temp_dir.join("pack_nc.xml");
        let out_md = temp_dir.join("pack_nc.md");

        let xml_str = pack_files(&temp_dir, &[&sub], &files, false, PackStyle::parse("xml"));
        std::fs::write(&out_xml, &xml_str).unwrap();
        let xml_content = std::fs::read_to_string(&out_xml).unwrap();
        assert!(xml_content.contains("pub fn add(a: i32, b: i32) -> i32 { a + b }"));
        assert!(!xml_content.contains("⋮----"));

        let md_str = pack_files(
            &temp_dir,
            &[&sub],
            &files,
            false,
            PackStyle::parse("markdown"),
        );
        std::fs::write(&out_md, &md_str).unwrap();
        let md_content = std::fs::read_to_string(&out_md).unwrap();
        assert!(md_content.contains("pub fn add(a: i32, b: i32) -> i32 { a + b }"));
        assert!(!md_content.contains("⋮----"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pack_builtin_non_rust_files() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir =
            std::env::temp_dir().join(format!("test_pack_poly_{}_{}", std::process::id(), nanos));
        let _ = std::fs::create_dir_all(&temp_dir);

        let py_code = "def calculate(a, b):\n    # Indented block\n    result = a + b    \n\n\n    return result\n";
        let py_file = temp_dir.join("main.py");
        std::fs::write(&py_file, py_code).unwrap();

        let sub = SubsystemInfo {
            id: "scripts".to_string(),
            name: "Scripts".to_string(),
            summary: "Python script module".to_string(),
            l1_path: "subsystems/scripts.md".to_string(),
            tags: vec!["scripts".to_string()],
            entrypoints: vec!["main.py".to_string()],
            key_files: vec!["main.py".to_string()],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 15,
            parent: None,
            summary_source: crate::storage::SummarySource::Heuristic,
            summary_model: None,
            needs_review: false,
        };

        let files = vec!["main.py".to_string()];
        let out_nc = temp_dir.join("pack_nc.xml");
        let out_comp = temp_dir.join("pack_comp.xml");

        // With no-compress: content is 100% identical including extra blank lines and trailing whitespace
        let nc_str = pack_files(&temp_dir, &[&sub], &files, false, PackStyle::parse("xml"));
        std::fs::write(&out_nc, &nc_str).unwrap();
        assert!(nc_str.contains("    result = a + b    \n\n\n    return result"));

        // With compress: bodies are elided with ELIDED_BODY
        let comp_str = pack_files(&temp_dir, &[&sub], &files, true, PackStyle::parse("xml"));
        std::fs::write(&out_comp, &comp_str).unwrap();
        assert!(comp_str.contains(crate::analysis::lang::ELIDED_BODY));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[cfg(test)]
mod pack_subsystem_tests {
    use super::*;
    use crate::storage::SubsystemFile;
    use std::fs;

    fn tsub(id: &str, files: &[&str]) -> SubsystemInfo {
        SubsystemInfo {
            id: id.to_string(),
            name: id.to_string(),
            summary: "s".to_string(),
            l1_path: format!("subsystems/{id}.md"),
            tags: vec![],
            entrypoints: vec![],
            key_files: vec![],
            files: files
                .iter()
                .map(|p| SubsystemFile {
                    path: (*p).to_string(),
                    is_entrypoint: false,
                    lines: 1,
                })
                .collect(),
            key_symbols: vec![],
            test_command: None,
            tokens: 0,
            parent: None,
            summary_source: crate::storage::SummarySource::Heuristic,
            summary_model: None,
            needs_review: false,
        }
    }

    fn tdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ci_pack_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn test_pack_filters_by_glob_and_short_filename() {
        let root = tdir("glob");
        fs::create_dir_all(root.join("src/nested")).unwrap();
        for p in ["src/a.rs", "src/nested/target.rs", "src/b.rs"] {
            fs::write(root.join(p), format!("pub fn f_{}() {{}}", p.len())).unwrap();
        }
        let index = L0Index {
            subsystems: vec![tsub("s", &["src/a.rs", "src/nested/target.rs", "src/b.rs"])],
            ..L0Index::default()
        };
        let pats = vec!["target.rs".to_string()];
        let mut req = PackRequest::new(&root, "s");
        req.files = &pats;
        let out = pack_subsystem_with(&root, &root.join("kb"), &index, &req).unwrap();
        assert_eq!(out.files, vec!["src/nested/target.rs".to_string()]);

        let globs = vec!["src/*.rs".to_string()];
        req.files = &globs;
        let out = pack_subsystem_with(&root, &root.join("kb"), &index, &req).unwrap();
        assert!(out.files.contains(&"src/a.rs".to_string()));
        assert!(out.files.contains(&"src/b.rs".to_string()));

        let missing = vec!["missing.rs".to_string()];
        req.files = &missing;
        let err = pack_subsystem_with(&root, &root.join("kb"), &index, &req).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("src/a.rs") && msg.contains("src/nested/target.rs"),
            "{msg}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_pack_rejects_symlinks_backslashes_and_allows_secret_named_code() {
        let root = tdir("sym");
        fs::create_dir_all(root.join("src")).unwrap();
        for p in [
            "src/CredentialsProvider.kt",
            "src/secret_rotation.rs",
            "credentials.json",
            "secrets.yaml",
            ".npmrc",
        ] {
            fs::write(root.join(p), "class X").unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/hosts", root.join("src/link.rs")).unwrap();
        let all = [
            "src/CredentialsProvider.kt",
            "src/secret_rotation.rs",
            "credentials.json",
            "secrets.yaml",
            ".npmrc",
            "src/link.rs",
        ];
        let index = L0Index {
            subsystems: vec![tsub("s", &all)],
            ..L0Index::default()
        };
        let out = pack_subsystem_with(
            &root,
            &root.join("kb"),
            &index,
            &PackRequest::new(&root, "s"),
        )
        .unwrap();
        assert!(out
            .files
            .contains(&"src/CredentialsProvider.kt".to_string()));
        assert!(out.files.contains(&"src/secret_rotation.rs".to_string()));
        for bad in ["credentials.json", "secrets.yaml", ".npmrc", "src/link.rs"] {
            assert!(!out.files.contains(&bad.to_string()), "{bad}");
        }
        assert!(!out.content.contains("localhost"), "{}", out.content);

        let direct: Vec<String> = all.iter().map(|s| s.to_string()).collect();
        let sub = tsub("s", &all);
        let text = pack_files_with(&root, &[&sub], &direct, &PackRequest::new(&root, "s"));
        assert!(!text.contains("link.rs"), "{text}");
        assert!(!text.contains("secrets.yaml"), "{text}");

        for pat in [
            "..\\foo.env",
            "/etc/passwd",
            "C:\\foo.rs",
            "[invalid",
            "",
            "a/../b.rs",
            "a\0b",
        ] {
            let pats = vec![pat.to_string()];
            let mut req = PackRequest::new(&root, "s");
            req.files = &pats;
            assert!(
                pack_subsystem_with(&root, &root.join("kb"), &index, &req).is_err(),
                "{pat:?}"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn secrets_and_databases_are_never_packed() {
        for p in [
            ".env",
            "cfg/.env.local",
            "mise.local.toml",
            "a\\b\\id.KEY",
            "x/api.token",
            "data/app.sqlite",
            "d.db",
        ] {
            assert!(is_secret_path(p), "{p}");
        }
        for p in ["src/lib.rs", "environment.rs", "keys.rs", "README.md"] {
            assert!(!is_secret_path(p), "{p}");
        }
    }

    #[test]
    fn pack_subsystem_falls_back_to_l1_card_without_files() {
        let root = std::env::temp_dir().join(format!(
            "ci_pack_card_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let kd = root.join("kb");
        fs::create_dir_all(kd.join("subsystems")).unwrap();
        fs::write(kd.join("subsystems/wm.md"), "# Window manager card\n").unwrap();
        let index = L0Index {
            subsystems: vec![SubsystemInfo {
                id: "wm".to_string(),
                name: "WM".to_string(),
                summary: "s".to_string(),
                l1_path: "subsystems/wm.md".to_string(),
                tags: vec![],
                entrypoints: vec![],
                key_files: vec![],
                files: vec![],
                key_symbols: vec![],
                test_command: None,
                tokens: 0,
                parent: None,
                summary_source: crate::storage::SummarySource::Heuristic,
                summary_model: None,
                needs_review: false,
            }],
            ..L0Index::default()
        };
        let packed = pack_subsystem(&root, &kd, &index, "wm", true, PackStyle::Xml).unwrap();
        assert!(
            packed.content.contains("Window manager card"),
            "{}",
            packed.content
        );
        assert_eq!(packed.files, vec!["kb/subsystems/wm.md".to_string()]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pack_subsystem_ignores_paths_outside_root() {
        let base = std::env::temp_dir().join(format!(
            "ci_pack_escape_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = base.join("repo");
        let kd = root.join("kb");
        fs::create_dir_all(kd.join("subsystems")).unwrap();
        let secret = base.join("secret.txt");
        fs::write(&secret, "TOP-SECRET").unwrap();
        let sub = |id: &str, files: Vec<SubsystemFile>, key_files: Vec<String>, l1: String| {
            SubsystemInfo {
                id: id.to_string(),
                name: id.to_string(),
                summary: "s".to_string(),
                l1_path: l1,
                tags: vec![],
                entrypoints: vec![],
                key_files,
                files,
                key_symbols: vec![],
                test_command: None,
                tokens: 0,
                parent: None,
                summary_source: crate::storage::SummarySource::Heuristic,
                summary_model: None,
                needs_review: false,
            }
        };
        let index = L0Index {
            subsystems: vec![
                sub(
                    "a",
                    vec![SubsystemFile {
                        path: "../secret.txt".to_string(),
                        is_entrypoint: false,
                        lines: 1,
                    }],
                    vec![secret.to_string_lossy().into_owned()],
                    "subsystems/a.md".to_string(),
                ),
                sub("b", vec![], vec![], secret.to_string_lossy().into_owned()),
                sub("c", vec![], vec![], "../../secret.txt".to_string()),
            ],
            ..L0Index::default()
        };
        for id in ["a", "b", "c", "all"] {
            match pack_subsystem(&root, &kd, &index, id, false, PackStyle::Xml) {
                Ok(p) => assert!(!p.content.contains("TOP-SECRET"), "{id}: {}", p.content),
                Err(e) => assert!(!format!("{e:#}").contains("TOP-SECRET")),
            }
        }
        let _ = fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;

    fn make_sub(name: &str) -> SubsystemInfo {
        SubsystemInfo {
            id: "core".to_string(),
            name: name.to_string(),
            summary: "Core logic".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec![],
            entrypoints: vec![],
            key_files: vec![],
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 10,
            parent: None,
            summary_source: crate::storage::SummarySource::Heuristic,
            summary_model: None,
            needs_review: false,
        }
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ci_packer_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn test_pack_style_try_parse_rejects_unknown() {
        assert_eq!(PackStyle::try_parse("compact").unwrap(), PackStyle::Compact);
        assert!(PackStyle::try_parse("YAML").is_err());
    }

    #[test]
    fn test_pack_compact_format_has_no_xml_or_markdown_banners() {
        let dir = tmp("compact");
        std::fs::write(dir.join("lib.rs"), "pub fn add() {}").unwrap();
        let sub = make_sub("Core");
        let files = vec!["lib.rs".to_string()];
        let out = pack_files(&dir, &[&sub], &files, false, PackStyle::Compact);
        assert!(out.contains("=== lib.rs (1L) ==="), "{out}");
        assert!(!out.contains("<?xml"));
        assert!(!out.contains("<![CDATA["));
        assert!(!out.contains("# 📦 AI Context Pack"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_pack_xml_escapes_cdata_terminator_and_attributes() {
        let dir = tmp("xmlesc");
        let name = "a&\"b<c.rs";
        std::fs::write(dir.join(name), "let s = \"]]>\";").unwrap();
        let sub = make_sub("Co&re<\"");
        let files = vec![name.to_string()];
        let out = pack_files(&dir, &[&sub], &files, false, PackStyle::Xml);
        let mut reader = quick_xml::Reader::from_str(&out);
        let mut text = String::new();
        loop {
            match reader.read_event().unwrap_or_else(|e| panic!("{e}: {out}")) {
                quick_xml::events::Event::Eof => break,
                quick_xml::events::Event::CData(c) => {
                    text.push_str(&String::from_utf8_lossy(&c.into_inner()))
                }
                _ => {}
            }
        }
        assert!(text.contains("\"]]>\""), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod compress_with_tests {
    use super::*;
    use crate::analysis::detector::count_tokens;

    fn tdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ci_cw_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn rust_fixture() -> String {
        let mut code = String::from(
            "//! Crate docs.\nuse std::collections::HashMap;\nuse std::fmt;\nuse std::io;\nuse std::path::Path;\nuse std::sync::Arc;\nuse anyhow::Result;\n\n/// A service.\npub struct Service {\n    /// Name.\n    pub name: String,\n}\n\nimpl Service {\n    /// Runs it.\n    pub fn run(&self) -> usize {\n        self.name.len()\n    }\n\n    fn private_helper(&self) -> bool {\n        true\n    }\n}\n\nfn private_free() {}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n",
        );
        for i in 0..8 {
            code.push_str(&format!(
                "\n    #[test]\n    fn t{i}() {{\n        let s = Service {{ name: \"x{i}\".into() }};\n        assert_eq!(s.run(), 2);\n    }}\n"
            ));
        }
        code.push_str("}\n");
        code
    }

    #[test]
    fn test_rust_compress_with_compact_strips_tests_imports_and_inlines_elide() {
        let code = rust_fixture();
        assert_eq!(
            compress_source_with(&code, "rs", &CompressOptions::legacy()),
            compress_source(&code, "rs")
        );
        let c = compress_source_with(&code, "rs", &CompressOptions::compact());
        assert!(c.contains("// [6 use omitted]"), "{c}");
        assert!(!c.contains("use std"), "{c}");
        assert!(!c.contains("#[cfg(test)]"), "{c}");
        assert!(!c.contains("fn t0"), "{c}");
        assert!(!c.contains("doc ="), "{c}");
        assert!(c.contains("pub fn run(&self) -> usize { ⋮ }"), "{c}");
        assert!(c.contains("fn private_helper(&self) -> bool { ⋮ }"), "{c}");

        let mut po = CompressOptions::compact();
        po.public_only = true;
        let p = compress_source_with(&code, "rs", &po);
        assert!(
            !p.contains("private_helper") && !p.contains("private_free"),
            "{p}"
        );
        assert!(p.contains("pub fn run"), "{p}");

        // Compact style + compact options vs XML + legacy compression: >= 40% fewer tokens.
        let root = tdir("save");
        std::fs::write(root.join("lib.rs"), &code).unwrap();
        let files = vec!["lib.rs".to_string()];
        let xml = pack_files(&root, &[], &files, true, PackStyle::Xml);
        let mut req = PackRequest::new(&root, "core");
        req.options = CompressOptions::compact();
        let compact = pack_files_with(&root, &[], &files, &req);
        let (before, after) = (count_tokens(&xml), count_tokens(&compact));
        let saved = (before - after) as f64 * 100.0 / before as f64;
        assert!(
            saved >= 40.0,
            "saved {saved:.1}% ({before} -> {after})\n{compact}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_pack_symbols_extracts_exact_method_bodies() {
        let root = tdir("sym");
        let rs = "pub struct Service;\n\nimpl Service {\n    #[inline]\n    pub fn handle(&self, query: &str) -> bool {\n        // keep   this    comment\n        query.is_empty()\n    }\n\n    pub fn other(&self) {\n        println!(\"other body\");\n    }\n}\n";
        let kt = "class Repo {\n    fun load() {\n        val s = \"{\"\n        println(\"load body\")\n    }\n\n    fun save() {\n        println(\"save body\")\n    }\n}\n";
        std::fs::write(root.join("lib.rs"), rs).unwrap();
        std::fs::write(root.join("Repo.kt"), kt).unwrap();
        let files = vec!["lib.rs".to_string(), "Repo.kt".to_string()];
        let symbols = vec!["handle".to_string(), "Repo.load".to_string()];
        let mut req = PackRequest::new(&root, "core");
        req.symbols = &symbols;
        let out = pack_files_with(&root, &[], &files, &req);
        assert!(
            out.contains("    pub fn handle(&self, query: &str) -> bool {\n        // keep   this    comment\n        query.is_empty()\n    }"),
            "{out}"
        );
        assert!(out.contains("println(\"load body\")"), "{out}");
        assert!(!out.contains("other body"), "{out}");
        assert!(!out.contains("save body"), "{out}");

        let sub = SubsystemInfo {
            id: "core".to_string(),
            name: "core".to_string(),
            summary: "s".to_string(),
            l1_path: "subsystems/core.md".to_string(),
            tags: vec![],
            entrypoints: vec![],
            key_files: files.clone(),
            files: vec![],
            key_symbols: vec![],
            test_command: None,
            tokens: 0,
            parent: None,
            summary_source: crate::storage::SummarySource::Heuristic,
            summary_model: None,
            needs_review: false,
        };
        let index = L0Index {
            subsystems: vec![sub],
            ..L0Index::default()
        };
        let ok = pack_subsystem_with(&root, &root.join("kb"), &index, &req).unwrap();
        assert!(ok.content.contains("query.is_empty()"), "{}", ok.content);
        let missing = vec!["nope".to_string()];
        req.symbols = &missing;
        let err = pack_subsystem_with(&root, &root.join("kb"), &index, &req).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("handle") && msg.contains("load"), "{msg}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
