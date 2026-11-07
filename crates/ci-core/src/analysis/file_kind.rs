//! Canonical extension and manifest classifier for `scan`, `scope`, and `detector` (D9, S5).

use std::path::Path;

use crate::analysis::detector::CODE_EXTENSIONS;

const TEMPLATE_WRAPPERS: &[&str] = &["tmpl", "j2", "jinja", "jinja2", "gotmpl", "in", "example"];

/// Returns the canonical extension or manifest key for a file path.
///
/// 1. Standard manifests with version/flavor suffixes:
///    - `Dockerfile`, `Dockerfile.*`, `Containerfile`, `Containerfile.*` -> `"dockerfile"`
///    - `Makefile`, `Makefile.*`, `GNUmakefile`, `GNUmakefile.*` -> `"makefile"`
///    - `Justfile`, `justfile`, `Justfile.*`, `justfile.*`, `*.just` -> `"just"`
///    - `CMakeLists.txt` -> `"cmake"`
///    - `Modelfile`, `Modelfile.*` -> `"modelfile"`
/// 2. Compound template suffixes (e.g. `*.yaml.j2`, `*.toml.tmpl`):
///    - If `<inner>` is in [`CODE_EXTENSIONS`], returns `"<inner>.<wrapper>"`.
///    - Otherwise returns `"<wrapper>"`.
/// 3. Normal files:
///    - Lowercase last extension, or `None` if extensionless.
pub fn canonical_file_ext(rel_path: &str) -> Option<String> {
    let path = Path::new(rel_path);
    let file_name = path.file_name()?.to_str()?;
    let name_lower = file_name.to_ascii_lowercase();

    // 1. Manifests
    if name_lower == "dockerfile"
        || name_lower.starts_with("dockerfile.")
        || name_lower == "containerfile"
        || name_lower.starts_with("containerfile.")
    {
        return Some("dockerfile".to_string());
    }
    if name_lower == "makefile"
        || name_lower.starts_with("makefile.")
        || name_lower == "gnumakefile"
        || name_lower.starts_with("gnumakefile.")
    {
        return Some("makefile".to_string());
    }
    if name_lower == "justfile"
        || name_lower.starts_with("justfile.")
        || name_lower.ends_with(".just")
    {
        return Some("just".to_string());
    }
    if name_lower == "cmakelists.txt" {
        return Some("cmake".to_string());
    }
    if name_lower == "modelfile" || name_lower.starts_with("modelfile.") {
        return Some("modelfile".to_string());
    }

    // 2. Compound templates (<stem>.<inner>.<wrapper>)
    let parts: Vec<&str> = file_name.split('.').collect();
    if parts.len() >= 3 {
        let wrapper = parts.last()?.to_ascii_lowercase();
        let inner = parts[parts.len() - 2].to_ascii_lowercase();
        if TEMPLATE_WRAPPERS.contains(&wrapper.as_str()) {
            if CODE_EXTENSIONS.contains(&inner.as_str()) {
                return Some(format!("{inner}.{wrapper}"));
            }
            return Some(wrapper);
        }
    }

    // 3. Normal extension
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_file_ext_manifests_and_templates() {
        assert_eq!(
            canonical_file_ext("Dockerfile.prod"),
            Some("dockerfile".to_string())
        );
        assert_eq!(
            canonical_file_ext("deploy/Containerfile.v1"),
            Some("dockerfile".to_string())
        );
        assert_eq!(
            canonical_file_ext("Modelfile.qwen2.5-ctx64k"),
            Some("modelfile".to_string())
        );
        assert_eq!(
            canonical_file_ext("deploy/config.yaml.j2"),
            Some("yaml.j2".to_string())
        );
        assert_eq!(
            canonical_file_ext("configs/app.toml.tmpl"),
            Some("toml.tmpl".to_string())
        );
        assert_eq!(
            canonical_file_ext("notes.unknown.tmpl"),
            Some("tmpl".to_string())
        );
        assert_eq!(canonical_file_ext("LICENSE"), None);
        assert_eq!(
            canonical_file_ext("Makefile.dist"),
            Some("makefile".to_string())
        );
        assert_eq!(
            canonical_file_ext("CMakeLists.txt"),
            Some("cmake".to_string())
        );
    }
}
