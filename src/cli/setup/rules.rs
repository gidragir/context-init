//! The short agent rule installed by `setup`, and marker-delimited upserts.

/// The rule text installed into harness rule files (≤ 15 lines: it lands in every prompt).
pub const RULE_TEMPLATE: &str = include_str!("rule_template.md");

pub const BEGIN_MARKER: &str = "<!-- context-init:begin -->";
pub const END_MARKER: &str = "<!-- context-init:end -->";

/// Replaces the marker-delimited block in `existing` with `body`, or appends the block
/// (after one blank line) when there is none. Text outside the markers is untouched.
pub fn upsert_marked_block(existing: Option<&str>, body: &str) -> String {
    let block = format!("{BEGIN_MARKER}\n{}\n{END_MARKER}", body.trim_end());
    let existing = existing.unwrap_or("");
    if let (Some(start), Some(end)) = (existing.find(BEGIN_MARKER), existing.find(END_MARKER)) {
        if start < end {
            let after = &existing[end + END_MARKER.len()..];
            return format!("{}{block}{after}", &existing[..start]);
        }
    }
    let head = existing.trim_end();
    if head.is_empty() {
        format!("{block}\n")
    } else {
        format!("{head}\n\n{block}\n")
    }
}

/// Claude Code skill wrapping the rule.
pub fn skill_file() -> String {
    format!(
        "---\nname: context-init\ndescription: Navigate this project through the context-init knowledge base (subsystem map, cards, compressed code) instead of reading source files broadly.\n---\n\n{RULE_TEMPLATE}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_marked_block_is_idempotent() {
        let once = upsert_marked_block(Some("# My rules\n"), "body v1");
        let twice = upsert_marked_block(Some(&once), "body v1");
        assert_eq!(once, twice);
        assert_eq!(twice.matches(BEGIN_MARKER).count(), 1);
        let updated = upsert_marked_block(Some(&twice), "body v2");
        assert!(
            updated.contains("body v2") && !updated.contains("body v1"),
            "{updated}"
        );
        assert_eq!(updated.matches(BEGIN_MARKER).count(), 1);
    }

    #[test]
    fn upsert_marked_block_preserves_surrounding_text() {
        let existing = format!("before\n\n{BEGIN_MARKER}\nold\n{END_MARKER}\n\nafter\n");
        let out = upsert_marked_block(Some(&existing), "new");
        assert!(out.starts_with("before\n"), "{out}");
        assert!(out.ends_with("after\n"), "{out}");
        assert!(out.contains("new") && !out.contains("old"), "{out}");
        assert_eq!(
            upsert_marked_block(None, "x"),
            format!("{BEGIN_MARKER}\nx\n{END_MARKER}\n")
        );
    }

    #[test]
    fn rule_template_is_at_most_15_lines() {
        let lines = RULE_TEMPLATE.trim_end().lines().count();
        assert!(lines <= 15, "rule template has {lines} lines");
        for tool in [
            "context_get_subsystems",
            "context_get_subsystem",
            "context_pack",
            "context_check",
        ] {
            assert!(RULE_TEMPLATE.contains(tool), "missing {tool}");
        }
        assert!(RULE_TEMPLATE.contains("context-init pack <subsystem_id> --compress"));
    }
}
