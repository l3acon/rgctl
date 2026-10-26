//! Phase 12.1 integration: CFG, PDG, and backward slicing.
#![allow(dead_code, unused_imports, unused_macros)]

mod rgctl_harness;

use rgctl::analysis::{
    BackwardSlicer, CfgPdgArchive, CfgPdgArchiveMmap, ProgramDependenceGraph, SliceCriterion,
    build_cfg_for_function, stable_record_uuid,
};
use rgctl::graph::{SnapshotNodeStore, hash_code};
use rgctl_harness::{assert_ok, assert_rgctl_snapshot, run_in_repo, run_in_repo_json};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_rust_backward_slice_excludes_dead_assignments() {
    let code = r#"
fn process(input: String) -> String {
    let a = 10;
    let b = 20;
    let x = input.len();
    let y = x * 2;
    format!("{}", y)
}
"#;
    let cfg = build_cfg_for_function("rust", code, "process").unwrap();
    let pdg = ProgramDependenceGraph::build(&cfg, code.as_bytes()).unwrap();
    let slicer = BackwardSlicer::new(&pdg, &cfg);

    let y_line = pdg
        .nodes
        .values()
        .find(|n| n.defined_vars.contains("y"))
        .unwrap()
        .statement
        .line;

    let slice = slicer
        .slice(SliceCriterion {
            variable: "y".to_string(),
            line: y_line,
        })
        .unwrap();

    let a_line = pdg
        .nodes
        .values()
        .find(|n| n.defined_vars.contains("a"))
        .unwrap()
        .statement
        .line;

    assert!(!slice.lines.contains(&a_line));
    assert!(slice.lines.contains(&y_line));
    assert!(slice.reduction_percent > 0.0);
}
#[test]
fn test_cfg_has_entry_and_exit() {
    let code = r#"
fn simple() {
    let x = 1;
}
"#;
    let cfg = build_cfg_for_function("rust", code, "simple").unwrap();
    assert!(!cfg.exits.is_empty());
    assert!(!cfg.blocks.is_empty());
    assert!(cfg.blocks.contains_key(&cfg.entry));
}
#[test]
fn test_pdg_builds_data_dependencies() {
    let code = r#"
fn flow() {
    let x = 1;
    let y = x + 1;
    let _ = y;
}
"#;
    let cfg = build_cfg_for_function("rust", code, "flow").unwrap();
    let pdg = ProgramDependenceGraph::build(&cfg, code.as_bytes()).unwrap();
    assert!(!pdg.data_deps.is_empty());
    assert!(!pdg.nodes.is_empty());
}
#[test]
fn test_python_cfg_builds() {
    let code = r#"
def process(input_val):
    a = 10
    x = len(input_val)
    y = x * 2
    return y
"#;
    let cfg = build_cfg_for_function("python", code, "process").unwrap();
    assert!(!cfg.blocks.is_empty());
}

/// Discover without `--with-cfg`, then lazy `slice` builds/caches a single CFG/PDG record.
#[test]
fn lazy_slice_without_with_cfg_builds_and_caches_single_function() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path();

    let process_order = r#"
pub fn process_order(input: String) -> String {
    let a = 10;
    let b = 20;
    let x = input.len();
    let y = x * 2;
    format!("{y}")
}

pub fn helper_a() -> i32 { 1 }
pub fn helper_b() -> i32 { 2 }
"#;
    fs::write(repo.join("a.rs"), process_order).unwrap();
    fs::write(
        repo.join("b.rs"),
        r#"
pub fn billing() -> i32 { 3 }
pub fn invoice() -> i32 { 4 }
pub fn refund() -> i32 { 5 }
"#,
    )
    .unwrap();
    fs::write(
        repo.join("c.rs"),
        r#"
pub fn ship() -> i32 { 6 }
pub fn track() -> i32 { 7 }
pub fn deliver() -> i32 { 8 }
pub fn cancel() -> i32 { 9 }
"#,
    )
    .unwrap();

    assert_ok(
        &run_in_repo(repo, &["discover", ".", "--languages", "rust"]),
        "discover without --with-cfg",
    );
    assert_rgctl_snapshot(repo);

    let archive_path = CfgPdgArchive::default_path(repo);
    assert!(
        !archive_path.is_file()
            || CfgPdgArchiveMmap::open(&archive_path)
                .map(|v| v.is_empty())
                .unwrap_or(true),
        "cfg archive must be missing or empty before lazy slice"
    );

    let snap = SnapshotNodeStore::open(&repo.join(".rgctl/graph.snapshot.bin")).unwrap();
    let digest_before = snap.content_digest().unwrap().to_string();

    // Line of `let y = x * 2` inside process_order (1-indexed source lines).
    let y_line = process_order
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("let y ="))
        .map(|(i, _)| i + 1)
        .expect("y line");

    let out = run_in_repo_json(
        repo,
        &[
            "slice",
            "a.rs",
            "--line",
            &y_line.to_string(),
            "--variable",
            "y",
            "--function",
            "process_order",
        ],
    );
    assert_ok(&out, "lazy slice");

    assert!(
        archive_path.is_file(),
        "lazy slice must create {}",
        archive_path.display()
    );
    let view = CfgPdgArchiveMmap::open(&archive_path).expect("open cfg archive");
    assert_eq!(
        view.len(),
        1,
        "archive must hold only the sliced function, got ids={:?}",
        view.function_ids()
    );
    let record = view
        .load_record(view.function_ids()[0])
        .unwrap()
        .expect("primary record");
    assert_eq!(record.function_name, "process_order");

    let alias = stable_record_uuid(
        record.file_path.as_deref().unwrap_or("a.rs"),
        &record.function_name,
        &record.code_hash,
    );
    assert!(
        view.contains(record.function_id) || view.contains(alias),
        "TOC must resolve primary or stable alias"
    );
    assert!(
        view.contains(alias),
        "stable_record_uuid must hit TOC via binary search"
    );

    let snap_after = SnapshotNodeStore::open(&repo.join(".rgctl/graph.snapshot.bin")).unwrap();
    let digest_after = snap_after.content_digest().unwrap().to_string();
    assert_eq!(
        digest_before, digest_after,
        "lazy slice must not rewrite graph.snapshot.bin digest"
    );

    // Source hash used by ensure_cfg_pdg matches the file contents we wrote.
    let source = fs::read_to_string(repo.join("a.rs")).unwrap();
    assert_eq!(record.code_hash, hash_code(&source));
}

