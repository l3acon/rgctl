//! Subprocess CLI checks for vuln triage / deps check.

use serde_json::Value;
use std::process::Command;

fn rgctl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rgctl"))
}

fn stdout_json(out: std::process::Output) -> Value {
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("json stdout")
}

#[test]
fn vuln_triage_json_schema() {
    let osv = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/osv/jackson-CVE-2022-42004.json"
    );
    let out = rgctl()
        .args(["-f", "json", "vuln", "triage", "--osv", osv])
        .output()
        .unwrap();
    let v = stdout_json(out);
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["command"], "vuln triage");
}

#[test]
fn deps_check_xalan_not_affected() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(
        repo.path().join("pom.xml"),
        r#"<?xml version="1.0"?><project><modelVersion>4.0.0</modelVersion>
        <groupId>a</groupId><artifactId>b</artifactId><version>1</version></project>"#,
    )
    .unwrap();
    let osv = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/osv/xalan-CVE-2022-34169.json"
    );
    let out = rgctl()
        .args([
            "-r",
            repo.path().to_str().unwrap(),
            "-f",
            "json",
            "deps",
            "check",
            "--osv",
            osv,
        ])
        .output()
        .unwrap();
    let v = stdout_json(out);
    assert_eq!(v["verdict"], "not_affected");
}
