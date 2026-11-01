//! BPE token counting shared by analysis, pagination and telemetry.

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
