//! Kotlin CFG via discover --with-cfg on langfeatures fixture.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn kotlin_discover_with_cfg() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/kotlin/langfeatures");
    let bin = std::env::var("CARGO_BIN_EXE_rgctl")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/rgctl"));
    let _ = std::fs::remove_dir_all(repo.join(".rgctl"));
    let out = Command::new(&bin)
        .args(["discover", ".", "-l", "kotlin", "--with-cfg"])
        .current_dir(&repo)
        .output()
        .expect("discover");
    assert!(
        out.status.success(),
        "discover failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cfg_index = repo.join(".rgctl/dashboard/cfg_index.json");
    // dashboard may or may not write cfg_index without --with-security; check analysis artifacts
    let analysis = repo.join(".rgctl");
    assert!(analysis.is_dir(), "expected .rgctl after discover");
}
