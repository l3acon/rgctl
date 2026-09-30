//! Puppet CFG discover integration on ecommerce-puppet.

use std::path::PathBuf;
use std::process::Command;

fn puppet_bin() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_rgctl")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/rgctl"))
}

fn puppet_repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rgctl-tests/ecommerce-puppet")
}

#[test]
fn discover_with_cfg_indexes_puppet() {
    let repo = puppet_repo();
    if !repo.is_dir() {
        return;
    }
    let bin = puppet_bin();
    let _ = std::fs::remove_dir_all(repo.join(".rgctl"));
    let out = Command::new(&bin)
        .args(["discover", ".", "-l", "puppet", "--with-cfg"])
        .current_dir(&repo)
        .output()
        .expect("discover");
    assert!(
        out.status.success(),
        "discover --with-cfg failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cfg_index = repo.join(".rgctl/dashboard/cfg_index.json");
    if cfg_index.is_file() {
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&cfg_index).unwrap()).unwrap();
        assert_eq!(v["available"], true);
    }
}

#[test]
fn discover_with_ast_skeleton_on_puppet() {
    let repo = puppet_repo();
    if !repo.is_dir() {
        return;
    }
    let bin = puppet_bin();
    let _ = std::fs::remove_dir_all(repo.join(".rgctl"));
    let out = Command::new(&bin)
        .args([
            "discover",
            ".",
            "-l",
            "puppet",
            "--with-cfg",
            "--with-ast-skeleton",
        ])
        .current_dir(&repo)
        .output()
        .expect("discover");
    assert!(
        out.status.success(),
        "discover --with-ast-skeleton failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        repo.join(".rgctl").is_dir(),
        "expected .rgctl artifacts after skeleton discover"
    );
}
