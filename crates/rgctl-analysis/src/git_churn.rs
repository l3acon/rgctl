//! Recency-weighted git churn and `CoChangesWith` temporal coupling.
//!
//! Opt-in via `rgctl discover --with-git`. Streams a single bounded `git log
//! --numstat` subprocess (no `libgit2`). Mega-commits and Jaccard/top-K filters
//! keep the co-change matrix sparse. `CoChangesWith` is intentionally excluded
//! from call-traversal and behavioral centrality defaults.

use rgctl_error::{Error, Result};
use rgctl_graph::backend::MemoryBackend;
use rgctl_graph::schema::{Edge, EdgeType, NodeType};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::warn;
use uuid::Uuid;

/// Configuration for the git churn / co-change pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitChurnConfig {
    /// Maximum commits to scan (default: 500).
    pub max_commits: usize,
    /// Git `--since` window (default: `"90.days"`).
    pub since: String,
    /// Minimum shared commits for a `CoChangesWith` edge (default: 3).
    pub min_cochange_count: u32,
    /// Minimum Jaccard similarity `cochange / (c_a + c_b - cochange)` (default: 0.25).
    pub min_jaccard: f32,
    /// Skip mega-commits touching more than this many indexed files (default: 50).
    pub max_files_per_commit: usize,
    /// Maximum `CoChangesWith` edges emitted per file (default: 15).
    pub max_edges_per_file: usize,
}

impl Default for GitChurnConfig {
    fn default() -> Self {
        Self {
            max_commits: 500,
            since: "90.days".to_string(),
            min_cochange_count: 3,
            min_jaccard: 0.25,
            max_files_per_commit: 50,
            max_edges_per_file: 15,
        }
    }
}

/// Per-file churn / hotspot metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileChurnMetrics {
    /// Repo-relative file path.
    pub file_path: String,
    /// Graph node id for the [`NodeType::File`] node.
    pub file_node_id: Uuid,
    /// Commits touching this file (non-mega only).
    pub commit_count: u32,
    /// Lines added across scanned commits.
    pub lines_added: u64,
    /// Lines deleted across scanned commits.
    pub lines_deleted: u64,
    /// Latest commit unix timestamp touching this file.
    pub last_modified_ts: i64,
    /// Recency-weighted churn: `sum ln(1+added+deleted) * exp(-λ * age_days)`.
    pub recency_churn_score: f64,
    /// Max cyclomatic complexity among functions in this file (default 1).
    pub max_cyclomatic: u32,
    /// `recency_churn_score * ln(1 + max_cyclomatic)`.
    pub hotspot_score: f64,
}

/// Summary of a git churn pass.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitChurnReport {
    /// Per-file metrics for files that appeared in at least one non-mega commit.
    pub files: Vec<FileChurnMetrics>,
    /// Number of directed `CoChangesWith` edges inserted (includes both directions).
    pub edges_emitted: usize,
    /// Commits seen in the `git log` stream (including mega / empty).
    pub commits_scanned: usize,
    /// Commits that contributed to churn / co-change (non-empty, non-mega).
    pub commits_used: usize,
    /// Indexed file nodes considered.
    pub files_indexed: usize,
}

/// Half-life of 30 days → `λ = ln(2) / 30`.
const LAMBDA: f64 = std::f64::consts::LN_2 / 30.0;

