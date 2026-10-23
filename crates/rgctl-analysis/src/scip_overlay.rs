//! Optional SCIP index overlay for compiler-exact call resolution.
//!
//! When `rgctl discover --scip <index.scip>` is set, definition/reference
//! occurrences from a standard SCIP protobuf index are merged onto the graph:
//! ambiguous or missing `Calls` edges are upgraded with `resolved_via=scip`.

use protobuf::Message;
use rgctl_error::{Error, Result};
use rgctl_graph::backend::MemoryBackend;
use rgctl_graph::schema::{Edge, EdgeType, Node, NodeType};
use rustc_hash::{FxHashMap, FxHashSet};
use scip::types::{Document, Index, Occurrence, SymbolRole};
use std::path::Path;
use uuid::Uuid;

/// Bit mask for [`SymbolRole::Definition`].
const ROLE_DEFINITION: i32 = SymbolRole::Definition as i32;

/// Stats from [`apply_scip_overlay`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScipOverlayStats {
    /// SCIP documents processed.
    pub documents: usize,
    /// Definition occurrences mapped onto graph nodes.
    pub definitions_mapped: usize,
    /// New `Calls` edges inserted from reference occurrences.
    pub calls_added: usize,
}

/// Load a SCIP protobuf index from `path` and merge compiler-exact Calls onto `backend`.
pub fn apply_scip_overlay(backend: &mut MemoryBackend, scip_path: &Path) -> Result<ScipOverlayStats> {
    let index = load_scip_index(scip_path)?;
    apply_scip_index(backend, &index)
}

/// Parse a `.scip` protobuf payload.
pub fn load_scip_index(path: &Path) -> Result<Index> {
    let bytes = std::fs::read(path).map_err(|e| {
        Error::Other(format!("failed to read SCIP index {}: {e}", path.display()))
    })?;
    Index::parse_from_bytes(&bytes).map_err(|e| {
        Error::SerdeError(format!(
            "failed to parse SCIP index {}: {e}",
            path.display()
        ))
    })
}

/// Apply an already-parsed SCIP [`Index`] onto `backend`.
pub fn apply_scip_index(backend: &mut MemoryBackend, index: &Index) -> Result<ScipOverlayStats> {
    let nodes = backend.all_nodes()?;
    let project_root = index
        .metadata
        .as_ref()
        .map(|m| m.project_root.as_str())
        .filter(|s| !s.is_empty());
    let locator = GraphLocator::build(&nodes, project_root);

    // SCIP symbol string → graph node UUID (first confident definition mapping).
    let mut symbol_to_node: FxHashMap<String, Uuid> = FxHashMap::default();
    let mut definitions_mapped = 0usize;

    for doc in &index.documents {
        let path = normalize_path(&doc.relative_path);
        for occ in &doc.occurrences {
            if !is_definition(occ.symbol_roles) {
                continue;
            }
            let Some((start, end)) = occ_line_span(occ) else {
                continue;
            };
            let Some(node_id) = locator.resolve_definition(&path, &occ.symbol, start, end) else {
                continue;
            };
            if symbol_to_node.insert(occ.symbol.clone(), node_id).is_none() {
                definitions_mapped += 1;
            }
        }
    }

    // Also map SymbolInformation entries that name symbols defined in-document.
    for doc in &index.documents {
        let path = normalize_path(&doc.relative_path);
        for info in &doc.symbols {
            if symbol_to_node.contains_key(&info.symbol) {
                continue;
            }
            let Some(name) = scip_display_name(&info.symbol) else {
                continue;
            };
            if let Some(id) = locator.resolve_by_path_name(&path, name) {
                symbol_to_node.insert(info.symbol.clone(), id);
                definitions_mapped += 1;
            }
        }
    }

    let mut extras: Vec<Edge> = Vec::new();
    let mut seen: FxHashSet<(Uuid, Uuid)> = FxHashSet::default();

    for doc in &index.documents {
        let path = normalize_path(&doc.relative_path);
        let def_spans = definition_spans(doc);

        for occ in &doc.occurrences {
            if is_definition(occ.symbol_roles) {
                continue;
            }
            let Some(&to) = symbol_to_node.get(&occ.symbol) else {
                continue;
            };
            let Some((start, _)) = occ_line_span(occ) else {
                continue;
            };
            let Some(from) = enclosing_callable(&locator, &path, start, &def_spans, &symbol_to_node)
            else {
                continue;
            };
            if from == to || !seen.insert((from, to)) {
                continue;
            }
            if backend.has_edge(from, to, EdgeType::Calls) {
                continue;
            }
            let callee = scip_display_name(&occ.symbol).unwrap_or("").to_string();
            let mut edge = Edge::new(from, to, EdgeType::Calls)
                .with_property("resolved_via".into(), "scip".into());
            if !callee.is_empty() {
                edge = edge.with_property("callee".into(), callee);
            }
            edge = edge.with_property("call_site_line".into(), start.to_string());
            extras.push(edge);
        }
    }

    let calls_added = extras.len();
    if !extras.is_empty() {
        backend.insert_edges_batch(extras)?;
    }

    Ok(ScipOverlayStats {
        documents: index.documents.len(),
        definitions_mapped,
        calls_added,
    })
}

