//! Response token budget enforcement, opaque cursor pagination, and truncation footer (D19, R1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use ci_core::tokens::count_tokens;

/// Structured opaque cursor payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CursorData {
    /// Byte offset in the original payload text.
    pub byte_offset: usize,
    /// 0-indexed page number.
    #[serde(default)]
    pub page: usize,
    /// 64-bit hash of the query parameters and content the cursor was issued for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_hash: Option<u64>,
}

/// Extra pagination context: cursor/query binding and the list of sections in the payload.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaginationMeta<'a> {
    /// Hash binding cursors to a specific query; mismatching cursors are rejected.
    pub query_hash: Option<u64>,
    /// Section names (e.g. file paths) of the full payload; empty means "derive from headers".
    pub remaining_sections: &'a [String],
}

/// Computes a 64-bit query hash from parameter parts and the full payload text.
pub fn compute_query_hash(parts: &[&str], full_text: &str) -> u64 {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_be_bytes());
        h.update(p.as_bytes());
    }
    h.update(full_text.as_bytes());
    let d = h.finalize();
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[..8]);
    u64::from_be_bytes(b)
}

/// Extracts section names from `## File: \`x\`` and `=== file x ===` header lines.
pub fn extract_section_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            if let Some(r) = l.strip_prefix("## File:") {
                Some(r.trim().trim_matches('`').to_string())
            } else {
                l.strip_prefix("=== file").map(|r| {
                    r.trim()
                        .trim_end_matches('=')
                        .trim()
                        .trim_matches('`')
                        .to_string()
                })
            }
        })
        .filter(|s| !s.is_empty())
        .collect()
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encodes binary bytes to URL-safe base64 without padding.
pub fn base64_url_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() * 4).div_ceil(3));
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };

        let idx0 = (b0 >> 2) & 0x3F;
        let idx1 = ((b0 & 0x03) << 4) | ((b1 >> 4) & 0x0F);
        out.push(BASE64_ALPHABET[idx0 as usize] as char);
        out.push(BASE64_ALPHABET[idx1 as usize] as char);

        if i + 1 < data.len() {
            let idx2 = ((b1 & 0x0F) << 2) | ((b2 >> 6) & 0x03);
            out.push(BASE64_ALPHABET[idx2 as usize] as char);
        }
        if i + 2 < data.len() {
            let idx3 = b2 & 0x3F;
            out.push(BASE64_ALPHABET[idx3 as usize] as char);
        }

        i += 3;
    }
    out
}

/// Decodes URL-safe base64 string to bytes.
pub fn base64_url_decode(input: &str) -> Option<Vec<u8>> {
    let mut decode_table = [0xFFu8; 256];
    for (idx, &byte) in BASE64_ALPHABET.iter().enumerate() {
        decode_table[byte as usize] = idx as u8;
    }
    // Also tolerate standard '+' and '/'
    decode_table[b'+' as usize] = 62;
    decode_table[b'/' as usize] = 63;

    let clean: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'=' && !b.is_ascii_whitespace())
        .collect();

    let mut out = Vec::with_capacity((clean.len() * 3) / 4);
    let mut i = 0;
    while i < clean.len() {
        let c0 = *clean.get(i)?;
        let c1 = *clean.get(i + 1)?;
        let v0 = decode_table[c0 as usize];
        let v1 = decode_table[c1 as usize];
        if v0 == 0xFF || v1 == 0xFF {
            return None;
        }

        let b0 = (v0 << 2) | (v1 >> 4);
        out.push(b0);

        if let Some(&c2) = clean.get(i + 2) {
            let v2 = decode_table[c2 as usize];
            if v2 == 0xFF {
                return None;
            }
            let b1 = ((v1 & 0x0F) << 4) | (v2 >> 2);
            out.push(b1);

            if let Some(&c3) = clean.get(i + 3) {
                let v3 = decode_table[c3 as usize];
                if v3 == 0xFF {
                    return None;
                }
                let b2 = ((v2 & 0x03) << 6) | v3;
                out.push(b2);
            }
        }

        i += 4;
    }

    Some(out)
}

