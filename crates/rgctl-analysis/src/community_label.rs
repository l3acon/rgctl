//! Heuristic community naming (Graphify-style sidebar labels).
//!
//! Labels are derived metadata stored beside topology — never written into the
//! code-graph snapshot.

use crate::results::AnalysisResults;
use rgctl_error::Result;
use rgctl_graph::schema::Node;
use std::collections::HashMap;
use uuid::Uuid;

/// Inputs for naming a single community.
#[derive(Debug, Clone)]
pub struct CommunityLabelHints<'a> {
    /// Community id from Fast-Leiden.
    pub id: usize,
    /// Member symbol names.
    pub names: &'a [String],
    /// Member file paths.
    pub file_paths: &'a [String],
    /// Package / module labels (majority vote preferred).
    pub package_labels: &'a [String],
    /// Highest-PageRank member name within the community, if known.
    pub top_pagerank_name: Option<&'a str>,
    /// True when this id is the infrastructure / hub cluster.
    pub is_infrastructure: bool,
}

/// Generic container / build-layout segments that must never become community labels.
const CONTAINER_SEGMENTS: &[&str] = &["root", "src", "main", "test", "java", "crates", "packages"];

/// Language modifiers and boilerplate tokens filtered from symbol-name inference.
const STOPWORDS: &[&str] = &[
    "mutable",
    "immutable",
    "get",
    "set",
    "is",
    "has",
    "new",
    "build",
    "builder",
    "create",
    "default",
    "impl",
    "test",
    "tests",
    "from",
    "into",
    "with",
    "value",
    "values",
    "data",
    "item",
    "items",
    "node",
    "helper",
    "util",
    "utils",
    "common",
    "base",
    "abstract",
    "internal",
    "public",
    "private",
    "static",
    "async",
    "main",
    "init",
    "run",
    "call",
    "execute",
    "handle",
    "process",
];

/// Infer a human-readable community label.
pub fn infer_community_label(hints: &CommunityLabelHints<'_>) -> String {
    if hints.is_infrastructure {
        return "Infrastructure / Common Library".to_string();
    }

    if let Some(pkg) = majority_token(hints.package_labels, 2) {
        if !pkg.is_empty() && !is_container_segment(&pkg) {
            let base = shorten_package_label(&pkg);
            if let Some(name) = hints.top_pagerank_name {
                if let Some(short) = meaningful_symbol_token(name) {
                    return format!("{base}::{short}");
                }
            }
            return base;
        }
    }

    if let Some(common) = find_common_package_prefix(hints.package_labels) {
        if !common.is_empty() {
            return shorten_package_label(&common);
        }
    }

    if let Some(name) = hints.top_pagerank_name {
        if let Some(short) = meaningful_symbol_token(name) {
            return short;
        }
    }

    infer_label_from_names(hints.names, hints.id)
}

/// Ensure colliding labels become unique (`auth`, `auth (2)`, …).
pub fn dedupe_community_labels(labels: &mut [(usize, String)]) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (_, label) in labels.iter_mut() {
        let key = label.to_ascii_lowercase();
        let count = seen.entry(key).or_insert(0);
        *count += 1;
        if *count > 1 {
            *label = format!("{label} ({count})");
        }
    }
}

/// Fill [`AnalysisResults::community`] labels from member metadata.
///
/// `lookup` returns `(name, file_path)` for each assigned node UUID.
pub fn fill_community_labels<F>(
    analysis: &mut AnalysisResults,
    infrastructure_id: Option<usize>,
    mut lookup: F,
) -> Result<()>
where
    F: FnMut(Uuid) -> Option<(String, Option<String>)>,
{
    let Some(table) = analysis.community.as_ref() else {
        return Ok(());
    };

    let max_cid = table.assignments.iter().copied().max().unwrap_or(0);
    let mut counts = vec![0u32; max_cid + 1];
    for &cid in &table.assignments {
        counts[cid] += 1;
    }
    let multi_count = counts
        .iter()
        .enumerate()
        .filter(|&(cid, &c)| c > 1 || infrastructure_id == Some(cid))
        .count();

    let mut members: HashMap<usize, Vec<(Uuid, u32)>> = HashMap::with_capacity(multi_count);
    for (compact, &cid) in table.assignments.iter().enumerate() {
        // Skip singleton isolate communities before allocating Vecs — on linux-scale
        // corpora (>1.5M isolates), inserting singletons into `members` allocates
        // millions of 1-element heap Vecs that are immediately discarded.
        if counts[cid] <= 1 && infrastructure_id != Some(cid) {
            continue;
        }
        let compact = compact as u32;
        if let Some(uuid) = analysis.get_uuid(compact) {
            members
                .entry(cid)
                .or_insert_with(|| Vec::with_capacity(counts[cid] as usize))
                .push((uuid, compact));
        }
    }
    let pagerank = analysis.centrality.as_ref().map(|c| c.pagerank.clone());

    let mut labeled: Vec<(usize, String)> = Vec::with_capacity(members.len());
    for (cid, entries) in &members {
        let mut names = Vec::new();
        let mut paths = Vec::new();
        let mut packages = Vec::new();
        let mut top_pr_name: Option<String> = None;
        let mut top_pr = f32::MIN;

        for (uuid, compact) in entries {
            let Some((name, path)) = lookup(*uuid) else {
                continue;
            };
            if let Some(pr_table) = &pagerank {
                let score = pr_table.get(*compact as usize).copied().unwrap_or(0.0);
                if score > top_pr {
                    top_pr = score;
                    top_pr_name = Some(name.clone());
                }
            }
            if let Some(p) = &path {
                packages.push(package_label_from_path(p));
                paths.push(p.clone());
            }
            names.push(name);
        }

        let hints = CommunityLabelHints {
            id: *cid,
            names: &names,
            file_paths: &paths,
            package_labels: &packages,
            top_pagerank_name: top_pr_name.as_deref(),
            is_infrastructure: infrastructure_id == Some(*cid),
        };
        labeled.push((*cid, infer_community_label(&hints)));
    }

    labeled.sort_by_key(|(id, _)| *id);
    dedupe_community_labels(&mut labeled);

    if let Some(table) = analysis.community.as_mut() {
        table.infrastructure_community_id = infrastructure_id;
        table.labels = labeled.into_iter().collect();
    }
    Ok(())
}

