//! JSON payload extraction from Markdown code fences and recursive JSON Schema validation (D14).

use crate::provider::{LlmExecutionError, RoleName};
use anyhow::{bail, Result};
use serde_json::{json, Value};

/// Strips Markdown code fences (` ```json ... ``` ` or ` ``` ... ``` `) and surrounding prose
/// to isolate a raw JSON object (`{...}`) or array (`[...]`).
pub fn extract_json_payload(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return trimmed;
    }

    // 1. If `trimmed` already starts with `{` or `[` and is a complete valid JSON value, return it directly
    //    (prevents inner ``` code blocks inside JSON string fields from being mistaken for outer fences).
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && is_complete_json_value(trimmed) {
        return trimmed;
    }

    // 2. Handle outer ```json ... ``` or ``` ... ``` fenced code blocks at the start of the string
    if let Some(after_open) = trimmed.strip_prefix("```") {
        let body_start = strip_fence_header(after_open);
        let without_close = if let Some(close_idx) = body_start.rfind("```") {
            &body_start[..close_idx]
        } else {
            body_start
        };
        let candidate = without_close.trim();
        if let Some(extracted) = find_first_valid_json_slice(candidate) {
            return extracted;
        }
        return candidate;
    }

    // 3. If `trimmed` starts with `{` or `[` (with trailing prose after the JSON value), slice the exact JSON prefix
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Some(extracted) = find_first_valid_json_slice(trimmed) {
            return extracted;
        }
    }

    // 4. Handle fenced code block embedded inside surrounding prose
    if let Some(fence_start) = trimmed.find("```") {
        let after_open = &trimmed[fence_start + 3..];
        let body_start = strip_fence_header(after_open);
        // Prefer the last closing fence first so inner ``` inside JSON string values do not truncate the payload
        if let Some(last_close) = body_start.rfind("```") {
            let candidate = body_start[..last_close].trim();
            if is_complete_json_value(candidate) {
                return candidate;
            }
            if let Some(extracted) = find_first_valid_json_slice(candidate) {
                return extracted;
            }
        }
        if let Some(first_close) = body_start.find("```") {
            let candidate = body_start[..first_close].trim();
            if let Some(extracted) = find_first_valid_json_slice(candidate) {
                return extracted;
            }
            return candidate;
        }
    }

    // 5. Scan for the first valid JSON object `{...}` or array `[...]` embedded in prose
    if let Some(extracted) = find_first_valid_json_slice(trimmed) {
        return extracted;
    }

    trimmed
}

fn strip_fence_header(after_open: &str) -> &str {
    if let Some(idx) = after_open.find('\n') {
        let first_line = after_open[..idx].trim();
        // If the first line after ``` is just a language identifier (or empty), skip it
        if first_line.is_empty()
            || first_line
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return &after_open[idx + 1..];
        }
    }
    after_open
        .strip_prefix("json")
        .or_else(|| after_open.strip_prefix("JSON"))
        .unwrap_or(after_open)
}

fn is_complete_json_value(s: &str) -> bool {
    let mut stream = serde_json::Deserializer::from_str(s).into_iter::<serde::de::IgnoredAny>();
    if let Some(Ok(_)) = stream.next() {
        let rest = s[stream.byte_offset()..].trim();
        rest.is_empty()
    } else {
        false
    }
}

fn find_first_valid_json_slice(s: &str) -> Option<&str> {
    for (idx, ch) in s.char_indices() {
        if ch == '{' || ch == '[' {
            let sub = &s[idx..];
            let mut stream =
                serde_json::Deserializer::from_str(sub).into_iter::<serde::de::IgnoredAny>();
            if let Some(Ok(_)) = stream.next() {
                let end = stream.byte_offset();
                if end > 0 {
                    return Some(&sub[..end]);
                }
            }
        }
    }
    None
}

/// Parses a JSON string (stripping Markdown fences if present) and validates it against an optional JSON Schema.
/// If parsing or validation fails, wraps the error in `LlmExecutionError` with `schema_error = true`.
pub fn parse_and_validate_json(
    raw: &str,
    schema: Option<&Value>,
    tokens_in: usize,
) -> Result<Value> {
    let cleaned = extract_json_payload(raw);
    let parsed: Value = match serde_json::from_str(cleaned) {
        Ok(val) => val,
        Err(err) => {
            return Err(anyhow::Error::new(LlmExecutionError {
                message: format!("Invalid JSON in LLM response: {err}"),
                retries: 0,
                schema_error: true,
                timed_out: false,
                budget_exceeded: false,
                tokens_in,
            }));
        }
    };

    if let Some(schema_val) = schema {
        if let Err(err) = validate_json_schema(schema_val, &parsed) {
            return Err(anyhow::Error::new(LlmExecutionError {
                message: format!("JSON Schema validation failed: {err}"),
                retries: 0,
                schema_error: true,
                timed_out: false,
                budget_exceeded: false,
                tokens_in,
            }));
        }
    }

    Ok(parsed)
}