/// Encodes `CursorData` into an opaque string cursor with `cur_` prefix.
///
/// The payload is a compact binary form (flags byte, varint offset, varint page,
/// optional 8-byte big-endian query hash) so the footer stays within the small-budget
/// token tier. Legacy JSON payloads (starting with `{`) are still accepted on decode.
pub fn encode_cursor(cursor: &CursorData) -> String {
    let mut buf = Vec::with_capacity(24);
    buf.push(u8::from(cursor.query_hash.is_some()));
    write_varint(&mut buf, cursor.byte_offset as u64);
    write_varint(&mut buf, cursor.page as u64);
    if let Some(h) = cursor.query_hash {
        buf.extend_from_slice(&h.to_be_bytes());
    }
    format!("cur_{}", base64_url_encode(&buf))
}

/// Decodes an opaque cursor string into `CursorData`.
pub fn decode_cursor(cursor_str: &str) -> Option<CursorData> {
    let trimmed = cursor_str.trim();
    let payload = trimmed.strip_prefix("cur_").unwrap_or(trimmed);
    let bytes = base64_url_decode(payload)?;
    if bytes.first() == Some(&b'{') {
        return serde_json::from_slice(&bytes).ok();
    }
    let (&flags, mut rest) = bytes.split_first()?;
    if flags > 1 {
        return None;
    }
    let byte_offset = usize::try_from(read_varint(&mut rest)?).ok()?;
    let page = usize::try_from(read_varint(&mut rest)?).ok()?;
    let query_hash = if flags == 1 {
        let arr: [u8; 8] = rest.get(..8)?.try_into().ok()?;
        rest = &rest[8..];
        Some(u64::from_be_bytes(arr))
    } else {
        None
    };
    if !rest.is_empty() {
        return None;
    }
    Some(CursorData {
        byte_offset,
        page,
        query_hash,
    })
}

fn write_varint(buf: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        buf.push((v as u8 & 0x7F) | 0x80);
        v >>= 7;
    }
    buf.push(v as u8);
}

fn read_varint(input: &mut &[u8]) -> Option<u64> {
    let mut out = 0u64;
    for shift in (0..70).step_by(7) {
        let (&b, rest) = input.split_first()?;
        *input = rest;
        let part = u64::from(b & 0x7F);
        if shift >= 64 || (shift == 63 && part > 1) {
            return None;
        }
        out |= part << shift;
        if b & 0x80 == 0 {
            return Some(out);
        }
    }
    None
}

/// Formats the structured machine-readable footer per D19 and R1.
pub fn format_footer(
    truncated: bool,
    next_cursor: Option<&str>,
    stale: bool,
    tokens: usize,
) -> String {
    if truncated {
        format!(
            "[truncated: true | next_cursor: \"{}\" | stale: {} | tokens: {}]",
            next_cursor.unwrap_or(""),
            stale,
            tokens
        )
    } else {
        format!("[truncated: false | stale: {} | tokens: {}]", stale, tokens)
    }
}

/// Page position and remaining sections for the rich footer tier.
#[derive(Debug, Clone, Copy)]
pub struct FooterExtras<'a> {
    /// 1-based current page.
    pub page: usize,
    /// Estimated total number of pages.
    pub est_total: usize,
    /// Section names remaining after this page.
    pub remaining: &'a [String],
}

