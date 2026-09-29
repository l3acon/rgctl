//! Groovy taint integration (pattern coverage in `rgctl-analysis`).

use rgctl::analysis::{canonical_language_id, cfg_language_id_from_path};
use std::path::Path;

#[test]
fn groovy_canonical_language_id() {
    assert_eq!(canonical_language_id("groovy"), Some("groovy"));
    assert_eq!(
        cfg_language_id_from_path(Path::new("scripts/Job.groovy")),
        Some("groovy")
    );
}

#[test]
fn groovy_taint_integration_reexport() {
    // Covered by `rgctl-analysis` `test_groovy_taint_http_to_sql_patterns`.
}
