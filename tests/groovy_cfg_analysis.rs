//! Groovy CFG via discover --with-cfg on langfeatures fixture.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn groovy_discover_with_cfg() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/groovy/langfeatures");
    let bin = std::env::var("CARGO_BIN_EXE_rgctl")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/rgctl"));
    let _ = std::fs::remove_dir_all(repo.join(".rgctl"));
    let out = Command::new(&bin)
        .args(["discover", ".", "-l", "groovy", "--with-cfg"])
        .current_dir(&repo)
        .output()
        .expect("discover");
    assert!(
        out.status.success(),
        "discover failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(repo.join(".rgctl").is_dir());
}
