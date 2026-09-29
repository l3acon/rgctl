//! Kotlin language-feature GQL gates.
//!
//! Fixture: `tests/fixtures/kotlin/langfeatures`
//!
//! ```bash
//! cargo build --release --bin rgctl
//! cargo test --test kotlin_langfeatures -- --nocapture
//! ```

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/kotlin/langfeatures")
}

fn bin() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_rgctl") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/rgctl")
}

fn ensure_discovered() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let repo = repo();
        assert!(repo.is_dir(), "missing fixture {}", repo.display());
        let _ = std::fs::remove_dir_all(repo.join(".rgctl"));
        let out = Command::new(bin())
            .args(["discover", ".", "-l", "kotlin", "--with-cfg"])
            .current_dir(&repo)
            .output()
            .expect("run discover");
        assert!(
            out.status.success(),
            "discover failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    });
}

fn gql_json(query: &str) -> Value {
    ensure_discovered();
    let out = Command::new(bin())
        .args(["gql", "-f", "json", query])
        .current_dir(repo())
        .output()
        .expect("gql");
    assert!(
        out.status.success(),
        "gql failed: {}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let start = stdout.find('{').expect("json object");
    serde_json::from_str(&stdout[start..]).expect("parse gql json")
}

fn row_count(v: &Value) -> usize {
    v.get("count")
        .and_then(|c| c.as_u64())
        .or_else(|| v.get("rows").and_then(|r| r.as_array()).map(|a| a.len() as u64))
        .unwrap_or(0) as usize
}

#[test]
fn kotlin_class_and_method_indexed() {
    let v = gql_json(
        "MATCH (n:Class) WHERE n.qualified_name = 'demo.OrderService' RETURN n LIMIT 5",
    );
    assert!(row_count(&v) >= 1, "OrderService class: {v}");
    let v = gql_json(
        "MATCH (n:Function) WHERE n.qualified_name = 'demo.OrderService.find' RETURN n LIMIT 5",
    );
    assert!(row_count(&v) >= 1, "find method: {v}");
}

#[test]
fn kotlin_calls_non_zero() {
    let v = gql_json("MATCH (a)-[:Calls]->(b) RETURN a, b LIMIT 20");
    assert!(row_count(&v) >= 1, "expected Calls edges: {v}");
}

#[test]
fn kotlin_implements_repository() {
    let impls = gql_json(
        "MATCH (a)-[:Implements]->(b) WHERE a.name = 'OrderService' RETURN a, b LIMIT 10",
    );
    let extends = gql_json(
        "MATCH (a)-[:Extends]->(b) WHERE a.name = 'OrderService' RETURN a, b LIMIT 10",
    );
    assert!(
        row_count(&impls) + row_count(&extends) >= 1,
        "expected Extends/Implements: implements={impls} extends={extends}"
    );
}

#[test]
fn kotlin_fixture_path_exists() {
    assert!(Path::new(&repo()).join("src/LangFeatures.kt").is_file());
}