/// Footer with budget tiers: compact (`<= 35` tokens) below 256 tokens, rich from 256.
pub fn format_footer_ex(
    truncated: bool,
    next_cursor: Option<&str>,
    stale: bool,
    tokens: usize,
    effective_max_tokens: usize,
    extras: &FooterExtras<'_>,
) -> String {
    if !truncated {
        return format_footer(truncated, next_cursor, stale, tokens);
    }
    if effective_max_tokens < 256 {
        // Minimal tier: the cursor alone must fit into <= 35 tokens.
        let stale_part = if stale { " | stale: true" } else { "" };
        return format!(
            "[truncated: true | next_cursor: \"{}\"{} | tokens: {}]",
            next_cursor.unwrap_or(""),
            stale_part,
            tokens
        );
    }
    let mut out = format!(
        "[truncated: true | next_cursor: \"{}\" | stale: {} | tokens: {} | page: {}/{}",
        next_cursor.unwrap_or(""),
        stale,
        tokens,
        extras.page,
        extras.est_total.max(extras.page)
    );
    if !extras.remaining.is_empty() {
        const SHOWN: usize = 3;
        let shown: Vec<&str> = extras
            .remaining
            .iter()
            .take(SHOWN)
            .map(String::as_str)
            .collect();
        out.push_str(&format!(" | remaining: {}", shown.join(", ")));
        if extras.remaining.len() > SHOWN {
            out.push_str(&format!(" (+{})", extras.remaining.len() - SHOWN));
        }
    }
    out.push_str(" | next: pass cursor to continue, or files=[...] to narrow]");
    out
}

/// Output of a paginated response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PaginatedOutput {
    /// Slice of content returned for this page.
    pub body: String,
    /// Complete rendered output including structured machine-readable footer.
    pub rendered: String,
    /// `true` if content was truncated and more pages exist.
    pub truncated: bool,
    /// Opaque cursor to fetch the next page.
    pub next_cursor: Option<String>,
    /// `true` if the underlying card/data is stale relative to Git.
    pub stale: bool,
    /// Exact token count of the rendered output.
    pub token_count: usize,
}

/// Paginates `full_text` adhering strictly to `max_tokens` (D19, R1).
pub fn paginate_content(
    full_text: &str,
    cursor: Option<&str>,
    max_tokens: usize,
    stale: bool,
) -> PaginatedOutput {
    paginate_content_with(
        full_text,
        cursor,
        max_tokens,
        stale,
        &PaginationMeta::default(),
    )
    .expect("a cursor cannot mismatch when no query_hash is bound")
}