fn is_definition(roles: i32) -> bool {
    roles & ROLE_DEFINITION != 0
}

/// Normalize repo-relative paths for matching.
pub fn normalize_path(path: &str) -> String {
    let p = path
        .trim()
        .strip_prefix("file://")
        .unwrap_or(path.trim())
        .replace('\\', "/");
    p.trim_start_matches("./").trim_end_matches('/').to_string()
}

/// Check whether a graph node path (which may be absolute `/abs/repo/src/a.rs` or relative `src/a.rs`)
/// refers to the same file as a SCIP document `relative_path` (`src/a.rs`).
fn paths_match(node_path: &str, doc_path: &str) -> bool {
    if node_path == doc_path {
        return true;
    }
    if let Some(prefix) = node_path.strip_suffix(doc_path) {
        return prefix.ends_with('/');
    }
    if let Some(prefix) = doc_path.strip_suffix(node_path) {
        return prefix.ends_with('/');
    }
    false
}

/// Extract the file basename (`"a.rs"` from `"src/a.rs"`) for fast bucket indexing.
fn file_basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Extract a human-readable leaf name from a SCIP symbol string.
///
/// Handles common descriptor suffixes (`name().`, `name(+1).`, `name.`, `Type#method().`, `` `escaped`(). ``).
pub fn scip_display_name(symbol: &str) -> Option<&str> {
    let s = symbol.trim().trim_end_matches('.');
    if s.is_empty() {
        return None;
    }
    let after_space = s.rsplit(' ').next().unwrap_or(s);
    let after_slash = after_space.rsplit('/').next().unwrap_or(after_space);
    let without_parens = match after_slash.rfind('(') {
        Some(idx) if after_slash.ends_with(')') => &after_slash[..idx],
        _ => after_slash,
    };
    let name = if let Some(stripped) = without_parens.strip_suffix('`') {
        match stripped.rfind('`') {
            Some(open_idx) => &stripped[open_idx + 1..],
            None => stripped,
        }
    } else {
        without_parens.rsplit('#').next().unwrap_or(without_parens)
    };
    if name.is_empty() || name == "local" {
        None
    } else {
        Some(name)
    }
}

/// SCIP occurrence range → 1-based inclusive line span.
fn occ_line_span(occ: &Occurrence) -> Option<(usize, usize)> {
    range_to_lines(&occ.range)
}

fn range_to_lines(range: &[i32]) -> Option<(usize, usize)> {
    match range {
        [start_line, _, end_line, _] if *start_line >= 0 && *end_line >= 0 => {
            Some((*start_line as usize + 1, *end_line as usize + 1))
        }
        [start_line, _, _] if *start_line >= 0 => {
            let line = *start_line as usize + 1;
            Some((line, line))
        }
        _ => None,
    }
}

struct DefSpan {
    symbol: String,
    start: usize,
    end: usize,
}

fn definition_spans(doc: &Document) -> Vec<DefSpan> {
    let mut out = Vec::new();
    for occ in &doc.occurrences {
        if !is_definition(occ.symbol_roles) {
            continue;
        }
        // Prefer full `enclosing_range` when the SCIP indexer provides it; fall back to `range`.
        let Some((start, end)) =
            range_to_lines(&occ.enclosing_range).or_else(|| occ_line_span(occ))
        else {
            continue;
        };
        out.push(DefSpan {
            symbol: occ.symbol.clone(),
            start,
            end,
        });
    }
    out
}

fn enclosing_callable(
    locator: &GraphLocator,
    path: &str,
    line: usize,
    def_spans: &[DefSpan],
    symbol_to_node: &FxHashMap<String, Uuid>,
) -> Option<Uuid> {
    // Prefer SCIP enclosing definition mapped to a callable node when its span covers `line`
    // (e.g. when `enclosing_range` or multi-line definition range is present).
    let mut best: Option<(usize, Uuid)> = None; // (span_len, id)
    for span in def_spans {
        if line < span.start || line > span.end {
            continue;
        }
        let Some(&id) = symbol_to_node.get(&span.symbol) else {
            continue;
        };
        if !locator.is_callable(id) {
            continue;
        }
        let len = span.end.saturating_sub(span.start);
        match best {
            Some((best_len, _)) if len >= best_len => {}
            _ => best = Some((len, id)),
        }
    }
    if let Some((_, id)) = best {
        return Some(id);
    }
    locator.innermost_callable_at(path, line)
}

