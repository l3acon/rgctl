//! Release-binary smoke for `vuln triage` / `deps check` (AGENTS.md: release for timing claims).
//!
//! ```bash
//! cargo build --release --bin rgctl
//! cargo test --release --test osv_deps_release_smoke -- --ignored --nocapture
//! ```

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn rgctl_bin() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_rgctl") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/rgctl")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/osv")
        .join(name)
}

fn run_json(args: &[&str]) -> Value {
    let bin = rgctl_bin();
    assert!(
        bin.is_file(),
        "missing {}; run cargo build --release --bin rgctl",
        bin.display()
    );
    let out = Command::new(&bin)
        .args(args)
        .output()
        .expect("spawn rgctl");
    assert!(
        out.status.success(),
        "rgctl failed {:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // progress lines may interleave on stderr only; stdout should be JSON
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("json parse {e}: {stdout}")
    })
}

#[test]
#[ignore = "requires release binary; run with --ignored after cargo build --release --bin rgctl"]
fn release_vuln_triage_jackson() {
    let osv = fixture("jackson-CVE-2022-42004.json");
    let v = run_json(&[
        "-f",
        "json",
        "vuln",
        "triage",
        "--osv",
        osv.to_str().unwrap(),
    ]);
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["command"], "vuln triage");
    assert!(v["packages"][0]["affected_methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m.as_str().unwrap().contains("readValue")));
}

#[test]
#[ignore = "requires release binary + example/coolstore"]
fn release_deps_coolstore_scenarios() {
    let cool = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example/coolstore");
    assert!(cool.join("pom.xml").is_file(), "missing coolstore fixture");
    let xalan = fixture("xalan-CVE-2022-34169.json");
    let jackson = fixture("jackson-CVE-2022-42004.json");

    let a = run_json(&[
        "-r",
        cool.to_str().unwrap(),
        "-f",
        "json",
        "deps",
        "check",
        "--osv",
        xalan.to_str().unwrap(),
    ]);
    assert_eq!(a["verdict"], "not_affected");

    let b = run_json(&[
        "-r",
        cool.to_str().unwrap(),
        "-f",
        "json",
        "deps",
        "check",
        "--osv",
        jackson.to_str().unwrap(),
        "--include-jars",
        "lib",
    ]);
    assert_eq!(b["verdict"], "affected_candidate");
    assert!(b["matched"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["in_range"] == true));
}

#[test]
#[ignore = "requires release binary"]
fn release_deps_npm_fixture() {
    // Synthetic npm tree under a temp dir written by the test process via rgctl -r
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"app","version":"1.0.0","dependencies":{"lodash":"4.17.20"}}"#,
    )
    .unwrap();
    // OSV-shaped npm advisory (minimal)
    let osv = root.join("lodash-osv.json");
    std::fs::write(
        &osv,
        r#"{
  "schema_version": "1.6.8",
  "id": "TEST-lodash",
  "modified": "2026-01-01T00:00:00Z",
  "affected": [{
    "package": {"ecosystem": "npm", "name": "lodash", "purl": "pkg:npm/lodash"},
    "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "0"}, {"fixed": "4.17.21"}]}]
  }]
}"#,
    )
    .unwrap();
    let v = run_json(&[
        "-r",
        root.to_str().unwrap(),
        "-f",
        "json",
        "deps",
        "check",
        "--osv",
        osv.to_str().unwrap(),
    ]);
    // package.json declares ^-style? we used exact 4.17.20 in dependencies value —
    // extract may keep "4.17.20" as requirement
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["command"], "deps check");
    assert_eq!(v["verdict"], "affected_candidate");
}
