//! JavaScript extraction-depth GQL gates on `rgctl-tests/ecommerce-javascript`.
//!
//! ```bash
//! cargo build --release -p rgctl
//! cargo test --test javascript_langfeatures -- --nocapture
//! ```

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rgctl-tests/ecommerce-javascript")
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
            .args(["discover", ".", "-l", "javascript"])
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

fn gql(repo: &Path, query: &str) -> Value {
    let out = Command::new(bin())
        .args(["-f", "json", "gql", query])
        .current_dir(repo)
        .output()
        .expect("gql");
    assert!(
        out.status.success(),
        "gql failed: {}\n{}",
        query,
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("json")
}

fn node_count(repo: &Path, label: &str) -> usize {
    let q = format!("MATCH (n:{label}) RETURN n LIMIT 10000");
    gql(repo, &q)
        .get("count")
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize
}

fn edge_count(repo: &Path, rel: &str) -> usize {
    let q = format!("MATCH (a)-[:{rel}]->(b) RETURN a,b LIMIT 10000");
    gql(repo, &q)
        .get("count")
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize
}

#[test]
fn javascript_ecommerce_import_nonzero() {
    ensure_discovered();
    let n = node_count(&repo(), "Import");
    assert!(n > 0, "expected Import nodes on ecommerce-javascript, got {n}");
}

#[test]
fn javascript_ecommerce_extends_nonzero() {
    ensure_discovered();
    let n = edge_count(&repo(), "EXTENDS");
    assert!(n > 0, "expected Extends edges on ecommerce-javascript, got {n}");
}

#[test]
fn javascript_named_arrows_present() {
    ensure_discovered();
    let names = all_function_names(&repo());
    for expected in ["arrowAdd", "arrowHelper", "declaredAdd", "fetchAll"] {
        assert!(
            names.iter().any(|n| n == expected),
            "missing Function `{expected}` in {names:?}"
        );
    }
}

#[test]
fn javascript_case_c1_no_functions() {
    ensure_discovered();
    let names = functions_in_file(&repo(), "caseC1.js");
    assert!(
        names.is_empty(),
        "caseC1 (const-only) must emit 0 Function nodes, got {names:?}"
    );
}

#[test]
fn javascript_case_declarations_have_no_anonymous_duplicates() {
    ensure_discovered();
    assert_eq!(
        functions_in_file(&repo(), "caseC2.js"),
        vec!["gamma".to_string()]
    );
    assert_eq!(
        functions_in_file(&repo(), "caseA.js"),
        vec!["alpha".to_string()]
    );
    let a2 = functions_in_file(&repo(), "caseA2.js");
    assert_eq!(a2.len(), 2, "{a2:?}");
    assert!(a2.contains(&"alpha2".to_string()) && a2.contains(&"beta".to_string()), "{a2:?}");
    assert!(a2.iter().all(|n| !n.starts_with("anonymous")), "{a2:?}");
}

#[test]
fn javascript_case_b_named_arrows_and_no_decl_duplicates() {
    ensure_discovered();
    let one = functions_in_file(&repo(), "caseB_one.js");
    for expected in ["dup", "uniqueOne", "dupDecl", "uniqueDecl"] {
        assert!(one.iter().any(|n| n == expected), "missing {expected} in {one:?}");
    }
    assert_eq!(one.len(), 4, "{one:?}");
    assert_eq!(functions_in_file(&repo(), "caseB_two.js").len(), 4);
}

fn all_function_names(repo: &Path) -> Vec<String> {
    function_rows(repo)
        .into_iter()
        .map(|(_, name)| name)
        .collect()
}

fn functions_in_file(repo: &Path, file_suffix: &str) -> Vec<String> {
    let mut names: Vec<String> = function_rows(repo)
        .into_iter()
        .filter(|(file, _)| file.replace('\\', "/").ends_with(file_suffix))
        .map(|(_, name)| name)
        .collect();
    names.sort();
    names
}

fn function_rows(repo: &Path) -> Vec<(String, String)> {
    let v = gql(repo, "MATCH (n:Function) RETURN n LIMIT 10000");
    let mut out = Vec::new();
    if let Some(rows) = v.get("rows").and_then(|r| r.as_array()) {
        for row in rows {
            let cells = row.as_array().map(|a| a.as_slice()).unwrap_or(std::slice::from_ref(row));
            for cell in cells {
                let name = cell
                    .get("node")
                    .and_then(|n| n.as_str())
                    .or_else(|| cell.get("name").and_then(|n| n.as_str()))
                    .or_else(|| {
                        cell.get("n")
                            .and_then(|n| n.get("name"))
                            .and_then(|n| n.as_str())
                    });
                let file = cell
                    .get("file")
                    .and_then(|f| f.as_str())
                    .or_else(|| {
                        cell.get("n")
                            .and_then(|n| n.get("file"))
                            .and_then(|f| f.as_str())
                    })
                    .unwrap_or("");
                if let Some(name) = name {
                    out.push((file.to_string(), name.to_string()));
                }
            }
        }
    }
    out
}