/// Paginates `full_text` like [`paginate_content`], binding cursors to `meta.query_hash`.
///
/// Returns an error when the supplied cursor was issued for a different query.
pub fn paginate_content_with(
    full_text: &str,
    cursor: Option<&str>,
    max_tokens: usize,
    stale: bool,
    meta: &PaginationMeta<'_>,
) -> Result<PaginatedOutput, String> {
    let effective_max_tokens = if max_tokens == 0 {
        ci_core::config::DEFAULT_MAX_RESPONSE_TOKENS
    } else {
        max_tokens
    };

    let parsed_cursor = cursor.and_then(decode_cursor);
    if let (Some(c), Some(expected)) = (parsed_cursor.as_ref(), meta.query_hash) {
        if let Some(got) = c.query_hash {
            if got != expected {
                return Err(
                    "cursor does not match this request (query parameters or content changed); restart pagination without a cursor"
                        .to_string(),
                );
            }
        }
    }

    if full_text.is_empty() {
        let footer_if_stale = if stale {
            Some(format_footer(false, None, true, 0))
        } else {
            None
        };
        let rendered = footer_if_stale.unwrap_or_default();
        let token_count = count_tokens(&rendered);
        return Ok(PaginatedOutput {
            body: String::new(),
            rendered,
            truncated: false,
            next_cursor: None,
            stale,
            token_count,
        });
    }

    let start_offset = parsed_cursor.as_ref().map(|c| c.byte_offset).unwrap_or(0);
    let page_no = parsed_cursor.as_ref().map(|c| c.page).unwrap_or(0);

    let start_offset = find_valid_char_boundary(full_text, start_offset);
    if start_offset >= full_text.len() {
        // Cursor reached end of content
        let footer_if_stale = if stale {
            Some(format_footer(false, None, true, 0))
        } else {
            None
        };
        let rendered = footer_if_stale.unwrap_or_default();
        let token_count = count_tokens(&rendered);
        return Ok(PaginatedOutput {
            body: String::new(),
            rendered,
            truncated: false,
            next_cursor: None,
            stale,
            token_count,
        });
    }

    let remaining = &full_text[start_offset..];
    let remaining_tokens = count_tokens(remaining);

    // If remaining content fits completely within budget
    if remaining_tokens <= effective_max_tokens {
        let needs_footer = stale || parsed_cursor.is_some();
        let footer = if needs_footer {
            let tentative = format!(
                "{}\n\n{}",
                remaining,
                format_footer(false, None, stale, remaining_tokens)
            );
            let final_tokens = count_tokens(&tentative);
            if final_tokens <= effective_max_tokens {
                Some(format_footer(false, None, stale, final_tokens))
            } else {
                // With footer, it exceeds budget; fall through to truncation branch below
                None
            }
        } else {
            None
        };

        if !needs_footer || footer.is_some() {
            let rendered = if let Some(ref f) = footer {
                format!("{}\n\n{}", remaining, f)
            } else {
                remaining.to_string()
            };
            let token_count = count_tokens(&rendered);
            return Ok(PaginatedOutput {
                body: remaining.to_string(),
                rendered,
                truncated: false,
                next_cursor: None,
                stale,
                token_count,
            });
        }
    }

    // Content exceeds budget: truncate and append footer with next_cursor
    let rich = effective_max_tokens >= 256;
    let all_sections: Vec<String> = if !rich {
        Vec::new()
    } else if meta.remaining_sections.is_empty() {
        extract_section_names(full_text)
    } else {
        meta.remaining_sections.to_vec()
    };
    let remaining_after = |offset: usize| -> Vec<String> {
        let tail_start = find_valid_char_boundary(full_text, offset);
        all_sections
            .iter()
            .filter(|n| full_text.rfind(n.as_str()).is_some_and(|p| p >= tail_start))
            .cloned()
            .collect()
    };
    let make_cursor = |offset: usize| {
        encode_cursor(&CursorData {
            byte_offset: offset,
            page: page_no.saturating_add(1),
            query_hash: meta.query_hash,
        })
    };

    // Reserve: compact footer ~35 tokens; rich footer measured on a sample
    let footer_reserve = if rich {
        let sample_cur = make_cursor(full_text.len());
        let sample_rem = remaining_after(start_offset);
        let sample = format_footer_ex(
            true,
            Some(&sample_cur),
            stale,
            effective_max_tokens,
            effective_max_tokens,
            &FooterExtras {
                page: page_no.saturating_add(1),
                est_total: page_no.saturating_add(1),
                remaining: &sample_rem,
            },
        );
        (count_tokens(&sample) + 8).min(effective_max_tokens / 2)
    } else {
        35.min(effective_max_tokens / 2).max(5)
    };
    let target_body_tokens = effective_max_tokens.saturating_sub(footer_reserve).max(1);
    let est_total = page_no
        .saturating_add(remaining_tokens.div_ceil(target_body_tokens))
        .max(page_no.saturating_add(1));

    // Smallest non-empty body: the first character (may span several UTF-8 bytes)
    let min_split = find_next_char_boundary(remaining, 1);
    let mut split_len = find_split_point(remaining, target_body_tokens);
    if split_len == 0 {
        split_len = min_split;
    }

    // Iteratively adjust body to guarantee body + footer strictly <= effective_max_tokens
    let mut body;
    let mut next_offset;
    let mut next_cur;
    let mut rendered;
    let mut final_token_count;

    loop {
        body = &remaining[..split_len];
        next_offset = start_offset + split_len;
        next_cur = make_cursor(next_offset);
        let rem = if rich {
            remaining_after(next_offset)
        } else {
            Vec::new()
        };
        let extras = FooterExtras {
            page: page_no.saturating_add(1),
            est_total,
            remaining: &rem,
        };
        let footer = |tokens: usize| {
            format_footer_ex(
                true,
                Some(&next_cur),
                stale,
                tokens,
                effective_max_tokens,
                &extras,
            )
        };

        // 1. Format tentative footer with effective_max_tokens as placeholder token count
        let mut f = footer(effective_max_tokens);
        rendered = format!("{}\n\n{}", body, f);
        final_token_count = count_tokens(&rendered);

        // 2. Refine footer with measured tokens
        f = footer(final_token_count);
        rendered = format!("{}\n\n{}", body, f);
        final_token_count = count_tokens(&rendered);

        if final_token_count <= effective_max_tokens || split_len <= min_split {
            break;
        }

        // Back off strictly monotonically to an earlier character boundary, never below min_split
        let backoff = (split_len / 10).max(1);
        let new_len = split_len.saturating_sub(backoff);
        split_len = find_valid_char_boundary(remaining, new_len).max(min_split);
    }

    Ok(PaginatedOutput {
        body: body.to_string(),
        rendered,
        truncated: true,
        next_cursor: Some(next_cur),
        stale,
        token_count: final_token_count,
    })
}

