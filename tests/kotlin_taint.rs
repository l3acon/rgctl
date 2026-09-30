//! Kotlin taint integration (pattern coverage in `rgctl-analysis`).

use rgctl::analysis::{canonical_language_id, cfg_language_id_from_path};
use std::path::Path;

#[test]
fn kotlin_canonical_language_id() {
    assert_eq!(canonical_language_id("kt"), Some("kotlin"));
    assert_eq!(
        cfg_language_id_from_path(Path::new("src/main/kotlin/App.kt")),
        Some("kotlin")
    );
}

#[test]
fn kotlin_taint_integration_reexport() {
    // Covered by `rgctl-analysis` `test_kotlin_taint_http_to_sql_patterns`.
}