/// Validates a `serde_json::Value` instance against a JSON Schema definition.
pub fn validate_json_schema(schema: &Value, instance: &Value) -> Result<()> {
    validate_at_path(schema, schema, instance, "$", 0)
}

const MAX_SCHEMA_REF_DEPTH: usize = 64;

fn validate_at_path(
    root_schema: &Value,
    schema: &Value,
    instance: &Value,
    path: &str,
    depth: usize,
) -> Result<()> {
    if depth > MAX_SCHEMA_REF_DEPTH {
        bail!("JSON Schema validation exceeded maximum $ref recursion depth at '{path}'");
    }

    if let Some(b) = schema.as_bool() {
        if b {
            return Ok(());
        } else {
            bail!("Schema at '{path}' is false (no value allowed)");
        }
    }

    let Some(schema_obj) = schema.as_object() else {
        return Ok(());
    };

    // 0. Resolve local `$ref` (`"#"` or `"#/definitions/..."` / `"#/$defs/..."`)
    if let Some(ref_str) = schema_obj.get("$ref").and_then(Value::as_str) {
        let target_schema = if ref_str == "#" {
            Some(root_schema)
        } else if let Some(ptr) = ref_str.strip_prefix('#') {
            root_schema.pointer(ptr)
        } else {
            None
        };
        let Some(resolved) = target_schema else {
            bail!("Unresolvable JSON Schema $ref '{ref_str}' at '{path}'");
        };
        validate_at_path(root_schema, resolved, instance, path, depth + 1)?;
    }

    // 1. Validate `const`
    if let Some(expected_const) = schema_obj.get("const") {
        if instance != expected_const {
            bail!("Value at '{path}' does not match const {expected_const} (got {instance})");
        }
    }

    // 2. Validate `enum`
    if let Some(enum_arr) = schema_obj.get("enum").and_then(Value::as_array) {
        if !enum_arr.contains(instance) {
            bail!("Value at '{path}' ({instance}) is not one of allowed enum values: {enum_arr:?}");
        }
    }

    // 3. Validate `type`
    if let Some(type_spec) = schema_obj.get("type") {
        match type_spec {
            Value::String(expected_type) => {
                if !matches_json_type(expected_type, instance) {
                    bail!(
                        "Type mismatch at '{path}': expected '{expected_type}', got {}",
                        json_type_name(instance)
                    );
                }
            }
            Value::Array(types) => {
                let any_matched = types
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|t| matches_json_type(t, instance));
                if !any_matched {
                    bail!(
                        "Type mismatch at '{path}': expected one of {types:?}, got {}",
                        json_type_name(instance)
                    );
                }
            }
            _ => {}
        }
    }

    // 4. Validate string constraints
    if let Some(s) = instance.as_str() {
        let char_len = s.chars().count();
        if let Some(min_len) = schema_obj.get("minLength").and_then(Value::as_u64) {
            if (char_len as u64) < min_len {
                bail!(
                    "String at '{path}' is shorter than minLength {min_len} (length: {char_len})"
                );
            }
        }
        if let Some(max_len) = schema_obj.get("maxLength").and_then(Value::as_u64) {
            if (char_len as u64) > max_len {
                bail!("String at '{path}' exceeds maxLength {max_len} (length: {char_len})");
            }
        }
    }

    // 5. Validate numeric constraints
    if let Some(num) = instance.as_f64() {
        if let Some(min_val) = schema_obj.get("minimum").and_then(Value::as_f64) {
            if num < min_val {
                bail!("Number at '{path}' ({num}) is less than minimum {min_val}");
            }
        }
        if let Some(max_val) = schema_obj.get("maximum").and_then(Value::as_f64) {
            if num > max_val {
                bail!("Number at '{path}' ({num}) exceeds maximum {max_val}");
            }
        }
        if let Some(ex_min) = schema_obj.get("exclusiveMinimum").and_then(Value::as_f64) {
            if num <= ex_min {
                bail!("Number at '{path}' ({num}) must be > exclusiveMinimum {ex_min}");
            }
        }
        if let Some(ex_max) = schema_obj.get("exclusiveMaximum").and_then(Value::as_f64) {
            if num >= ex_max {
                bail!("Number at '{path}' ({num}) must be < exclusiveMaximum {ex_max}");
            }
        }
    }

    // 6. Validate array constraints
    if let Some(arr) = instance.as_array() {
        if let Some(min_items) = schema_obj.get("minItems").and_then(Value::as_u64) {
            if (arr.len() as u64) < min_items {
                bail!(
                    "Array at '{path}' has {} items, fewer than minItems {min_items}",
                    arr.len()
                );
            }
        }
        if let Some(max_items) = schema_obj.get("maxItems").and_then(Value::as_u64) {
            if (arr.len() as u64) > max_items {
                bail!(
                    "Array at '{path}' has {} items, exceeding maxItems {max_items}",
                    arr.len()
                );
            }
        }
        if schema_obj
            .get("uniqueItems")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            for i in 0..arr.len() {
                for j in (i + 1)..arr.len() {
                    if arr[i] == arr[j] {
                        bail!("Array at '{path}' contains duplicate item at indices {i} and {j}");
                    }
                }
            }
        }
        if let Some(items_schema) = schema_obj.get("items") {
            for (idx, item) in arr.iter().enumerate() {
                let child_path = format!("{path}[{idx}]");
                validate_at_path(root_schema, items_schema, item, &child_path, depth + 1)?;
            }
        }
    }

    // 7. Validate object constraints
    if let Some(obj) = instance.as_object() {
        if let Some(required_arr) = schema_obj.get("required").and_then(Value::as_array) {
            for req_key in required_arr.iter().filter_map(Value::as_str) {
                if !obj.contains_key(req_key) {
                    bail!("Missing required property '{req_key}' at '{path}'");
                }
            }
        }

        let props_map = schema_obj.get("properties").and_then(Value::as_object);
        if let Some(props) = props_map {
            for (prop_name, prop_schema) in props {
                if let Some(prop_val) = obj.get(prop_name) {
                    let child_path = format!("{path}.{prop_name}");
                    validate_at_path(root_schema, prop_schema, prop_val, &child_path, depth + 1)?;
                }
            }
        }

        if let Some(add_props) = schema_obj.get("additionalProperties") {
            match add_props {
                Value::Bool(false) => {
                    for key in obj.keys() {
                        let is_known = props_map.is_some_and(|p| p.contains_key(key));
                        if !is_known {
                            bail!("Unexpected additional property '{key}' at '{path}'");
                        }
                    }
                }
                Value::Object(_) => {
                    for (key, val) in obj {
                        let is_known = props_map.is_some_and(|p| p.contains_key(key));
                        if !is_known {
                            let child_path = format!("{path}.{key}");
                            validate_at_path(root_schema, add_props, val, &child_path, depth + 1)?;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // 8. Validate combinators: `allOf`, `anyOf`, `oneOf`, `not`
    if let Some(all_of) = schema_obj.get("allOf").and_then(Value::as_array) {
        for sub_schema in all_of {
            validate_at_path(root_schema, sub_schema, instance, path, depth + 1)?;
        }
    }

    if let Some(any_of) = schema_obj.get("anyOf").and_then(Value::as_array) {
        let matched = any_of
            .iter()
            .any(|sub| validate_at_path(root_schema, sub, instance, path, depth + 1).is_ok());
        if !matched {
            bail!("Value at '{path}' did not match any branch in 'anyOf'");
        }
    }

    if let Some(one_of) = schema_obj.get("oneOf").and_then(Value::as_array) {
        let match_count = one_of
            .iter()
            .filter(|sub| validate_at_path(root_schema, sub, instance, path, depth + 1).is_ok())
            .count();
        if match_count != 1 {
            bail!(
                "Value at '{path}' matched {match_count} branches in 'oneOf' (expected exactly 1)"
            );
        }
    }

    if let Some(not_schema) = schema_obj.get("not") {
        if validate_at_path(root_schema, not_schema, instance, path, depth + 1).is_ok() {
            bail!("Value at '{path}' matched forbidden 'not' schema");
        }
    }

    Ok(())
}

fn matches_json_type(expected: &str, val: &Value) -> bool {
    match expected {
        "object" => val.is_object(),
        "array" => val.is_array(),
        "string" => val.is_string(),
        "boolean" => val.is_boolean(),
        "null" => val.is_null(),
        "integer" => {
            val.is_i64()
                || val.is_u64()
                || val
                    .as_f64()
                    .is_some_and(|f| f.is_finite() && f.fract() == 0.0)
        }
        "number" => val.is_number(),
        _ => false,
    }
}

fn json_type_name(val: &Value) -> &'static str {
    match val {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer"
            } else {
                "number"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Returns the default JSON Schema for a given `RoleName` (D14, D16).
pub fn default_role_json_schema(role: RoleName) -> Value {
    match role {
        RoleName::Init => json!({
            "type": "object",
            "required": ["subsystems"],
            "properties": {
                "subsystems": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id", "name", "summary", "tags"],
                        "properties": {
                            "id": { "type": "string", "minLength": 1 },
                            "name": { "type": "string", "minLength": 1 },
                            "summary": { "type": "string", "minLength": 1 },
                            "tags": {
                                "type": "array",
                                "items": { "type": "string" }
                            }
                        }
                    }
                }
            }
        }),
        RoleName::Update => json!({
            "type": "object",
            "required": ["id", "summary", "body"],
            "properties": {
                "id": { "type": "string", "minLength": 1 },
                "summary": { "type": "string", "minLength": 1 },
                "body": { "type": "string", "minLength": 1 },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        }),
        RoleName::Task => json!({
            "type": "object",
            "required": ["summary", "affected_subsystems"],
            "properties": {
                "summary": { "type": "string", "minLength": 1 },
                "affected_subsystems": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "key_changes": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        }),
        RoleName::Search => json!({
            "type": "object",
            "required": ["expanded_terms", "ranked_ids"],
            "properties": {
                "expanded_terms": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "ranked_ids": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        }),
    }
}