/// Stream git history, stamp churn properties, and emit `CoChangesWith` edges.
///
/// On missing `git` or non-zero exit, logs a warning and returns
/// [`GitChurnReport::default`] so discover is not failed.
pub fn compute_and_apply_git_churn(
    repo_root: &Path,
    backend: &mut MemoryBackend,
    config: &GitChurnConfig,
) -> Result<GitChurnReport> {
    let nodes = backend.all_nodes()?;
    let canon_root = repo_root.canonicalize().ok();
    let mut file_path_to_dense: FxHashMap<String, u32> = FxHashMap::default();
    let mut dense_to_uuid: Vec<Uuid> = Vec::new();
    let mut dense_paths: Vec<String> = Vec::new();
    let mut dense_max_cyclomatic: Vec<u32> = Vec::new();

    for node in &nodes {
        if node.node_type != NodeType::File {
            continue;
        }
        let raw = node
            .file_path
            .as_deref()
            .map(|s| s.as_ref())
            .unwrap_or(node.name.as_str());
        let path = normalize_node_repo_path(repo_root, canon_root.as_deref(), raw);
        if path.is_empty() || file_path_to_dense.contains_key(&path) {
            continue;
        }
        let idx = dense_to_uuid.len() as u32;
        file_path_to_dense.insert(path.clone(), idx);
        dense_to_uuid.push(node.id);
        dense_paths.push(path);
        dense_max_cyclomatic.push(1);
    }

    // One pass: max cyclomatic per file from Function / Method nodes.
    let mut functions_by_file: Vec<Vec<(Uuid, u32)>> = vec![Vec::new(); dense_to_uuid.len()];
    for node in &nodes {
        if node.node_type != NodeType::Function {
            continue;
        }
        let Some(fp) = node.file_path.as_deref() else {
            continue;
        };
        let path = normalize_node_repo_path(repo_root, canon_root.as_deref(), fp);
        let Some(&dense) = file_path_to_dense.get(&path) else {
            continue;
        };
        let cyclo = parse_u32_prop(node.get_property("cyclomatic"))
            .unwrap_or(1)
            .max(1);
        let d = dense as usize;
        if cyclo > dense_max_cyclomatic[d] {
            dense_max_cyclomatic[d] = cyclo;
        }
        functions_by_file[d].push((node.id, cyclo));
    }

    let files_indexed = dense_to_uuid.len();
    if files_indexed == 0 {
        return Ok(GitChurnReport {
            files_indexed: 0,
            ..GitChurnReport::default()
        });
    }

    let commits = match stream_git_commits(repo_root, config, &file_path_to_dense) {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "git churn skipped (git unavailable or not a repository)");
            return Ok(GitChurnReport {
                files_indexed,
                ..GitChurnReport::default()
            });
        }
    };

    let commits_scanned = commits.len();
    let t_now = commits.iter().map(|c| c.ts).max().unwrap_or(0);

    let n = files_indexed;
    let mut commits_count = vec![0u32; n];
    let mut lines_added = vec![0u64; n];
    let mut lines_deleted = vec![0u64; n];
    let mut last_ts = vec![0i64; n];
    let mut recency_churn = vec![0f64; n];
    let mut cochange_counts: FxHashMap<(u32, u32), u32> = FxHashMap::default();
    let mut commits_used = 0usize;

    for commit in &commits {
        let mut files = commit.files.clone();
        if files.is_empty() || files.len() > config.max_files_per_commit {
            // Mega / empty: skip co-change AND per-file churn accumulation.
            continue;
        }
        // Deduplicate by dense idx (sum line deltas) then sort ascending.
        files.sort_unstable_by_key(|(idx, _, _)| *idx);
        let mut dedup: Vec<(u32, u64, u64)> = Vec::with_capacity(files.len());
        for (idx, added, deleted) in files {
            if let Some(last) = dedup.last_mut() {
                if last.0 == idx {
                    last.1 = last.1.saturating_add(added);
                    last.2 = last.2.saturating_add(deleted);
                    continue;
                }
            }
            dedup.push((idx, added, deleted));
        }
        files = dedup;

        commits_used += 1;
        let age_days = ((t_now - commit.ts) as f64 / 86_400.0).max(0.0);
        let w_c = (-LAMBDA * age_days).exp();

        for &(u, added, deleted) in &files {
            let i = u as usize;
            commits_count[i] = commits_count[i].saturating_add(1);
            lines_added[i] = lines_added[i].saturating_add(added);
            lines_deleted[i] = lines_deleted[i].saturating_add(deleted);
            last_ts[i] = last_ts[i].max(commit.ts);
            recency_churn[i] += ((1 + added + deleted) as f64).ln() * w_c;
        }

        for i in 0..files.len() {
            for j in (i + 1)..files.len() {
                let u = files[i].0;
                let v = files[j].0;
                *cochange_counts.entry((u, v)).or_insert(0) += 1;
            }
        }
    }

    // Jaccard filter → per-file candidates → top-K.
    let mut per_file_candidates: Vec<Vec<(u32, u32, f32)>> = vec![Vec::new(); n];
    for ((u, v), count) in &cochange_counts {
        if *count < config.min_cochange_count {
            continue;
        }
        let cu = commits_count[*u as usize];
        let cv = commits_count[*v as usize];
        let denom = cu + cv - *count;
        if denom == 0 {
            continue;
        }
        let jaccard = (*count as f32) / (denom as f32);
        if jaccard < config.min_jaccard {
            continue;
        }
        per_file_candidates[*u as usize].push((*v, *count, jaccard));
        per_file_candidates[*v as usize].push((*u, *count, jaccard));
    }

    for cands in &mut per_file_candidates {
        cands.sort_by(|a, b| {
            b.2.total_cmp(&a.2)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| b.0.cmp(&a.0))
        });
        cands.truncate(config.max_edges_per_file);
    }

    // Emit symmetric edges once per undirected pair.
    let mut emitted_undirected: FxHashMap<(u32, u32), (u32, f32)> = FxHashMap::default();
    for (u, cands) in per_file_candidates.iter().enumerate() {
        let u = u as u32;
        for &(v, count, jaccard) in cands {
            let key = if u < v { (u, v) } else { (v, u) };
            emitted_undirected.entry(key).or_insert((count, jaccard));
        }
    }

    let mut edges = Vec::with_capacity(emitted_undirected.len() * 2);
    for ((u, v), (count, jaccard)) in &emitted_undirected {
        let from = dense_to_uuid[*u as usize];
        let to = dense_to_uuid[*v as usize];
        let count_s = count.to_string();
        let jaccard_s = format!("{jaccard:.4}");
        edges.push(
            Edge::new(from, to, EdgeType::CoChangesWith)
                .with_property("cochange_count".into(), count_s.clone())
                .with_property("jaccard".into(), jaccard_s.clone()),
        );
        edges.push(
            Edge::new(to, from, EdgeType::CoChangesWith)
                .with_property("cochange_count".into(), count_s)
                .with_property("jaccard".into(), jaccard_s),
        );
    }
    let edges_emitted = edges.len();
    if !edges.is_empty() {
        backend.insert_edges_batch(edges)?;
    }

    // Stamp File + Function/Method properties.
    let mut prop_updates: std::collections::HashMap<
        Uuid,
        std::collections::HashMap<String, String>,
    > = std::collections::HashMap::new();
    let mut report_files = Vec::new();

    for u in 0..n {
        if commits_count[u] == 0 {
            continue;
        }
        let max_cyclo = dense_max_cyclomatic[u];
        let hotspot = recency_churn[u] * (1.0 + f64::from(max_cyclo)).ln();
        let churn_lines = lines_added[u].saturating_add(lines_deleted[u]);
        let file_id = dense_to_uuid[u];

        let mut props = std::collections::HashMap::new();
        props.insert("git_commits".into(), commits_count[u].to_string());
        props.insert("git_churn_lines".into(), churn_lines.to_string());
        props.insert("git_churn_score".into(), format!("{:.4}", recency_churn[u]));
        props.insert("git_hotspot_score".into(), format!("{:.4}", hotspot));
        prop_updates.insert(file_id, props);

        for &(fn_id, cyclo) in &functions_by_file[u] {
            let fn_hotspot = recency_churn[u] * (1.0 + f64::from(cyclo)).ln();
            let mut fp = std::collections::HashMap::new();
            fp.insert("git_hotspot_score".into(), format!("{:.4}", fn_hotspot));
            prop_updates.insert(fn_id, fp);
        }

        report_files.push(FileChurnMetrics {
            file_path: dense_paths[u].clone(),
            file_node_id: file_id,
            commit_count: commits_count[u],
            lines_added: lines_added[u],
            lines_deleted: lines_deleted[u],
            last_modified_ts: last_ts[u],
            recency_churn_score: recency_churn[u],
            max_cyclomatic: max_cyclo,
            hotspot_score: hotspot,
        });
    }

    report_files.sort_by(|a, b| b.hotspot_score.total_cmp(&a.hotspot_score));
    if !prop_updates.is_empty() {
        backend.batch_update_node_properties(prop_updates)?;
    }

    Ok(GitChurnReport {
        files: report_files,
        edges_emitted,
        commits_scanned,
        commits_used,
        files_indexed,
    })
}

