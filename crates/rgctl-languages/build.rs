//! Build-time AST coverage drift check for bundled language plugins.
//!
//! Emits `cargo:warning=` when a tree-sitter grammar and `*-ast-coverage.json`
//! disagree. Set `RGCTL_AST_COVERAGE_STRICT=1` to fail the build instead.

use std::path::Path;

fn main() {
    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for path in rgctl_ast_coverage::rerun_if_changed_paths(&crates_dir) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-env-changed=RGCTL_AST_COVERAGE_STRICT");

    let issues = rgctl_ast_coverage::check_crates_dir(&crates_dir);
    let strict = std::env::var("RGCTL_AST_COVERAGE_STRICT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if let Err(e) = rgctl_ast_coverage::emit_cargo_warnings(&issues, strict) {
        panic!("{e}");
    }
}
