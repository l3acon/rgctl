//! Puppet CFG discover integration on ecommerce-puppet.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn puppet_bin() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_rgctl")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/rgctl"))
}

fn puppet_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rgctl-tests/ecommerce-puppet")
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            if entry.file_name() == ".rgctl" || entry.file_name() == "target" {
                continue;
            }
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Isolated copy so parallel discovers do not race on shared `rgctl-tests/…/.rgctl`.
fn materialize_puppet() -> Option<(tempfile::TempDir, PathBuf)> {
    let src = puppet_fixture();
    if !src.is_dir() {
        return None;
    }
    let tmp = tempfile::tempdir().ok()?;
    let repo = tmp.path().join("ecommerce-puppet");
    copy_dir_all(&src, &repo).ok()?;
    Some((tmp, repo))
}

#[test]
fn discover_with_cfg_indexes_puppet() {
    let Some((_tmp, repo)) = materialize_puppet() else {
        return;
    };
    let bin = puppet_bin();
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
    let Some((_tmp, repo)) = materialize_puppet() else {
        return;
    };
    let bin = puppet_bin();
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