struct ParsedCommit {
    ts: i64,
    files: Vec<(u32, u64, u64)>,
}

fn stream_git_commits(
    repo_root: &Path,
    config: &GitChurnConfig,
    file_path_to_dense: &FxHashMap<String, u32>,
) -> Result<Vec<ParsedCommit>> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args([
            "log",
            "--no-merges",
            "--numstat",
            "--format=commit:%H%x09%ct",
        ])
        .arg(format!("-n{}", config.max_commits))
        .arg(format!("--since={}", config.since))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Other(format!("failed to spawn git: {e}")))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Other("git stdout missing".into()))?;
    let reader = BufReader::new(stdout);

    let mut commits: Vec<ParsedCommit> = Vec::new();
    let mut cur_ts: Option<i64> = None;
    let mut cur_files: Vec<(u32, u64, u64)> = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| Error::Other(format!("git log read: {e}")))?;
        if let Some(rest) = line.strip_prefix("commit:") {
            if let Some(ts) = cur_ts.take() {
                commits.push(ParsedCommit {
                    ts,
                    files: std::mem::take(&mut cur_files),
                });
            }
            // format: <sha>\t<unix_ts>
            let ts = rest
                .rsplit('\t')
                .next()
                .and_then(|s| s.trim().parse::<i64>().ok())
                .unwrap_or(0);
            cur_ts = Some(ts);
            continue;
        }
        if cur_ts.is_none() {
            continue;
        }
        // numstat: <added>\t<deleted>\t<path>
        let mut parts = line.splitn(3, '\t');
        let Some(added_s) = parts.next() else {
            continue;
        };
        let Some(deleted_s) = parts.next() else {
            continue;
        };
        let Some(path_s) = parts.next() else {
            continue;
        };
        if path_s.is_empty() {
            continue;
        }
        let added = parse_numstat_count(added_s);
        let deleted = parse_numstat_count(deleted_s);
        let path = normalize_git_numstat_path(path_s);
        if let Some(&dense) = file_path_to_dense.get(&path) {
            cur_files.push((dense, added, deleted));
        }
    }

    if let Some(ts) = cur_ts.take() {
        commits.push(ParsedCommit {
            ts,
            files: cur_files,
        });
    }

    let status = child
        .wait()
        .map_err(|e| Error::Other(format!("git wait: {e}")))?;
    if !status.success() {
        return Err(Error::Other(format!("git log exited with status {status}")));
    }

    Ok(commits)
}