struct GraphLocator {
    /// (file basename, simple name) → candidate `(normalized_node_path, id)` pairs.
    by_base_and_name: FxHashMap<(String, String), Vec<(String, Uuid)>>,
    /// File basename → callable spans in that file.
    callables_by_base: FxHashMap<String, Vec<CallableSpan>>,
    /// Node UUID → CallableSpan for fast span lookup.
    callable_by_id: FxHashMap<Uuid, CallableSpan>,
}

#[derive(Clone)]
struct CallableSpan {
    id: Uuid,
    path: String,
    start: usize,
    end: usize,
    name: String,
}

impl GraphLocator {
    fn build(nodes: &[Node], project_root: Option<&str>) -> Self {
        let norm_root = project_root.map(normalize_path).filter(|s| !s.is_empty());
        let mut by_base_and_name: FxHashMap<(String, String), Vec<(String, Uuid)>> =
            FxHashMap::default();
        let mut callables_by_base: FxHashMap<String, Vec<CallableSpan>> = FxHashMap::default();
        let mut callable_by_id: FxHashMap<Uuid, CallableSpan> = FxHashMap::default();

        for node in nodes {
            let is_callable = matches!(node.node_type, NodeType::Function | NodeType::Macro);
            let Some(path_raw) = node.file_path.as_ref() else {
                continue;
            };
            let mut path = normalize_path(path_raw.as_str());
            if let Some(ref root) = norm_root
                && let Some(stripped) = path.strip_prefix(root.as_str())
            {
                let rel = stripped.trim_start_matches('/');
                if !rel.is_empty() {
                    path = rel.to_string();
                }
            }
            let base = file_basename(&path).to_string();
            let name = node.name.as_str().to_string();
            if is_callable {
                let start = node.start_line.unwrap_or(1);
                let end = node.end_line.unwrap_or(start);
                let span = CallableSpan {
                    id: node.id,
                    path: path.clone(),
                    start,
                    end,
                    name: name.clone(),
                };
                callable_by_id.insert(node.id, span.clone());
                callables_by_base
                    .entry(base.clone())
                    .or_default()
                    .push(span);
            }
            // Index callables and type-ish owners for definition mapping.
            if is_callable
                || matches!(
                    node.node_type,
                    NodeType::Class
                        | NodeType::Struct
                        | NodeType::Enum
                        | NodeType::Interface
                        | NodeType::Module
                        | NodeType::TypeAlias
                )
            {
                by_base_and_name
                    .entry((base.clone(), name))
                    .or_default()
                    .push((path.clone(), node.id));
                if let Some(qn) = node.qualified_name.as_ref() {
                    let leaf = qn
                        .as_str()
                        .rsplit(['.', ':', '#'])
                        .next()
                        .unwrap_or(qn.as_str());
                    if leaf != node.name.as_str() {
                        by_base_and_name
                            .entry((base, leaf.to_string()))
                            .or_default()
                            .push((path, node.id));
                    }
                }
            }
        }

        Self {
            by_base_and_name,
            callables_by_base,
            callable_by_id,
        }
    }

    fn is_callable(&self, id: Uuid) -> bool {
        self.callable_by_id.contains_key(&id)
    }

    fn candidates_for(&self, path: &str, name: &str) -> Vec<Uuid> {
        let base = file_basename(path);
        let Some(entries) = self
            .by_base_and_name
            .get(&(base.to_string(), name.to_string()))
        else {
            return Vec::new();
        };
        entries
            .iter()
            .filter(|(node_path, _)| paths_match(node_path, path))
            .map(|(_, id)| *id)
            .collect()
    }

    fn resolve_by_path_name(&self, path: &str, name: &str) -> Option<Uuid> {
        let ids = self.candidates_for(path, name);
        unique_id(&ids)
    }