/// Finds the split point within `text` yielding approximately `target_tokens`,
/// preferring newline boundaries (`\n`).
fn find_split_point(full: &str, target_tokens: usize) -> usize {
    if full.is_empty() || target_tokens == 0 {
        return 0;
    }

    // Bounded byte window (~6 bytes/token, at least 512), doubled only while the
    // window still fits into the token budget. Splitting is done on char boundaries
    // with `count_tokens` over prefixes, never by decoding a token slice.
    let mut win = find_valid_char_boundary(full, (target_tokens * 6).max(512).min(full.len()));
    while win < full.len() && count_tokens(&full[..win]) <= target_tokens {
        let next = find_valid_char_boundary(full, win.saturating_mul(2).min(full.len()));
        if next <= win {
            break;
        }
        win = next;
    }
    let text = if win == 0 { full } else { &full[..win] };

    let mut low = 0;
    let mut high = text.len();
    let mut best_split = 0;

    // Binary search over byte indices
    while low <= high {
        let mid = (low + high) / 2;
        let mid_boundary = find_valid_char_boundary(text, mid);
        let prefix = &text[..mid_boundary];
        let tokens = count_tokens(prefix);

        if tokens <= target_tokens {
            best_split = mid_boundary;
            // Advance past `mid`, not `mid_boundary`: when `mid` falls inside a
            // multibyte char, `mid_boundary + 1` may not exceed `low` and the search stalls.
            low = mid + 1;
        } else {
            if mid_boundary == 0 {
                break;
            }
            high = mid_boundary - 1;
        }
    }

    // Look backwards from best_split for a clean newline boundary within the last 15% of chunk
    if best_split > 0 && best_split < text.len() {
        let raw_start = best_split.saturating_sub((best_split / 6).max(20));
        let search_start = find_valid_char_boundary(text, raw_start);
        if search_start < best_split {
            let slice = &text[search_start..best_split];
            if let Some(rel_newline) = slice.rfind('\n') {
                let candidate = search_start + rel_newline + 1;
                if candidate > 0 && candidate <= best_split {
                    return candidate;
                }
            }
        }
    }

    best_split
}