fn parse_numstat_count(s: &str) -> u64 {
    if s == "-" { 0 } else { s.parse().unwrap_or(0) }
}

fn parse_u32_prop(v: Option<&str>) -> Option<u32> {
    v.and_then(|s| s.parse().ok())
}

fn normalize_node_repo_path(repo_root: &Path, canon_root: Option<&Path>, raw: &str) -> String {
    let p = Path::new(raw);
    if let Ok(rel) = p.strip_prefix(repo_root) {
        return normalize_repo_path(&rel.to_string_lossy());
    }
    if let Some(canon) = canon_root {
        if let Ok(rel) = p.strip_prefix(canon) {
            return normalize_repo_path(&rel.to_string_lossy());
        }
        if let Ok(canon_p) = p.canonicalize() {
            if let Ok(rel) = canon_p.strip_prefix(canon) {
                return normalize_repo_path(&rel.to_string_lossy());
            }
        }
    }
    normalize_repo_path(raw)
}

fn normalize_repo_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

/// Normalize a git `--numstat` path, resolving rename syntax to the post-rename path.
fn normalize_git_numstat_path(raw: &str) -> String {
    let s = raw.trim();
    // `old => new` or `prefix/{old => new}/suffix`
    let resolved = if let Some(brace_start) = s.find('{') {
        if let Some(brace_end) = s[brace_start..].find('}') {
            let brace_end = brace_start + brace_end;
            let inner = &s[brace_start + 1..brace_end];
            if let Some((_, new)) = inner.split_once(" => ") {
                let mut out = String::with_capacity(s.len());
                out.push_str(&s[..brace_start]);
                out.push_str(new.trim());
                out.push_str(&s[brace_end + 1..]);
                out
            } else {
                s.to_string()
            }
        } else {
            s.to_string()
        }
    } else if let Some((_, new)) = s.split_once(" => ") {
        new.trim().to_string()
    } else {
        s.to_string()
    };
    normalize_repo_path(&resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgctl_graph::backend::GraphBackend;
    use rgctl_graph::schema::Node;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    fn file_node(path: &str) -> Node {
        Node::new(NodeType::File, path.to_string()).with_file_path(path.to_string())
    }

    fn fn_node(path: &str, name: &str, cyclomatic: u32) -> Node {
        Node::new(NodeType::Function, name.to_string())
            .with_qualified_name(format!("{path}::{name}"))
            .with_file_path(path.to_string())
            .with_property("cyclomatic".into(), cyclomatic.to_string())
    }

    fn git(repo: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {:?} failed: {status}", args);
    }

    fn git_commit(repo: &Path, msg: &str, author_date: &str) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args([
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                msg,
                "--allow-empty-message",
            ])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_AUTHOR_DATE", author_date)
            .env("GIT_COMMITTER_DATE", author_date)
            .status()
            .expect("spawn git commit");
        assert!(status.success(), "git commit failed: {status}");
    }

    fn write_and_add(repo: &Path, rel: &str, body: &str) {
        let path = repo.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, body).unwrap();
        git(repo, &["add", rel]);
    }

    #[test]
    fn cochange_jaccard_and_mega_commit_guard() {
        let dir = tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init"]);
        git(repo, &["config", "user.email", "test@example.com"]);
        git(repo, &["config", "user.name", "Test"]);
        git(repo, &["config", "commit.gpgsign", "false"]);
        let _ = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["checkout", "-b", "main"])
            .status();

        // Seed indexed paths (seed commit itself is a mega with 7 files → skipped).
        write_and_add(repo, "a.rs", "// a0\n");
        write_and_add(repo, "b.rs", "// b0\n");
        write_and_add(repo, "c.rs", "// c0\n");
        write_and_add(repo, "mega0.rs", "// m0\n");
        write_and_add(repo, "mega1.rs", "// m1\n");
        write_and_add(repo, "mega2.rs", "// m2\n");
        write_and_add(repo, "mega3.rs", "// m3\n");
        git_commit(repo, "seed", "2026-01-01T12:00:00+00:00");

        // a+b together 4 times → count=4, J(a,b)=1.0.
        for i in 1..=4 {
            write_and_add(repo, "a.rs", &format!("// a{i}\n"));
            write_and_add(repo, "b.rs", &format!("// b{i}\n"));
            git_commit(
                repo,
                &format!("ab{i}"),
                &format!("2026-01-0{}T12:00:00+00:00", i + 1),
            );
        }

        // Non-mega 2-file commit (c.rs + mega1.rs once → support 1 < min_cochange_count = 3):
        // verifies min_cochange_count rejects low-support pairs on normal commits while
        // keeping J(a, b) = 4 / (4 + 4 - 4) = 1.0000.
        write_and_add(repo, "c.rs", "// c-with-m1\n");
        write_and_add(repo, "mega1.rs", "// m1-with-c\n");
        git_commit(repo, "cm1-once", "2026-01-10T12:00:00+00:00");

        // Explicit mega-commit: 4 files > max_files_per_commit=3 (skipped by mega-commit guard).
        write_and_add(repo, "a.rs", "// mega-a\n");
        write_and_add(repo, "b.rs", "// mega-b\n");
        write_and_add(repo, "c.rs", "// mega-c\n");
        write_and_add(repo, "mega0.rs", "// mega-m0\n");
        git_commit(repo, "mega", "2026-01-11T12:00:00+00:00");

        let mut backend = MemoryBackend::new();
        for p in [
            "a.rs", "b.rs", "c.rs", "mega0.rs", "mega1.rs", "mega2.rs", "mega3.rs",
        ] {
            backend.insert_node(file_node(p)).unwrap();
        }
        backend.insert_node(fn_node("a.rs", "fa", 3)).unwrap();

        let config = GitChurnConfig {
            max_commits: 50,
            since: "10.years".into(),
            min_cochange_count: 3,
            min_jaccard: 0.25,
            max_files_per_commit: 3,
            max_edges_per_file: 15,
        };

        let report = compute_and_apply_git_churn(repo, &mut backend, &config).unwrap();
        assert!(report.commits_scanned > 0);
        assert!(report.edges_emitted >= 2);

        let edges = backend.find_edges_by_type(EdgeType::CoChangesWith).unwrap();
        assert!(!EdgeType::CoChangesWith.is_call_traversal());

        let files = backend.find_nodes_by_type(NodeType::File).unwrap();
        let file_id = |name: &str| {
            files
                .iter()
                .find(|n| n.name.as_str() == name || n.file_path.as_deref() == Some(name))
                .map(|n| n.id)
                .unwrap_or_else(|| panic!("missing file node {name}"))
        };
        let a_id = file_id("a.rs");
        let b_id = file_id("b.rs");
        let c_id = file_id("c.rs");

        let ab: Vec<_> = edges
            .iter()
            .filter(|e| (e.from == a_id && e.to == b_id) || (e.from == b_id && e.to == a_id))
            .collect();
        assert_eq!(
            ab.len(),
            2,
            "expected symmetric a↔b CoChangesWith, got {edges:?}"
        );
        for e in &ab {
            assert_eq!(e.get_property("cochange_count"), Some("4"));
            assert_eq!(e.get_property("jaccard"), Some("1.0000"));
        }

        assert!(
            !edges
                .iter()
                .any(|e| (e.from == a_id && e.to == c_id) || (e.from == c_id && e.to == a_id)),
            "no a↔c CoChangesWith expected"
        );

        let a_node = backend.get_node(a_id).unwrap().unwrap();
        assert_eq!(a_node.get_property("git_commits"), Some("4"));
        assert!(a_node.get_property("git_churn_lines").is_some());
        assert!(a_node.get_property("git_churn_score").is_some());
        assert!(a_node.get_property("git_hotspot_score").is_some());

        let b_node = backend.get_node(b_id).unwrap().unwrap();
        assert_eq!(b_node.get_property("git_commits"), Some("4"));
        assert!(b_node.get_property("git_hotspot_score").is_some());

        // c.rs was touched in the non-mega 2-file commit (count=1 < min_cochange_count=3),
        // so it has git_commits == "1" but zero CoChangesWith edges.
        let c_node = backend.get_node(c_id).unwrap().unwrap();
        assert_eq!(c_node.get_property("git_commits"), Some("1"));
        assert!(
            !edges.iter().any(|e| e.from == c_id || e.to == c_id),
            "c.rs (count=1 < 3) must not have CoChangesWith edges"
        );

        // Verify CoChangesWith edge properties survive columnar snapshot round-trip.
        let snap_path = repo.join("roundtrip.snapshot.bin");
        rgctl_graph::write_columnar_from_backend(&backend, &snap_path).unwrap();
        let hydrated = rgctl_graph::snapshot::MmappedGraphSnapshot::open(&snap_path)
            .unwrap()
            .hydrate_backend()
            .unwrap();
        let rt_edges = hydrated
            .find_edges_by_type(EdgeType::CoChangesWith)
            .unwrap();
        assert_eq!(rt_edges.len(), 2);
        for e in &rt_edges {
            assert_eq!(e.get_property("cochange_count"), Some("4"));
            assert_eq!(e.get_property("jaccard"), Some("1.0000"));
        }
    }

    #[test]
    fn normalize_rename_paths() {
        assert_eq!(
            normalize_git_numstat_path("src/{old.rs => new.rs}"),
            "src/new.rs"
        );
        assert_eq!(normalize_git_numstat_path("old.rs => new.rs"), "new.rs");
        assert_eq!(normalize_git_numstat_path("./foo/bar.rs"), "foo/bar.rs");
    }
}