    fn resolve_definition(
        &self,
        path: &str,
        symbol: &str,
        start: usize,
        end: usize,
    ) -> Option<Uuid> {
        let name = scip_display_name(symbol)?;
        let ids = self.candidates_for(path, name);
        if ids.is_empty() {
            return None;
        }
        if let Some(id) = unique_id(&ids) {
            return Some(id);
        }
        // Disambiguate by overlapping line span among callables with that name.
        let mut best: Option<(usize, Uuid)> = None;
        for &id in &ids {
            let Some(span) = self.callable_by_id.get(&id) else {
                continue;
            };
            if !paths_match(&span.path, path) {
                continue;
            }
            let overlaps = (start >= span.start && start <= span.end)
                || (span.start >= start && span.start <= end)
                || (end >= span.start && end <= span.end);
            if !overlaps {
                continue;
            }
            let dist = span.start.abs_diff(start);
            match best {
                Some((best_dist, _)) if dist >= best_dist => {}
                _ => best = Some((dist, id)),
            }
        }
        best.map(|(_, id)| id).or_else(|| {
            let base = file_basename(path);
            let file_matches: Vec<Uuid> = self
                .callables_by_base
                .get(base)
                .into_iter()
                .flatten()
                .filter(|c| paths_match(&c.path, path) && c.name == name)
                .map(|c| c.id)
                .collect();
            unique_id(&file_matches)
        })
    }

    fn innermost_callable_at(&self, path: &str, line: usize) -> Option<Uuid> {
        let base = file_basename(path);
        let spans = self.callables_by_base.get(base)?;
        let mut best: Option<(usize, Uuid)> = None;
        for span in spans {
            if !paths_match(&span.path, path) || line < span.start || line > span.end {
                continue;
            }
            let len = span.end.saturating_sub(span.start);
            match best {
                Some((best_len, _)) if len >= best_len => {}
                _ => best = Some((len, span.id)),
            }
        }
        best.map(|(_, id)| id)
    }
}