#[test]
fn block_shadow_slice_from_outer_result_excludes_inner_binding() {
    let code = r#"
fn shadow_test(input: i32) -> i32 {
    let x = input + 1;
    {
        let x = 999;
        let _unused = x + 2;
    }
    let result = x * 2;
    result
}
"#;
    let cfg = build_cfg_for_function("rust", code, "shadow_test").unwrap();
    let pdg = ProgramDependenceGraph::build(&cfg, code.as_bytes()).unwrap();

    let result_line = pdg
        .nodes
        .values()
        .find(|n| n.defined_vars.iter().any(|d| d == "result"))
        .map(|n| n.statement.line)
        .expect("result def");
    let outer_x_line = pdg
        .nodes
        .values()
        .find(|n| n.defined_vars.iter().any(|d| d == "x") && n.statement.text.contains("input + 1"))
        .map(|n| n.statement.line)
        .expect("outer x");
    let inner_x_line = pdg
        .nodes
        .values()
        .find(|n| n.statement.text.contains("999"))
        .map(|n| n.statement.line);
    let unused_line = pdg
        .nodes
        .values()
        .find(|n| n.statement.text.contains("_unused"))
        .map(|n| n.statement.line);

    let slice = BackwardSlicer::new(&pdg, &cfg)
        .slice(SliceCriterion {
            variable: "result".into(),
            line: result_line,
        })
        .expect("slice result");

    assert!(
        slice.lines.contains(&outer_x_line),
        "outer let x = input + 1 must be in slice; lines={:?}",
        slice.lines
    );
    if let Some(inner) = inner_x_line {
        assert!(
            !slice.lines.contains(&inner),
            "inner let x = 999 must be excluded; lines={:?}",
            slice.lines
        );
    }
    if let Some(unused) = unused_line {
        assert!(
            !slice.lines.contains(&unused),
            "inner _unused must be excluded; lines={:?}",
            slice.lines
        );
    }
}

#[test]
fn self_referential_let_x_eq_x_plus_one_reaches_prior_x() {
    let code = r#"
fn rebind() {
    let x = 1;
    let x = x + 1;
    let y = x;
    let _ = y;
}
"#;
    let cfg = build_cfg_for_function("rust", code, "rebind").unwrap();
    let pdg = ProgramDependenceGraph::build(&cfg, code.as_bytes()).unwrap();

    let first = pdg
        .nodes
        .values()
        .find(|n| n.defined_vars.iter().any(|d| d == "x") && n.used_vars.is_empty())
        .expect("first let x = 1");
    let second = pdg
        .nodes
        .values()
        .find(|n| {
            n.statement.text.contains("x + 1")
                && n.used_vars.iter().any(|u| u == "x" || u.starts_with("x#"))
        })
        .expect("let x = x + 1");

    assert!(
        pdg.data_deps
            .iter()
            .any(|d| d.from == first.id && d.to == second.id),
        "rebind RHS must read prior x; deps={:?}",
        pdg.data_deps
    );

    let y_line = pdg
        .nodes
        .values()
        .find(|n| n.defined_vars.iter().any(|d| d == "y"))
        .map(|n| n.statement.line)
        .expect("y");
    let slice = BackwardSlicer::new(&pdg, &cfg)
        .slice(SliceCriterion {
            variable: "y".into(),
            line: y_line,
        })
        .expect("slice y");
    assert!(
        slice.lines.contains(&first.statement.line),
        "slice from y must reach prior x=1; lines={:?}",
        slice.lines
    );
}
