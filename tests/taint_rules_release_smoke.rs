//! Release-binary smoke: declarative taint packs load under `discover --with-taint`.
//!
//! ```bash
//! cargo build --release --bin rgctl
//! cargo test --release --test taint_rules_release_smoke -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::process::Command;

fn rgctl_bin() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_rgctl") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/rgctl")
}

#[test]
#[ignore = "requires release binary: cargo build --release --bin rgctl"]
fn discover_with_taint_loads_builtin_packs() {
    let bin = rgctl_bin();
    assert!(
        bin.is_file(),
        "missing {}; run cargo build --release --bin rgctl",
        bin.display()
    );
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/taint_smoke");
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::copy(fixture.join("app.py"), root.join("app.py")).unwrap();

    let out = Command::new(&bin)
        .current_dir(&root)
        .args([
            "discover",
            ".",
            "-l",
            "python",
            "--with-cfg",
            "--with-taint",
            "-f",
            "json",
        ])
        .output()
        .expect("spawn rgctl");
    assert!(
        out.status.success(),
        "discover --with-taint failed: {}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let analysis = root.join(".rgctl").join("analysis");
    assert!(
        analysis.is_dir(),
        "expected .rgctl/analysis after --with-taint"
    );
}