fn unique_id(ids: &[Uuid]) -> Option<Uuid> {
    match ids {
        [id] => Some(*id),
        [first, rest @ ..] if rest.iter().all(|id| id == first) => Some(*first),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgctl_graph::backend::GraphBackend;
    use rgctl_graph::schema::Node;
    use scip::types::Document;
    use tempfile::tempdir;

    fn fn_node(file: &str, name: &str, start: usize, end: usize) -> Node {
        Node::new(NodeType::Function, name.to_string())
            .with_qualified_name(format!("{file}::{name}"))
            .with_file_path(file.to_string())
            .with_location(start, end)
    }

    fn write_index(path: &Path, index: &Index) {
        scip::write_message_to_file(path, index.clone()).expect("write scip");
    }

    #[test]
    fn display_name_from_scip_symbol() {
        assert_eq!(scip_display_name("local foo()."), Some("foo"));
        assert_eq!(
            scip_display_name("scip-rust cargo pkg 0.1.0 pkg/mod#bar()."),
            Some("bar")
        );
        assert_eq!(scip_display_name("local Type#method()."), Some("method"));
        assert_eq!(
            scip_display_name("scip-java maven pkg 1.0 pkg/Svc#process(+1)."),
            Some("process")
        );
        assert_eq!(
            scip_display_name("scip-rust cargo pkg 0.1.0 pkg/mod#`r#type`()."),
            Some("r#type")
        );
    }

    #[test]
    fn overlay_matches_absolute_node_paths_and_single_line_def_ranges() {
        let mut backend = MemoryBackend::new();
        let caller = fn_node("/Users/dev/repo/src/a.rs", "caller", 1, 20);
        let callee = fn_node("/Users/dev/repo/src/b.rs", "target", 1, 10);
        let caller_id = caller.id;
        let callee_id = callee.id;
        backend.insert_node(caller).unwrap();
        backend.insert_node(callee).unwrap();

        let mut index = Index::new();
        let mut doc_a = Document::new();
        doc_a.relative_path = "src/a.rs".into();
        // Real SCIP indexers emit a 3-element single-line identifier span `[line, start_col, end_col]`.
        let mut caller_def = Occurrence::new();
        caller_def.symbol = "scip-java maven pkg 1.0 pkg/A#caller(+1).".into();
        caller_def.symbol_roles = ROLE_DEFINITION;
        caller_def.range = vec![0, 3, 9];
        let mut call_ref = Occurrence::new();
        call_ref.symbol = "scip-java maven pkg 1.0 pkg/B#target(+1).".into();
        call_ref.symbol_roles = 0;
        call_ref.range = vec![5, 4, 10]; // line 6 inside caller (1..=20)
        doc_a.occurrences.push(caller_def);
        doc_a.occurrences.push(call_ref);

        let mut doc_b = Document::new();
        doc_b.relative_path = "src/b.rs".into();
        let mut target_def = Occurrence::new();
        target_def.symbol = "scip-java maven pkg 1.0 pkg/B#target(+1).".into();
        target_def.symbol_roles = ROLE_DEFINITION;
        target_def.range = vec![0, 3, 9];
        doc_b.occurrences.push(target_def);

        index.documents.push(doc_a);
        index.documents.push(doc_b);

        let stats = apply_scip_index(&mut backend, &index).unwrap();
        assert_eq!(stats.definitions_mapped, 2);
        assert_eq!(stats.calls_added, 1);

        let calls = backend.find_edges_by_type(EdgeType::Calls).unwrap();
        assert!(
            calls.iter().any(|e| {
                e.from == caller_id
                    && e.to == callee_id
                    && e.properties.get("resolved_via").map(|s| s.as_str()) == Some("scip")
                    && e.properties.get("callee").map(|s| s.as_str()) == Some("target")
            }),
            "expected SCIP Calls edge across absolute node paths, got {calls:?}"
        );
    }

    #[test]
    fn overlay_adds_compiler_exact_calls() {
        let mut backend = MemoryBackend::new();
        let caller = fn_node("src/a.rs", "caller", 1, 20);
        let callee = fn_node("src/b.rs", "target", 1, 10);
        let caller_id = caller.id;
        let callee_id = callee.id;
        backend.insert_node(caller).unwrap();
        backend.insert_node(callee).unwrap();

        let mut index = Index::new();
        let mut doc_a = Document::new();
        doc_a.relative_path = "src/a.rs".into();
        // Definition of caller covering the whole function.
        let mut caller_def = Occurrence::new();
        caller_def.symbol = "local caller().".into();
        caller_def.symbol_roles = ROLE_DEFINITION;
        caller_def.range = vec![0, 0, 19, 1]; // lines 0..=19 → 1-based 1..=20
        // Reference to target inside caller.
        let mut call_ref = Occurrence::new();
        call_ref.symbol = "local target().".into();
        call_ref.symbol_roles = 0;
        call_ref.range = vec![5, 4, 5, 10]; // line 6
        doc_a.occurrences.push(caller_def);
        doc_a.occurrences.push(call_ref);

        let mut doc_b = Document::new();
        doc_b.relative_path = "src/b.rs".into();
        let mut target_def = Occurrence::new();
        target_def.symbol = "local target().".into();
        target_def.symbol_roles = ROLE_DEFINITION;
        target_def.range = vec![0, 0, 9, 1];
        doc_b.occurrences.push(target_def);

        index.documents.push(doc_a);
        index.documents.push(doc_b);

        let dir = tempdir().unwrap();
        let scip_path = dir.path().join("index.scip");
        write_index(&scip_path, &index);

        let stats = apply_scip_overlay(&mut backend, &scip_path).unwrap();
        assert!(stats.definitions_mapped >= 2);
        assert_eq!(stats.calls_added, 1);

        let calls = backend.find_edges_by_type(EdgeType::Calls).unwrap();
        assert!(
            calls.iter().any(|e| {
                e.from == caller_id
                    && e.to == callee_id
                    && e.properties.get("resolved_via").map(|s| s.as_str()) == Some("scip")
            }),
            "expected SCIP Calls edge, got {calls:?}"
        );
    }

    #[test]
    fn overlay_skips_duplicate_existing_calls() {
        let mut backend = MemoryBackend::new();
        let caller = fn_node("src/a.rs", "caller", 1, 10);
        let callee = fn_node("src/a.rs", "target", 12, 20);
        let caller_id = caller.id;
        let callee_id = callee.id;
        backend.insert_node(caller).unwrap();
        backend.insert_node(callee).unwrap();
        backend
            .insert_edges_batch(vec![Edge::new(caller_id, callee_id, EdgeType::Calls)])
            .unwrap();

        let mut index = Index::new();
        let mut doc = Document::new();
        doc.relative_path = "src/a.rs".into();
        let mut caller_def = Occurrence::new();
        caller_def.symbol = "local caller().".into();
        caller_def.symbol_roles = ROLE_DEFINITION;
        caller_def.range = vec![0, 0, 9, 1];
        let mut target_def = Occurrence::new();
        target_def.symbol = "local target().".into();
        target_def.symbol_roles = ROLE_DEFINITION;
        target_def.range = vec![11, 0, 19, 1];
        let mut call_ref = Occurrence::new();
        call_ref.symbol = "local target().".into();
        call_ref.range = vec![3, 0, 3, 6];
        doc.occurrences.extend([caller_def, target_def, call_ref]);
        index.documents.push(doc);

        let stats = apply_scip_index(&mut backend, &index).unwrap();
        assert_eq!(stats.calls_added, 0);
    }
}