/// Fill labels using full [`Node`] lookups (dashboard / MemoryBackend path).
pub fn fill_community_labels_from_nodes<F>(
    analysis: &mut AnalysisResults,
    infrastructure_id: Option<usize>,
    mut get_node: F,
) -> Result<()>
where
    F: FnMut(Uuid) -> Option<Node>,
{
    fill_community_labels(analysis, infrastructure_id, |uuid| {
        get_node(uuid).map(|n| {
            (
                n.name.to_string(),
                n.file_path.as_ref().map(|s| s.to_string()),
            )
        })
    })
}

fn majority_token(items: &[String], min_len: usize) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for item in items {
        if item.len() >= min_len {
            *counts.entry(item.as_str()).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .filter(|(_, c)| *c * 3 >= items.len())
        .map(|(s, _)| s.to_string())
}

fn shorten_package_label(pkg: &str) -> String {
    let parts: Vec<&str> = pkg.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 2 {
        return pkg.to_string();
    }
    parts[parts.len().saturating_sub(2)..].join(".")
}

fn package_label_from_path(file_path: &str) -> String {
    crate::migration::package_label(file_path)
}

fn is_container_segment(seg: &str) -> bool {
    CONTAINER_SEGMENTS
        .iter()
        .any(|c| seg.eq_ignore_ascii_case(c))
}

fn is_stopword(token: &str) -> bool {
    STOPWORDS.iter().any(|s| token.eq_ignore_ascii_case(s))
}

/// Longest common dot-segment prefix across package labels, skipping container segments.
fn find_common_package_prefix(packages: &[String]) -> Option<String> {
    if packages.is_empty() {
        return None;
    }
    let split: Vec<Vec<&str>> = packages
        .iter()
        .map(|p| {
            p.split('.')
                .filter(|s| !s.is_empty() && !is_container_segment(s))
                .collect()
        })
        .filter(|segs: &Vec<&str>| !segs.is_empty())
        .collect();
    if split.is_empty() {
        return None;
    }
    let first = &split[0];
    let mut common_len = first.len();
    for segs in &split[1..] {
        let shared = first
            .iter()
            .zip(segs.iter())
            .take(common_len)
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .count();
        common_len = shared;
        if common_len == 0 {
            return None;
        }
    }
    if common_len == 0 {
        return None;
    }
    Some(first[..common_len].join("."))
}

/// Split on `_` / `-` / `.` / `:` and camelCase / PascalCase boundaries.
fn split_identifier_tokens(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in name.split(['_', '-', '.', '/', ':']) {
        if part.is_empty() {
            continue;
        }
        let mut current = String::new();
        let chars: Vec<char> = part.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            let boundary = if ch.is_uppercase() && !current.is_empty() {
                let prev_lower = chars
                    .get(i.wrapping_sub(1))
                    .is_some_and(|c| c.is_lowercase());
                let next_lower = chars.get(i + 1).is_some_and(|c| c.is_lowercase());
                prev_lower || next_lower
            } else {
                false
            };
            if boundary {
                out.push(std::mem::take(&mut current));
            }
            current.push(ch);
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    out
}

/// Last path segment of a symbol name that survives stopword filtering.
fn meaningful_symbol_token(name: &str) -> Option<String> {
    let short = name.split(['/', '.', ':']).next_back().unwrap_or(name);
    let tokens = split_identifier_tokens(short);
    let meaningful: Vec<&str> = tokens
        .iter()
        .map(|t| t.as_str())
        .filter(|t| t.len() > 2 && !is_stopword(t))
        .collect();
    if meaningful.is_empty() {
        return None;
    }
    // Prefer the last meaningful token (e.g. MutableOrderLine → OrderLine's "Line"
    // is weak; join trailing meaningful run when short).
    if meaningful.len() == 1 {
        return Some(meaningful[0].to_string());
    }
    Some(meaningful.join(""))
}

fn infer_label_from_names(names: &[String], idx: usize) -> String {
    if names.is_empty() {
        return format!("Community {idx}");
    }

    let mut owned_tokens: Vec<String> = Vec::new();
    for name in names {
        for token in split_identifier_tokens(name) {
            if token.len() > 2 && !is_stopword(&token) {
                owned_tokens.push(token.to_ascii_lowercase());
            }
        }
    }

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for token in &owned_tokens {
        *counts.entry(token.as_str()).or_insert(0) += 1;
    }

    if let Some((token, _)) = counts.iter().max_by_key(|(_, count)| *count) {
        if counts[token] * 3 >= names.len() {
            // Preserve a readable casing from the first original match.
            for name in names {
                for orig in split_identifier_tokens(name) {
                    if orig.eq_ignore_ascii_case(token) {
                        return orig;
                    }
                }
            }
            return (*token).to_string();
        }
    }

    format!("Community {idx}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_majority_wins() {
        let packages = vec![
            "com.example.auth".into(),
            "com.example.auth".into(),
            "com.example.other".into(),
        ];
        let names = vec!["a".into(), "b".into(), "c".into()];
        let label = infer_community_label(&CommunityLabelHints {
            id: 1,
            names: &names,
            file_paths: &[],
            package_labels: &packages,
            top_pagerank_name: None,
            is_infrastructure: false,
        });
        assert_eq!(label, "example.auth");
    }

    #[test]
    fn infrastructure_label() {
        let label = infer_community_label(&CommunityLabelHints {
            id: 0,
            names: &["util".into()],
            file_paths: &[],
            package_labels: &[],
            top_pagerank_name: None,
            is_infrastructure: true,
        });
        assert_eq!(label, "Infrastructure / Common Library");
    }

    #[test]
    fn dedupe_suffixes() {
        let mut labels = vec![(1, "auth".into()), (2, "auth".into()), (3, "cart".into())];
        dedupe_community_labels(&mut labels);
        assert_eq!(labels[0].1, "auth");
        assert_eq!(labels[1].1, "auth (2)");
        assert_eq!(labels[2].1, "cart");
    }

    #[test]
    fn token_fallback() {
        let names = vec!["do_login".into(), "try_login".into(), "login_admin".into()];
        let label = infer_community_label(&CommunityLabelHints {
            id: 9,
            names: &names,
            file_paths: &[],
            package_labels: &[],
            top_pagerank_name: None,
            is_infrastructure: false,
        });
        assert_eq!(label, "login");
    }

    #[test]
    fn modifier_tokens_do_not_become_labels() {
        let names = vec![
            "MutableOrder".into(),
            "MutableLine".into(),
            "MutableAccount".into(),
        ];
        let label = infer_community_label(&CommunityLabelHints {
            id: 4,
            names: &names,
            file_paths: &[],
            package_labels: &[],
            top_pagerank_name: Some("MutableOrder"),
            is_infrastructure: false,
        });
        assert_ne!(label.to_ascii_lowercase(), "mutable");
        assert!(!label.eq_ignore_ascii_case("Mutable"));
    }

    #[test]
    fn checkout_crates_prefix_is_not_a_label() {
        // Absolute checkout paths used to yield "crates" via filesystem common-prefix.
        let packages = vec![
            "rgctl-analysis".into(),
            "rgctl-dashboard".into(),
            "rgctl-graph".into(),
        ];
        let paths = vec![
            "/Users/sami/git/rust/rbuilder/crates/rgctl-analysis/src/lib.rs".into(),
            "/Users/sami/git/rust/rbuilder/crates/rgctl-dashboard/src/lib.rs".into(),
            "/Users/sami/git/rust/rbuilder/crates/rgctl-graph/src/lib.rs".into(),
        ];
        let label = infer_community_label(&CommunityLabelHints {
            id: 2,
            names: &["detect".into(), "export".into(), "node".into()],
            file_paths: &paths,
            package_labels: &packages,
            top_pagerank_name: None,
            is_infrastructure: false,
        });
        assert!(!label.eq_ignore_ascii_case("crates"));
        assert!(!label.to_ascii_lowercase().starts_with("users"));
        assert!(!label.starts_with('.'));
    }

    #[test]
    fn camel_case_split_filters_get_set_boilerplate() {
        let names = vec![
            "getOrderTotal".into(),
            "setOrderTotal".into(),
            "buildOrderTotal".into(),
        ];
        let label = infer_community_label(&CommunityLabelHints {
            id: 5,
            names: &names,
            file_paths: &[],
            package_labels: &[],
            top_pagerank_name: None,
            is_infrastructure: false,
        });
        let lower = label.to_ascii_lowercase();
        assert_ne!(lower, "get");
        assert_ne!(lower, "set");
        assert_ne!(lower, "build");
        assert!(
            lower.contains("order") || lower.contains("total") || lower.starts_with("community"),
            "expected order/total signal or Community fallback, got {label}"
        );
    }
}
