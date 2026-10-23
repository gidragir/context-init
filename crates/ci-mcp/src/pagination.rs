//! Response token budget enforcement, opaque cursor pagination, and truncation footer (D19, R1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static BPE_TOKENIZER: OnceLock<Option<tiktoken_rs::CoreBPE>> = OnceLock::new();

/// Counts tokens using BPE (`cl100k_base`), falling back to character/word heuristics.
pub fn count_tokens(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let bpe = BPE_TOKENIZER.get_or_init(|| tiktoken_rs::cl100k_base().ok());
    if let Some(tokenizer) = bpe {
        tokenizer.encode_with_special_tokens(text).len()
    } else {
        let chars = trimmed.chars().count().div_ceil(4);
        let words = trimmed.split_whitespace().count();
        chars.max(words).max(1)
    }
}

/// Structured opaque cursor payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CursorData {
    /// Byte offset in the original payload text.
    pub byte_offset: usize,
    /// 0-indexed page number.
    #[serde(default)]
    pub page: usize,
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
pub fn encode_cursor(cursor: &CursorData) -> String {
    let json = serde_json::to_string(cursor).unwrap_or_else(|_| "{}".to_string());
    format!("cur_{}", base64_url_encode(json.as_bytes()))
}

/// Decodes an opaque cursor string into `CursorData`.
pub fn decode_cursor(cursor_str: &str) -> Option<CursorData> {
    let trimmed = cursor_str.trim();
    let payload = trimmed.strip_prefix("cur_").unwrap_or(trimmed);
    let bytes = base64_url_decode(payload)?;
    serde_json::from_slice(&bytes).ok()
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
    let effective_max_tokens = if max_tokens == 0 {
        ci_core::config::DEFAULT_MAX_RESPONSE_TOKENS
    } else {
        max_tokens
    };

    if full_text.is_empty() {
        let footer_if_stale = if stale {
            Some(format_footer(false, None, true, 0))
        } else {
            None
        };
        let rendered = footer_if_stale.unwrap_or_default();
        let token_count = count_tokens(&rendered);
        return PaginatedOutput {
            body: String::new(),
            rendered,
            truncated: false,
            next_cursor: None,
            stale,
            token_count,
        };
    }

    let parsed_cursor = cursor.and_then(decode_cursor);
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
        return PaginatedOutput {
            body: String::new(),
            rendered,
            truncated: false,
            next_cursor: None,
            stale,
            token_count,
        };
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
            return PaginatedOutput {
                body: remaining.to_string(),
                rendered,
                truncated: false,
                next_cursor: None,
                stale,
                token_count,
            };
        }
    }

    // Content exceeds budget: truncate and append footer with next_cursor
    // Reserve ~35 tokens for footer
    let footer_reserve = 35.min(effective_max_tokens / 2).max(5);
    let target_body_tokens = effective_max_tokens.saturating_sub(footer_reserve).max(1);

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
        next_cur = encode_cursor(&CursorData {
            byte_offset: next_offset,
            page: page_no + 1,
        });

        // 1. Format tentative footer with effective_max_tokens as placeholder token count
        let mut f = format_footer(true, Some(&next_cur), stale, effective_max_tokens);
        rendered = format!("{}\n\n{}", body, f);
        final_token_count = count_tokens(&rendered);

        // 2. Refine footer with measured tokens
        f = format_footer(true, Some(&next_cur), stale, final_token_count);
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

    PaginatedOutput {
        body: body.to_string(),
        rendered,
        truncated: true,
        next_cursor: Some(next_cur),
        stale,
        token_count: final_token_count,
    }
}

/// Finds the split point within `text` yielding approximately `target_tokens`,
/// preferring newline boundaries (`\n`).
fn find_split_point(text: &str, target_tokens: usize) -> usize {
    if text.is_empty() || target_tokens == 0 {
        return 0;
    }

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
    fn paged_completion_appends_footer_when_cursor_provided() {
        let text = "First segment of content.\nSecond segment of content.\nFinal line.";
        let out_p1 = paginate_content(text, None, 10, false);
        assert!(out_p1.truncated);
        let cur = out_p1.next_cursor.expect("cursor present");

        let out_p2 = paginate_content(text, Some(&cur), 4000, false);
        assert!(!out_p2.truncated);
        assert!(out_p2.rendered.contains("[truncated: false | stale: false"));
    }
}
