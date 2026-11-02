//! Test classification and target grouping rules.
//!
//! Provides unified detection of test files and test directories across ecosystems
//! (Rust, Kotlin/Java, TypeScript/JavaScript, Python, Go) and prevents atomization
//! of test suites into individual L1 subsystems.

/// Whether a project-relative path is test code: a test directory segment
/// (`tests`, `test`, `commonTest`, `jvmTest`, `jsTest`, `__tests__`, `e2e`, `spec`, `specs`),
/// or a test file name pattern (`*Test.kt`, `*Tests.kt`, `*Spec.kt`, `*Test.java`,
/// `*TestCase.java`, `*.test.*`, `*.spec.*`, `*_test.go`, `tests.rs`, `*_test.rs`,
/// `*_tests.rs`, `test_*.py`, `*_test.py`).
pub fn is_test_path(rel: &str) -> bool {
    let normalized = rel.replace('\\', "/");
    let mut segments: Vec<&str> = normalized.split('/').collect();
    let name = segments.pop().unwrap_or("");

    // Directory segments
    let has_test_segment = segments.iter().any(|&s| {
        matches!(
            s,
            "tests"
                | "test"
                | "commonTest"
                | "jvmTest"
                | "jsTest"
                | "__tests__"
                | "e2e"
                | "spec"
                | "specs"
        )
    });
    if has_test_segment {
        return true;
    }

    // Filename patterns
    if name == "tests.rs"
        || name.ends_with("_test.rs")
        || name.ends_with("_tests.rs")
        || name.ends_with("_test.go")
        || name.ends_with("_test.py")
        || (name.starts_with("test_") && name.ends_with(".py"))
        || name.ends_with("Test.kt")
        || name.ends_with("Tests.kt")
        || name.ends_with("Spec.kt")
        || name.ends_with("Test.java")
        || name.ends_with("TestCase.java")
    {
        return true;
    }

    // Patterns like *.test.* and *.spec.* (e.g. button.test.tsx, button.spec.js)
    if is_test_or_spec_infix(name) {
        return true;
    }

    false
}

fn is_test_or_spec_infix(name: &str) -> bool {
    // Check for .test. or .spec. infix (e.g., button.test.tsx, button.spec.js)
    // Note: conftest.py contains "test.", but not ".test."
    name.contains(".test.") || name.contains(".spec.")
}

/// Extracts the test target name from a path (e.g. `commonTest` -> `"common"`, `jvmTest` -> `"jvm"`).
pub fn test_target(rel: &str) -> Option<&'static str> {
    let normalized = rel.replace('\\', "/");
    for seg in normalized.split('/') {
        match seg {
            "commonTest" => return Some("common"),
            "jvmTest" => return Some("jvm"),
            "jsTest" => return Some("js"),
            "iosTest" => return Some("ios"),
            "androidTest" | "androidUnitTest" => return Some("android"),
            _ => {}
        }
    }
    None
}

/// Returns the module stem for a test path or test directory:
/// `"tests_<target>"` if a specific target is detected, or `"tests"`.
pub fn test_group_stem(rel: &str) -> String {
    if let Some(target) = test_target(rel) {
        format!("tests_{target}")
    } else {
        "tests".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_extraction() {
        assert_eq!(
            test_target("core/src/commonTest/kotlin/Test.kt"),
            Some("common")
        );
        assert_eq!(test_target("core/src/jvmTest/kotlin/Test.kt"), Some("jvm"));
        assert_eq!(test_target("core/src/jsTest/kotlin/Test.kt"), Some("js"));
        assert_eq!(test_target("core/src/iosTest/kotlin/Test.kt"), Some("ios"));
        assert_eq!(
            test_target("core/src/androidTest/kotlin/Test.kt"),
            Some("android")
        );
        assert_eq!(test_target("core/src/test/java/Test.java"), None);
        assert_eq!(test_target("tests/cli.rs"), None);
    }

    #[test]
    fn test_group_stem_mapping() {
        assert_eq!(
            test_group_stem("core/src/commonTest/kotlin/Test.kt"),
            "tests_common"
        );
        assert_eq!(
            test_group_stem("core/src/jvmTest/kotlin/Test.kt"),
            "tests_jvm"
        );
        assert_eq!(test_group_stem("tests/cli.rs"), "tests");
        assert_eq!(test_group_stem("src/foo_test.rs"), "tests");
    }
}