fn find_valid_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn find_next_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    if i == 0 && !s.is_empty() {
        i = s.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_url_roundtrip() {
        let cases = [
            "",
            "f",
            "fo",
            "foo",
            "foobar",
            "Hello, world! 🦀 🚀",
            "{\"byte_offset\": 12345, \"page\": 2}",
        ];
        for case in cases {
            let enc = base64_url_encode(case.as_bytes());
            let dec = base64_url_decode(&enc).expect("valid base64 decode");
            assert_eq!(String::from_utf8(dec).unwrap(), case);
        }
    }

    #[test]
    fn cursor_encode_decode_roundtrip() {
        let cursor = CursorData {
            byte_offset: 4096,
            page: 3,
            query_hash: None,
        };
        let encoded = encode_cursor(&cursor);
        assert!(encoded.starts_with("cur_"));

        let decoded = decode_cursor(&encoded).expect("valid cursor decode");
        assert_eq!(decoded, cursor);

        // Invalid cursor returns None without panic
        assert_eq!(decode_cursor("invalid-cursor-###"), None);
        assert_eq!(decode_cursor(""), None);
    }

    #[test]
    fn footer_formatting() {
        let f_trunc = format_footer(true, Some("cur_test"), false, 3950);
        assert_eq!(
            f_trunc,
            "[truncated: true | next_cursor: \"cur_test\" | stale: false | tokens: 3950]"
        );

        let f_trunc_stale = format_footer(true, Some("cur_test2"), true, 4000);
        assert_eq!(
            f_trunc_stale,
            "[truncated: true | next_cursor: \"cur_test2\" | stale: true | tokens: 4000]"
        );

        let f_clean = format_footer(false, None, false, 500);
        assert_eq!(f_clean, "[truncated: false | stale: false | tokens: 500]");
    }

    #[test]
    fn small_content_fits_without_truncation() {
        let text = "Short summary of the core subsystem.";
        let out = paginate_content(text, None, 4000, false);
        assert!(!out.truncated);
        assert_eq!(out.next_cursor, None);
        assert_eq!(out.rendered, text);
        assert!(out.token_count > 0 && out.token_count <= 20);
    }

    #[test]
    fn large_content_paginates_across_pages_and_respects_token_budget() {
        // Construct ~120 lines of repetitive text (~600 tokens)
        let lines: Vec<String> = (0..120)
            .map(|i| {
                format!("Line {i}: Subsystem detail and exported function signature definitions.")
            })
            .collect();
        let full_text = lines.join("\n");

        let budget = 120; // Small token budget to force multiple pages
        let mut pages = Vec::new();
        let mut cur = None;

        for _ in 0..40 {
            let out = paginate_content(&full_text, cur.as_deref(), budget, false);
            assert!(
                out.token_count <= budget,
                "Token count {} must not exceed budget {}",
                out.token_count,
                budget
            );
            if out.truncated {
                assert!(out
                    .rendered
                    .contains("[truncated: true | next_cursor: \"cur_"));
                assert!(out.next_cursor.is_some());
                pages.push(out.body.clone());
                cur = out.next_cursor;
            } else {
                pages.push(out.body.clone());
                break;
            }
        }

        // Reassembled content from all pages must equal original text
        let reassembled = pages.join("");
        assert_eq!(reassembled, full_text);
    }

    #[test]
    fn multibyte_utf8_truncation_does_not_panic_and_preserves_boundaries() {
        let text =
            "Multilingual test: привет мир! 🚀 🦀 Примеры функций и интерфейсов.\n".repeat(30);
        // Budgets below ~60 cannot fit the cursor footer itself and are out of contract
        for budget in 60..120 {
            let out = paginate_content(&text, None, budget, false);
            assert!(
                out.token_count <= budget,
                "Budget {} exceeded by count {}",
                budget,
                out.token_count
            );
            if out.truncated {
                assert!(out.next_cursor.is_some());
                assert!(out.rendered.contains("[truncated: true"));
            }
        }
    }

    #[test]
    fn multibyte_leading_char_pages_terminate_and_reassemble() {
        let text = "привет мир 🦀\n".repeat(40);
        for budget in [64, 70, 97, 150] {
            let mut cursor: Option<String> = None;
            let mut reassembled = String::new();
            for _ in 0..text.len() {
                let out = paginate_content(&text, cursor.as_deref(), budget, false);
                reassembled.push_str(&out.body);
                match out.next_cursor {
                    Some(c) if out.truncated => cursor = Some(c),
                    _ => break,
                }
            }
            assert_eq!(reassembled, text, "budget {}", budget);
        }
    }

    #[test]
    fn cursor_page_overflow_saturates() {
        let text = "line of content\n".repeat(200);
        let cursor = encode_cursor(&CursorData {
            byte_offset: 16,
            page: usize::MAX,
            query_hash: None,
        });
        let out = paginate_content(&text, Some(&cursor), 64, false);
        assert!(out.truncated);
        let next = decode_cursor(out.next_cursor.as_deref().unwrap()).unwrap();
        assert_eq!(next.page, usize::MAX);
    }

    #[test]
    fn paged_completion_appends_footer_when_cursor_provided() {
        let text = "First segment of content.\nSecond segment of content.\nFinal line.";
        let out_p1 = paginate_content(text, None, 10, false);
        assert!(out_p1.truncated);
        let cur = out_p1.next_cursor.expect("cursor present");

        let out_p2 = paginate_content(text, Some(&cur), 4000, false);
        assert!(!out_p2.truncated);
        assert!(out_p2.rendered.contains("[truncated: false | stale: false"));
    }

    fn collect_pages(text: &str, budget: usize, meta: &PaginationMeta<'_>) -> String {
        let mut cursor: Option<String> = None;
        let mut all = String::new();
        for _ in 0..text.len() {
            let out = paginate_content_with(text, cursor.as_deref(), budget, false, meta)
                .expect("pagination");
            assert!(
                out.token_count <= budget,
                "{} > {}: {}",
                out.token_count,
                budget,
                out.rendered
            );
            all.push_str(&out.body);
            match out.next_cursor {
                Some(c) if out.truncated => cursor = Some(c),
                _ => break,
            }
        }
        all
    }

    #[test]
    fn test_paginate_multibyte_utf8_boundary_and_query_hash_mismatch() {
        let text = "// Комментарий: ОбработатьДанные()\n// 🦀 Обработка\n".repeat(150);
        let hash_a = compute_query_hash(&["sub", "full"], &text);
        let meta_a = PaginationMeta {
            query_hash: Some(hash_a),
            remaining_sections: &[],
        };
        for budget in [64, 300] {
            assert_eq!(
                collect_pages(&text, budget, &meta_a),
                text,
                "budget {budget}"
            );
        }

        let first = paginate_content_with(&text, None, 64, false, &meta_a).unwrap();
        let cur = first.next_cursor.expect("cursor");
        let meta_b = PaginationMeta {
            query_hash: Some(compute_query_hash(&["sub", "full"], "другой текст")),
            remaining_sections: &[],
        };
        let err = paginate_content_with(&text, Some(&cur), 64, false, &meta_b)
            .expect_err("mismatch must fail");
        assert!(err.contains("cursor"), "{err}");
        assert!(paginate_content_with(&text, Some(&cur), 64, false, &meta_a).is_ok());
    }

    #[test]
    fn cursor_binary_roundtrip_with_hash_and_legacy_json() {
        let c = CursorData {
            byte_offset: 123_456,
            page: 7,
            query_hash: Some(u64::MAX - 3),
        };
        assert_eq!(decode_cursor(&encode_cursor(&c)), Some(c));
        let legacy = format!(
            "cur_{}",
            base64_url_encode(br#"{"byte_offset":9,"page":2}"#)
        );
        let d = decode_cursor(&legacy).expect("legacy json cursor");
        assert_eq!((d.byte_offset, d.page, d.query_hash), (9, 2, None));
    }

    #[test]
    fn test_format_footer_respects_budget_tiers() {
        let text = "// 🦀 Обработка данных\nfn f() {}\n".repeat(200);
        let out =
            paginate_content_with(&text, None, 64, false, &PaginationMeta::default()).unwrap();
        assert!(out.truncated);
        assert!(out
            .rendered
            .contains("[truncated: true | next_cursor: \"cur_"));
        let footer = &out.rendered[out.body.len()..];
        assert!(
            count_tokens(footer) <= 35,
            "{} {footer}",
            count_tokens(footer)
        );

        let mut big = String::new();
        for i in 0..80 {
            big.push_str(&format!(
                "## File: `src/f{i}.rs`\n{}\n",
                "let x = 1;\n".repeat(40)
            ));
        }
        let sections: Vec<String> = (0..80).map(|i| format!("src/f{i}.rs")).collect();
        let meta = PaginationMeta {
            query_hash: None,
            remaining_sections: &sections,
        };
        let out = paginate_content_with(&big, None, 1000, false, &meta).unwrap();
        assert!(out.truncated);
        assert!(out.token_count <= 1000);
        assert!(out.rendered.contains("page: 1/"), "{}", out.rendered);
        assert!(out.rendered.contains("remaining:"));
        assert!(out.rendered.contains("next:"));
    }
}
