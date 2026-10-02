//! Structured query primitives over mmap snapshots (no `MemoryBackend` hydrate).
//!
//! # Index complexity (honest)
//! - Exact name: O(1) hash via shared Arc `name_index` ([`ColumnarGraphMmap::indexes_shared`]).
//! - Prefix / suffix / contains name patterns: O(|name keys|) scan of the HashMap keys.
//! - `--scope` on `qualified_name`: per-candidate node filter (no dedicated prefix index yet).
//! - Seedless edge scan: sequential typed-edge walk via [`SnapshotNodeStore::for_each_edge`].
//! - Seeded multi-hop: builds a typed adjacency map once per invocation (O(E_type)).

use crate::schema::{EdgeType, Node, NodeType};
use crate::snapshot::SnapshotNodeStore;
use rgctl_error::{Error, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

/// JSON schema version for structured-query envelopes.
pub const STRUCTURED_QUERY_SCHEMA_VERSION: u32 = 1;

/// All [`NodeType`] variants for inventory zero-count emission.
pub const ALL_NODE_TYPES: &[NodeType] = &[
    NodeType::Function,
    NodeType::Class,
    NodeType::Struct,
    NodeType::Enum,
    NodeType::Interface,
    NodeType::Annotation,
    NodeType::Module,
    NodeType::Variable,
    NodeType::File,
    NodeType::ConfigKey,
    NodeType::TypeAlias,
    NodeType::Macro,
    NodeType::Import,
    NodeType::Table,
    NodeType::Dependency,
    NodeType::Job,
    NodeType::BuildStep,
    NodeType::AnsiblePlaybook,
    NodeType::AnsiblePlay,
    NodeType::AnsibleTask,
    NodeType::AnsibleRole,
    NodeType::AnsibleHandler,
    NodeType::AnsibleVariable,
    NodeType::AnsibleTemplate,
    NodeType::ChefCookbook,
    NodeType::ChefRecipe,
    NodeType::ChefResource,
    NodeType::ChefAttribute,
    NodeType::ChefTemplate,
    NodeType::ChefCustomResource,
    NodeType::PuppetModule,
    NodeType::PuppetClass,
    NodeType::PuppetDefinedType,
    NodeType::PuppetResource,
    NodeType::PuppetVariable,
    NodeType::PuppetFact,
    NodeType::PuppetNode,
    NodeType::KantraRuleset,
    NodeType::KantraRule,
];

/// All [`EdgeType`] variants except [`EdgeType::Unknown`] for inventory zeros.
pub const ALL_EDGE_TYPES: &[EdgeType] = &[
    EdgeType::Calls,
    EdgeType::Contains,
    EdgeType::Uses,
    EdgeType::Implements,
    EdgeType::Extends,
    EdgeType::References,
    EdgeType::Instantiates,
    EdgeType::Modifies,
    EdgeType::UsesConfig,
    EdgeType::DefinedIn,
    EdgeType::DependsOn,
    EdgeType::IncludesRole,
    EdgeType::DependsOnRole,
    EdgeType::ExecutesTask,
    EdgeType::NotifiesHandler,
    EdgeType::IncludesPlaybook,
    EdgeType::RendersTemplate,
    EdgeType::DependsOnCookbook,
    EdgeType::IncludesRecipe,
    EdgeType::DeclaresResource,
    EdgeType::UsesTemplate,
    EdgeType::DefinesAttribute,
    EdgeType::NotifiesResource,
    EdgeType::DependsOnModule,
    EdgeType::IncludesClass,
    EdgeType::InheritsClass,
    EdgeType::RequiresResource,
    EdgeType::UsesFact,
    EdgeType::AnnotatedWith,
    EdgeType::Permits,
    EdgeType::Violates,
];

/// How `--scope` filters `qualified_name` prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScopeMode {
    /// Keep nodes/endpoints whose FQN starts with the prefix.
    #[default]
    Inside,
    /// Keep nodes/endpoints whose FQN does not start with the prefix.
    Outside,
    /// For edges: keep pairs that straddle the prefix boundary.
    Crossing,
}

impl ScopeMode {
    /// Parse CLI token.
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "inside" | "in" => Ok(Self::Inside),
            "outside" | "out" => Ok(Self::Outside),
            "crossing" | "cross" => Ok(Self::Crossing),
            other => Err(Error::InvalidQuery(format!(
                "unknown scope-mode '{other}' (expected inside|outside|crossing)"
            ))),
        }
    }
}

/// Lean entity row for JSON.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EntityRow {
    /// Bare name
    pub name: String,
    /// Fully qualified name when present
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    /// Node type (lowercase CLI form)
    #[serde(rename = "type")]
    pub node_type: String,
    /// Source file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Definition line
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// Node UUID (string)
    pub id: String,
}

/// Keyed edge row (direction-stable; never positional).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EdgeRow {
    /// Edge origin
    pub source: EntityRow,
    /// Edge type (lowercase CLI form)
    pub edge: String,
    /// Traversal direction for this emission
    pub direction: String,
    /// Edge destination
    pub target: EntityRow,
    /// Hop distance from seed (1 for seedless scans)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hops: Option<usize>,
}

/// Find / list result envelope.
#[derive(Debug, Clone, Serialize)]
pub struct FindResult {
    /// Schema version
    pub schema_version: u32,
    /// Emitted row count
    pub returned: usize,
    /// Matches before limit when known
    pub total: usize,
    /// Entity rows (empty when count_only)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<EntityRow>,
}

/// Callers / callees envelope.
#[derive(Debug, Clone, Serialize)]
pub struct CallNeighborsResult {
    /// Schema version
    pub schema_version: u32,
    /// Resolved seed
    pub target: EntityRow,
    /// Neighbor rows
    pub neighbors: Vec<EntityRow>,
    /// Hop labels aligned with neighbors
    pub hops: Vec<usize>,
    /// Emitted neighbor count
    pub returned: usize,
    /// Total neighbors before limit
    pub total: usize,
    /// Requested depth
    pub depth: usize,
    /// `callers` or `callees`
    pub direction: String,
}

/// Relations envelope.
#[derive(Debug, Clone, Serialize)]
pub struct RelationsResult {
    /// Schema version
    pub schema_version: u32,
    /// Optional resolved seed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<EntityRow>,
    /// Keyed edges
    pub edges: Vec<EdgeRow>,
    /// Emitted count
    pub returned: usize,
    /// Total before limit
    pub total: usize,
}

/// Inventory envelope.
#[derive(Debug, Clone, Serialize)]
pub struct InventoryResult {
    /// Schema version
    pub schema_version: u32,
    /// Aggregation dimension
    pub by: String,
    /// Counts (includes zeros for type/edge)
    pub counts: Vec<InventoryCount>,
}

/// One inventory bucket.
#[derive(Debug, Clone, Serialize)]
pub struct InventoryCount {
    /// Bucket key (type/edge/lang/file/community)
    pub key: String,
    /// Count (may be zero)
    pub count: usize,
}

/// Filters shared by find / resolve.
#[derive(Debug, Clone, Default)]
pub struct QueryFilters {
    /// Node type filter
    pub node_type: Option<NodeType>,
    /// Path glob (`*` wildcards; `?` single char)
    pub file_glob: Option<String>,
    /// Language id (property `language` or file extension heuristic)
    pub lang: Option<String>,
    /// qualified_name prefix
    pub scope: Option<String>,
    /// Scope mode
    pub scope_mode: ScopeMode,
    /// Enclosing class/type name filter
    pub class: Option<String>,
    /// Max rows (None = unbounded)
    pub limit: Option<usize>,
    /// Count only
    pub count_only: bool,
    /// Exact name match (no glob)
    pub exact: bool,
}

/// Session over an open snapshot store.
pub struct StructuredQuery<'a> {
    store: &'a SnapshotNodeStore,
}

impl<'a> StructuredQuery<'a> {
    /// Borrow a snapshot store (must already be open; no hydrate).
    pub fn new(store: &'a SnapshotNodeStore) -> Self {
        Self { store }
    }

    /// Documented serve offload contract: call heavy methods from `spawn_blocking`.
    pub fn serve_offload_note() -> &'static str {
        "When invoking StructuredQuery from rgctl serve, run find/callers/relations/inventory on spawn_blocking"
    }

    fn project(node: &Node) -> EntityRow {
        EntityRow {
            name: node.name.to_string(),
            qualified_name: node.qualified_name.as_ref().map(|s| s.to_string()),
            node_type: node_type_cli(node.node_type),
            file: node.file_path.as_ref().map(|s| s.to_string()),
            line: node.start_line,
            id: node.id.to_string(),
        }
    }

    fn matches_scope(node: &Node, scope: Option<&str>, mode: ScopeMode) -> bool {
        let Some(prefix) = scope else {
            return mode != ScopeMode::Crossing;
        };
        let inside = node_in_scope(node, prefix);
        match mode {
            ScopeMode::Inside => inside,
            ScopeMode::Outside => !inside,
            // Node-level crossing is meaningless; treat as inside for node filters.
            ScopeMode::Crossing => inside,
        }
    }

    fn edge_scope_ok(src: &Node, dst: &Node, scope: Option<&str>, mode: ScopeMode) -> bool {
        let Some(prefix) = scope else {
            return true;
        };
        let s = node_in_scope(src, prefix);
        let d = node_in_scope(dst, prefix);
        match mode {
            // Migration-friendly: keep edges whose *source* is in-scope (annotations /
            // base types are often external). Use Crossing for true boundary edges.
            ScopeMode::Inside => s,
            ScopeMode::Outside => !s,
            ScopeMode::Crossing => s != d,
        }
    }

    fn matches_file(node: &Node, glob: Option<&str>) -> bool {
        let Some(raw) = glob else {
            return true;
        };
        let pat = normalize_file_glob(raw);
        let path = node.file_path.as_deref().unwrap_or("");
        if glob_match(&pat, path) {
            return true;
        }
        // Basename fallback when pattern has no path separator.
        if !raw.contains('/') && !raw.contains('\\') {
            let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
            return glob_match(&pat, base) || glob_match(raw, base);
        }
        false
    }

    fn matches_lang(node: &Node, lang: Option<&str>) -> bool {
        let Some(want) = lang else {
            return true;
        };
        let want = want.to_ascii_lowercase();
        if let Some(v) = node.get_property("language") {
            return v.eq_ignore_ascii_case(&want);
        }
        let ext = node
            .file_path
            .as_deref()
            .and_then(|p| p.rsplit('.').next())
            .unwrap_or("");
        lang_from_ext(ext).eq_ignore_ascii_case(&want)
    }

    fn matches_class(node: &Node, class: Option<&str>) -> bool {
        let Some(c) = class else {
            return true;
        };
        if let Some(qn) = node.qualified_name.as_deref() {
            if qn.contains(c) {
                return true;
            }
        }
        if let Some(v) = node.get_property("class") {
            return v == c;
        }
        if let Some(v) = node.get_property("enclosing_type") {
            return v == c;
        }
        false
    }

    fn node_passes(&self, node: &Node, f: &QueryFilters) -> bool {
        if let Some(t) = f.node_type
            && node.node_type != t
        {
            return false;
        }
        if !Self::matches_file(node, f.file_glob.as_deref()) {
            return false;
        }
        if !Self::matches_lang(node, f.lang.as_deref()) {
            return false;
        }
        if !Self::matches_scope(node, f.scope.as_deref(), f.scope_mode) {
            return false;
        }
        if !Self::matches_class(node, f.class.as_deref()) {
            return false;
        }
        true
    }

    fn indexes(
        &self,
    ) -> Result<Option<Arc<(HashMap<String, Vec<Uuid>>, HashMap<NodeType, Vec<Uuid>>)>>> {
        Ok(self
            .store
            .columnar()
            .map(|c| c.indexes_shared())
            .transpose()?)
    }

    /// Resolve a unique symbol or return ambiguity / not-found errors.
    pub fn resolve_symbol(
        &self,
        symbol: &str,
        filters: &QueryFilters,
    ) -> Result<Node> {
        let mut matches = self.lookup_name_candidates(symbol, filters.exact)?;
        matches.retain(|n| self.node_passes(n, filters));
        match matches.len() {
            0 => Err(Error::NodeNotFound(symbol.to_string())),
            1 => Ok(matches.remove(0)),
            n => Err(Error::AmbiguousSymbol {
                name: symbol.to_string(),
                count: n,
            }),
        }
    }

    fn lookup_name_candidates(&self, symbol: &str, exact: bool) -> Result<Vec<Node>> {
        if let Ok(uuid) = Uuid::parse_str(symbol) {
            if let Some(n) = self.store.get_node(uuid)? {
                return Ok(vec![n]);
            }
        }
        if exact || !is_glob_pattern(symbol) {
            if let Some(indexes) = self.indexes()? {
                if let Some(ids) = indexes.0.get(symbol) {
                    let mut out = Vec::with_capacity(ids.len());
                    for id in ids {
                        if let Some(n) = self.store.get_node(*id)? {
                            out.push(n);
                        }
                    }
                    if !out.is_empty() {
                        return Ok(out);
                    }
                }
            } else {
                return self.store.find_nodes_by_name(symbol);
            }
            // Fall through: also try qualified_name exact via type scan is expensive; return empty.
            return Ok(Vec::new());
        }
        // Glob over name keys (O(|keys|)).
        let Some(indexes) = self.indexes()? else {
            return Err(Error::GraphError(
                "name glob requires columnar snapshot indexes".into(),
            ));
        };
        let mut out = Vec::new();
        for (name, ids) in indexes.0.iter() {
            if glob_match(symbol, name) {
                for id in ids {
                    if let Some(n) = self.store.get_node(*id)? {
                        out.push(n);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Entity search (`rgctl find`).
    pub fn find(&self, pattern: Option<&str>, filters: &QueryFilters) -> Result<FindResult> {
        let mut candidates: Vec<Node> = Vec::new();
        if let Some(pat) = pattern {
            candidates = self.lookup_name_candidates(pat, filters.exact)?;
        } else if let Some(t) = filters.node_type {
            if let Some(indexes) = self.indexes()? {
                if let Some(ids) = indexes.1.get(&t) {
                    candidates.reserve(ids.len());
                    for id in ids {
                        if let Some(n) = self.store.get_node(*id)? {
                            candidates.push(n);
                        }
                    }
                }
            } else {
                for id in self.store.all_node_ids() {
                    if let Some(n) = self.store.get_node(id)? {
                        if n.node_type == t {
                            candidates.push(n);
                        }
                    }
                }
            }
        } else {
            for id in self.store.all_node_ids() {
                if let Some(n) = self.store.get_node(id)? {
                    candidates.push(n);
                }
            }
        }

        candidates.retain(|n| self.node_passes(n, filters));
        let total = candidates.len();
        let limit = filters.limit.unwrap_or(total);
        let entities: Vec<EntityRow> = if filters.count_only {
            Vec::new()
        } else {
            candidates
                .into_iter()
                .take(limit)
                .map(|n| Self::project(&n))
                .collect()
        };
        let returned = if filters.count_only {
            total.min(limit)
        } else {
            entities.len()
        };
        Ok(FindResult {
            schema_version: STRUCTURED_QUERY_SCHEMA_VERSION,
            returned,
            total,
            entities,
        })
    }

    /// Incoming (`callers`) or outgoing (`callees`) CALLS.
    pub fn call_neighbors(
        &self,
        symbol: &str,
        incoming: bool,
        depth: usize,
        filters: &QueryFilters,
    ) -> Result<CallNeighborsResult> {
        let seed = self.resolve_symbol(symbol, filters)?;
        let depth = depth.max(1);
        let adj = self.build_typed_adjacency(EdgeType::Calls)?;
        let mut seen = HashSet::from([seed.id]);
        let mut q = VecDeque::from([(seed.id, 0usize)]);
        let mut found: Vec<(Uuid, usize)> = Vec::new();
        while let Some((id, d)) = q.pop_front() {
            if d >= depth {
                continue;
            }
            let nexts = if incoming {
                adj.incoming.get(&id)
            } else {
                adj.outgoing.get(&id)
            };
            let Some(nexts) = nexts else { continue };
            for &nid in nexts {
                if !seen.insert(nid) {
                    continue;
                }
                let hop = d + 1;
                found.push((nid, hop));
                if hop < depth {
                    q.push_back((nid, hop));
                }
            }
        }

        let mut neighbors = Vec::new();
        let mut hops = Vec::new();
        for (id, hop) in &found {
            let Some(n) = self.store.get_node(*id)? else {
                continue;
            };
            if !self.node_passes(&n, filters) && filters.scope.is_some() {
                // For callers, scope typically filters the *neighbor* side.
                if !Self::matches_scope(&n, filters.scope.as_deref(), filters.scope_mode) {
                    continue;
                }
            }
            neighbors.push(Self::project(&n));
            hops.push(*hop);
        }
        let total = neighbors.len();
        let limit = filters.limit.unwrap_or(total);
        if neighbors.len() > limit {
            neighbors.truncate(limit);
            hops.truncate(limit);
        }
        Ok(CallNeighborsResult {
            schema_version: STRUCTURED_QUERY_SCHEMA_VERSION,
            target: Self::project(&seed),
            returned: neighbors.len(),
            total,
            neighbors,
            hops,
            depth,
            direction: if incoming {
                "callers".into()
            } else {
                "callees".into()
            },
        })
    }

    /// Seeded or seedless relations.
    pub fn relations(
        &self,
        symbol: Option<&str>,
        edge: EdgeType,
        direction: RelationDirection,
        from_type: Option<NodeType>,
        to_type: Option<NodeType>,
        depth: usize,
        filters: &QueryFilters,
    ) -> Result<RelationsResult> {
        if let Some(sym) = symbol {
            return self.relations_seeded(sym, edge, direction, from_type, to_type, depth, filters);
        }
        self.relations_seedless(edge, direction, from_type, to_type, filters)
    }

    fn relations_seedless(
        &self,
        edge: EdgeType,
        direction: RelationDirection,
        from_type: Option<NodeType>,
        to_type: Option<NodeType>,
        filters: &QueryFilters,
    ) -> Result<RelationsResult> {
        let mut edges = Vec::new();
        let mut total = 0usize;
        let limit = filters.limit.unwrap_or(usize::MAX);
        self.store.for_each_edge(|from, to, et| {
            if et != edge {
                return Ok(());
            }
            let Some(src) = self.store.get_node(from)? else {
                return Ok(());
            };
            let Some(dst) = self.store.get_node(to)? else {
                return Ok(());
            };
            if let Some(ft) = from_type
                && src.node_type != ft
            {
                return Ok(());
            }
            if let Some(tt) = to_type
                && dst.node_type != tt
            {
                return Ok(());
            }
            if !Self::edge_scope_ok(&src, &dst, filters.scope.as_deref(), filters.scope_mode) {
                return Ok(());
            }
            // Emit according to direction (both = outbound orientation as stored).
            let emit = match direction {
                RelationDirection::Out | RelationDirection::Both => true,
                RelationDirection::In => true, // still emit keyed as stored; direction field notes "in" view
            };
            if !emit {
                return Ok(());
            }
            total += 1;
            if edges.len() < limit {
                let (source, target, dir_label) = match direction {
                    RelationDirection::In => (Self::project(&dst), Self::project(&src), "in"),
                    RelationDirection::Out | RelationDirection::Both => {
                        (Self::project(&src), Self::project(&dst), "out")
                    }
                };
                edges.push(EdgeRow {
                    source,
                    edge: edge_type_cli(edge),
                    direction: dir_label.into(),
                    target,
                    hops: Some(1),
                });
            }
            Ok(())
        })?;
        Ok(RelationsResult {
            schema_version: STRUCTURED_QUERY_SCHEMA_VERSION,
            target: None,
            returned: edges.len(),
            total,
            edges,
        })
    }

    fn relations_seeded(
        &self,
        symbol: &str,
        edge: EdgeType,
        direction: RelationDirection,
        from_type: Option<NodeType>,
        to_type: Option<NodeType>,
        depth: usize,
        filters: &QueryFilters,
    ) -> Result<RelationsResult> {
        let seed = self.resolve_symbol(symbol, filters)?;
        let depth = depth.max(1);
        let adj = self.build_typed_adjacency(edge)?;
        let mut rows = Vec::new();
        let mut total = 0usize;
        let limit = filters.limit.unwrap_or(usize::MAX);

        let walk = |incoming: bool, rows: &mut Vec<EdgeRow>, total: &mut usize| -> Result<()> {
            let mut seen = HashSet::from([seed.id]);
            let mut q = VecDeque::from([(seed.id, 0usize)]);
            while let Some((id, d)) = q.pop_front() {
                if d >= depth {
                    continue;
                }
                let nexts = if incoming {
                    adj.incoming.get(&id)
                } else {
                    adj.outgoing.get(&id)
                };
                let Some(nexts) = nexts else { continue };
                for &nid in nexts {
                    let hop = d + 1;
                    let (src_id, dst_id) = if incoming { (nid, id) } else { (id, nid) };
                    let Some(src) = self.store.get_node(src_id)? else {
                        continue;
                    };
                    let Some(dst) = self.store.get_node(dst_id)? else {
                        continue;
                    };
                    if let Some(ft) = from_type
                        && src.node_type != ft
                    {
                        continue;
                    }
                    if let Some(tt) = to_type
                        && dst.node_type != tt
                    {
                        continue;
                    }
                    if !Self::edge_scope_ok(&src, &dst, filters.scope.as_deref(), filters.scope_mode)
                    {
                        continue;
                    }
                    *total += 1;
                    if rows.len() < limit {
                        rows.push(EdgeRow {
                            source: Self::project(&src),
                            edge: edge_type_cli(edge),
                            direction: if incoming { "in" } else { "out" }.into(),
                            target: Self::project(&dst),
                            hops: Some(hop),
                        });
                    }
                    if seen.insert(nid) && hop < depth {
                        q.push_back((nid, hop));
                    }
                }
            }
            Ok(())
        };

        match direction {
            RelationDirection::Out => walk(false, &mut rows, &mut total)?,
            RelationDirection::In => walk(true, &mut rows, &mut total)?,
            RelationDirection::Both => {
                walk(false, &mut rows, &mut total)?;
                walk(true, &mut rows, &mut total)?;
            }
        }

        Ok(RelationsResult {
            schema_version: STRUCTURED_QUERY_SCHEMA_VERSION,
            target: Some(Self::project(&seed)),
            returned: rows.len(),
            total,
            edges: rows,
        })
    }

    /// Inventory with zero-count enums for type/edge.
    pub fn inventory(
        &self,
        by: InventoryBy,
        filters: &QueryFilters,
    ) -> Result<InventoryResult> {
        match by {
            InventoryBy::Type => {
                let mut map: HashMap<NodeType, usize> =
                    ALL_NODE_TYPES.iter().map(|t| (*t, 0usize)).collect();
                for id in self.store.all_node_ids() {
                    let Some(n) = self.store.get_node(id)? else {
                        continue;
                    };
                    if !self.node_passes(&n, filters) {
                        continue;
                    }
                    *map.entry(n.node_type).or_insert(0) += 1;
                }
                let mut counts: Vec<InventoryCount> = ALL_NODE_TYPES
                    .iter()
                    .map(|t| InventoryCount {
                        key: node_type_cli(*t),
                        count: *map.get(t).unwrap_or(&0),
                    })
                    .collect();
                // Include any unexpected types not in ALL_NODE_TYPES.
                for (t, c) in map {
                    if !ALL_NODE_TYPES.contains(&t) {
                        counts.push(InventoryCount {
                            key: node_type_cli(t),
                            count: c,
                        });
                    }
                }
                Ok(InventoryResult {
                    schema_version: STRUCTURED_QUERY_SCHEMA_VERSION,
                    by: "type".into(),
                    counts,
                })
            }
            InventoryBy::Edge => {
                let mut map: HashMap<EdgeType, usize> =
                    ALL_EDGE_TYPES.iter().map(|t| (*t, 0usize)).collect();
                self.store.for_each_edge(|from, to, et| {
                    if et == EdgeType::Unknown {
                        return Ok(());
                    }
                    if filters.scope.is_some() {
                        let Some(src) = self.store.get_node(from)? else {
                            return Ok(());
                        };
                        let Some(dst) = self.store.get_node(to)? else {
                            return Ok(());
                        };
                        if !Self::edge_scope_ok(
                            &src,
                            &dst,
                            filters.scope.as_deref(),
                            filters.scope_mode,
                        ) {
                            return Ok(());
                        }
                    }
                    *map.entry(et).or_insert(0) += 1;
                    Ok(())
                })?;
                let counts = ALL_EDGE_TYPES
                    .iter()
                    .map(|t| InventoryCount {
                        key: edge_type_cli(*t),
                        count: *map.get(t).unwrap_or(&0),
                    })
                    .collect();
                Ok(InventoryResult {
                    schema_version: STRUCTURED_QUERY_SCHEMA_VERSION,
                    by: "edge".into(),
                    counts,
                })
            }
            InventoryBy::Lang => {
                let mut map: HashMap<String, usize> = HashMap::new();
                for id in self.store.all_node_ids() {
                    let Some(n) = self.store.get_node(id)? else {
                        continue;
                    };
                    if !self.node_passes(&n, filters) {
                        continue;
                    }
                    let lang = n
                        .get_property("language")
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            let ext = n
                                .file_path
                                .as_deref()
                                .and_then(|p| p.rsplit('.').next())
                                .unwrap_or("");
                            lang_from_ext(ext)
                        });
                    *map.entry(lang).or_insert(0) += 1;
                }
                let mut counts: Vec<_> = map
                    .into_iter()
                    .map(|(key, count)| InventoryCount { key, count })
                    .collect();
                counts.sort_by(|a, b| a.key.cmp(&b.key));
                Ok(InventoryResult {
                    schema_version: STRUCTURED_QUERY_SCHEMA_VERSION,
                    by: "lang".into(),
                    counts,
                })
            }
            InventoryBy::File => {
                let mut map: HashMap<String, usize> = HashMap::new();
                for id in self.store.all_node_ids() {
                    let Some(n) = self.store.get_node(id)? else {
                        continue;
                    };
                    if !self.node_passes(&n, filters) {
                        continue;
                    }
                    let key = n
                        .file_path
                        .as_deref()
                        .unwrap_or("<unknown>")
                        .to_string();
                    *map.entry(key).or_insert(0) += 1;
                }
                let mut counts: Vec<_> = map
                    .into_iter()
                    .map(|(key, count)| InventoryCount { key, count })
                    .collect();
                counts.sort_by(|a, b| a.key.cmp(&b.key));
                Ok(InventoryResult {
                    schema_version: STRUCTURED_QUERY_SCHEMA_VERSION,
                    by: "file".into(),
                    counts,
                })
            }
            InventoryBy::Community => {
                let mut map: HashMap<String, usize> = HashMap::new();
                for id in self.store.all_node_ids() {
                    let Some(n) = self.store.get_node(id)? else {
                        continue;
                    };
                    if !self.node_passes(&n, filters) {
                        continue;
                    }
                    let key = n
                        .get_property("community_id")
                        .unwrap_or("<none>")
                        .to_string();
                    *map.entry(key).or_insert(0) += 1;
                }
                let mut counts: Vec<_> = map
                    .into_iter()
                    .map(|(key, count)| InventoryCount { key, count })
                    .collect();
                counts.sort_by(|a, b| a.key.cmp(&b.key));
                Ok(InventoryResult {
                    schema_version: STRUCTURED_QUERY_SCHEMA_VERSION,
                    by: "community".into(),
                    counts,
                })
            }
        }
    }

    fn build_typed_adjacency(&self, edge: EdgeType) -> Result<TypedAdj> {
        let mut outgoing: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        let mut incoming: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        self.store.for_each_edge(|from, to, et| {
            if et != edge {
                return Ok(());
            }
            outgoing.entry(from).or_default().push(to);
            incoming.entry(to).or_default().push(from);
            Ok(())
        })?;
        Ok(TypedAdj { outgoing, incoming })
    }
}

struct TypedAdj {
    outgoing: HashMap<Uuid, Vec<Uuid>>,
    incoming: HashMap<Uuid, Vec<Uuid>>,
}

/// Relation walk direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationDirection {
    /// Follow edge as stored (from → to)
    Out,
    /// Reverse (to → from)
    In,
    /// Both
    Both,
}

impl RelationDirection {
    /// Parse CLI token.
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "out" | "outgoing" => Ok(Self::Out),
            "in" | "incoming" => Ok(Self::In),
            "both" => Ok(Self::Both),
            other => Err(Error::InvalidQuery(format!(
                "unknown direction '{other}' (expected in|out|both)"
            ))),
        }
    }
}

/// Inventory dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryBy {
    /// Node types
    Type,
    /// Edge types
    Edge,
    /// Language
    Lang,
    /// File path
    File,
    /// Community id property
    Community,
}

impl InventoryBy {
    /// Parse CLI token.
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "type" => Ok(Self::Type),
            "edge" => Ok(Self::Edge),
            "lang" | "language" => Ok(Self::Lang),
            "file" => Ok(Self::File),
            "community" => Ok(Self::Community),
            other => Err(Error::InvalidQuery(format!(
                "unknown inventory --by '{other}' (expected type|edge|lang|file|community)"
            ))),
        }
    }
}

/// Parse CLI node type string.
pub fn parse_node_type(s: &str) -> Result<NodeType> {
    let key = s.to_ascii_lowercase().replace('-', "").replace('_', "");
    for t in ALL_NODE_TYPES {
        if node_type_cli(*t).replace('_', "") == key {
            return Ok(*t);
        }
    }
    // Common aliases
    match key.as_str() {
        "config" => Ok(NodeType::ConfigKey),
        "fn" | "method" => Ok(NodeType::Function),
        "trait" => Ok(NodeType::Interface),
        _ => Err(Error::InvalidQuery(format!(
            "unknown node type '{s}' (e.g. function, class, import, annotation)"
        ))),
    }
}

/// Parse CLI edge type string.
pub fn parse_edge_type(s: &str) -> Result<EdgeType> {
    let key = s.to_ascii_lowercase().replace('-', "").replace('_', "");
    for t in ALL_EDGE_TYPES {
        if edge_type_cli(*t).replace('_', "") == key {
            return Ok(*t);
        }
    }
    Err(Error::InvalidQuery(format!(
        "unknown edge type '{s}' (e.g. calls, uses, extends, implements, annotatedwith)"
    )))
}

fn node_type_cli(t: NodeType) -> String {
    format!("{t:?}").to_ascii_lowercase()
}

fn edge_type_cli(t: EdgeType) -> String {
    format!("{t:?}").to_ascii_lowercase()
}

fn is_glob_pattern(s: &str) -> bool {
    s.contains('*') || s.contains('?')
}

/// Normalize scope keys so `/`, `\`, and `.` compare equivalently (PHP vs POSIX).
pub fn normalize_scope_key(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' | '/' | '.' => '/',
            other => other,
        })
        .collect()
}

/// True when node's FQN or (if missing) file_path matches the scope prefix.
///
/// Go/TS often leave `qualified_name` unset on types; package identity lives in `file_path`.
fn node_in_scope(node: &Node, scope_prefix: &str) -> bool {
    let want = normalize_scope_key(scope_prefix);
    if want.is_empty() {
        return true;
    }
    if let Some(qn) = node.qualified_name.as_deref().filter(|s| !s.is_empty()) {
        let hay = normalize_scope_key(qn);
        return hay.starts_with(&want);
    }
    // Fallback when qualified_name is absent (common for Go/TS types).
    let path = node.file_path.as_deref().unwrap_or("");
    if path.is_empty() {
        return false;
    }
    let hay = normalize_scope_key(path);
    path_contains_scope_segment(&hay, &want)
}

/// Match `want` as a path segment / prefix inside a normalized file path.
fn path_contains_scope_segment(hay: &str, want: &str) -> bool {
    if hay == want || hay.starts_with(&format!("{want}/")) || hay.ends_with(&format!("/{want}")) {
        return true;
    }
    hay.contains(&format!("/{want}/"))
}

/// If `--file` has no directory separator and no leading `*`, treat as basename glob.
pub fn normalize_file_glob(raw: &str) -> String {
    if raw.is_empty() {
        return raw.to_string();
    }
    if raw.contains('/') || raw.contains('\\') || raw.starts_with('*') {
        return raw.to_string();
    }
    format!("*{raw}")
}

/// Glob match with `*` and `?` (not full regex).
pub fn glob_match(pattern: &str, value: &str) -> bool {
    glob_match_rec(pattern.as_bytes(), value.as_bytes())
}

fn glob_match_rec(pat: &[u8], val: &[u8]) -> bool {
    let mut pi = 0usize;
    let mut vi = 0usize;
    let mut star_p = None;
    let mut star_v = 0usize;
    while vi < val.len() {
        if pi < pat.len() && (pat[pi] == b'?' || pat[pi] == val[vi]) {
            pi += 1;
            vi += 1;
        } else if pi < pat.len() && pat[pi] == b'*' {
            star_p = Some(pi);
            star_v = vi;
            pi += 1;
        } else if let Some(sp) = star_p {
            pi = sp + 1;
            star_v += 1;
            vi = star_v;
        } else {
            return false;
        }
    }
    while pi < pat.len() && pat[pi] == b'*' {
        pi += 1;
    }
    pi == pat.len()
}

fn lang_from_ext(ext: &str) -> String {
    match ext.to_ascii_lowercase().as_str() {
        "rs" => "rust".into(),
        "java" => "java".into(),
        "c" | "h" => "c".into(),
        "cc" | "cpp" | "cxx" | "hpp" => "cpp".into(),
        "py" => "python".into(),
        "go" => "go".into(),
        "js" | "mjs" | "cjs" => "javascript".into(),
        "ts" | "tsx" => "typescript".into(),
        "cs" => "csharp".into(),
        "rb" => "ruby".into(),
        "php" => "php".into(),
        "kt" | "kts" => "kotlin".into(),
        "groovy" => "groovy".into(),
        "pp" => "puppet".into(),
        "erb" => "erb".into(),
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{GraphBackend, MemoryBackend};
    use crate::columnar_snapshot::write_columnar_from_backend;
    use crate::schema::Edge;
    use crate::snapshot::SnapshotNodeStore;
    use tempfile::tempdir;

    fn sample_store() -> (tempfile::TempDir, SnapshotNodeStore) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("graph.snapshot.bin");
        let mut backend = MemoryBackend::new();
        let f1 = Node::new(NodeType::Function, "getNextPrintPackage")
            .with_qualified_name("de.metas.printing.esb.Svc.getNextPrintPackage")
            .with_file_path("src/Svc.java")
            .with_location(10, 20);
        let a1 = Node::new(NodeType::Annotation, "Path")
            .with_qualified_name("javax.ws.rs.Path")
            .with_file_path("<external>");
        let c1 = Node::new(NodeType::Class, "PRTRestServiceRoute")
            .with_qualified_name("de.metas.printing.esb.camel.PRTRestServiceRoute")
            .with_file_path("src/PRTRestServiceRoute.java")
            .with_location(1, 50);
        let base = Node::new(NodeType::Class, "RouteBuilder")
            .with_qualified_name("org.apache.camel.builder.RouteBuilder")
            .with_file_path("<external>");
        let imp = Node::new(NodeType::Import, "import javax.ws.rs.Path;")
            .with_file_path("src/Svc.java");
        let caller = Node::new(NodeType::Function, "handle")
            .with_qualified_name("de.metas.printing.esb.Svc.handle")
            .with_file_path("src/Svc.java")
            .with_location(30, 40);
        let f1_id = f1.id;
        let a1_id = a1.id;
        let c1_id = c1.id;
        let base_id = base.id;
        let caller_id = caller.id;
        backend.insert_node(f1).unwrap();
        backend.insert_node(a1).unwrap();
        backend.insert_node(c1).unwrap();
        backend.insert_node(base).unwrap();
        backend.insert_node(imp).unwrap();
        backend.insert_node(caller).unwrap();
        backend
            .insert_edge(Edge::new(f1_id, a1_id, EdgeType::AnnotatedWith))
            .unwrap();
        backend
            .insert_edge(Edge::new(c1_id, base_id, EdgeType::Extends))
            .unwrap();
        backend
            .insert_edge(Edge::new(caller_id, f1_id, EdgeType::Calls))
            .unwrap();
        write_columnar_from_backend(&backend, &path).unwrap();
        let store = SnapshotNodeStore::open(&path).unwrap();
        (dir, store)
    }

    #[test]
    fn seedless_annotatedwith_keyed_and_typed() {
        let (_dir, store) = sample_store();
        let q = StructuredQuery::new(&store);
        let res = q
            .relations(
                None,
                EdgeType::AnnotatedWith,
                RelationDirection::Out,
                Some(NodeType::Function),
                Some(NodeType::Annotation),
                1,
                &QueryFilters {
                    scope: Some("de.metas.printing.esb".into()),
                    scope_mode: ScopeMode::Inside,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(res.total, 1);
        assert_eq!(res.edges[0].source.node_type, "function");
        assert_eq!(res.edges[0].target.node_type, "annotation");
        assert_eq!(res.edges[0].source.name, "getNextPrintPackage");
        assert_eq!(res.edges[0].target.name, "Path");
    }

    #[test]
    fn extends_keyed_source_is_subtype() {
        let (_dir, store) = sample_store();
        let q = StructuredQuery::new(&store);
        let res = q
            .relations(
                None,
                EdgeType::Extends,
                RelationDirection::Out,
                Some(NodeType::Class),
                None,
                1,
                &QueryFilters::default(),
            )
            .unwrap();
        assert_eq!(res.total, 1);
        assert_eq!(res.edges[0].source.name, "PRTRestServiceRoute");
        assert_eq!(res.edges[0].target.name, "RouteBuilder");
    }

    #[test]
    fn inventory_type_includes_zeros() {
        let (_dir, store) = sample_store();
        let q = StructuredQuery::new(&store);
        let res = q
            .inventory(InventoryBy::Type, &QueryFilters::default())
            .unwrap();
        let config = res
            .counts
            .iter()
            .find(|c| c.key == "configkey")
            .expect("configkey present");
        assert_eq!(config.count, 0);
        let functions = res.counts.iter().find(|c| c.key == "function").unwrap();
        assert!(functions.count >= 2);
    }

    #[test]
    fn find_import_prefix() {
        let (_dir, store) = sample_store();
        let q = StructuredQuery::new(&store);
        let res = q
            .find(
                Some("import javax*"),
                &QueryFilters {
                    node_type: Some(NodeType::Import),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(res.total, 1);
        assert!(res.entities[0].name.starts_with("import javax"));
    }

    #[test]
    fn callers_depth_one() {
        let (_dir, store) = sample_store();
        let q = StructuredQuery::new(&store);
        let res = q
            .call_neighbors("getNextPrintPackage", true, 1, &QueryFilters::default())
            .unwrap();
        assert_eq!(res.returned, 1);
        assert_eq!(res.neighbors[0].name, "handle");
    }

    #[test]
    fn glob_contains() {
        assert!(glob_match("*Service*", "PRTRestServiceRoute"));
        assert!(!glob_match("*Service*", "Path"));
    }

    #[test]
    fn normalize_scope_separators() {
        assert_eq!(normalize_scope_key(r"App\Http"), "App/Http");
        assert_eq!(normalize_scope_key("App.Http"), "App/Http");
        assert_eq!(normalize_scope_key("App/Http"), "App/Http");
    }

    #[test]
    fn normalize_file_basename_glob() {
        assert_eq!(normalize_file_glob("PRTRestServiceRoute.java"), "*PRTRestServiceRoute.java");
        assert_eq!(
            normalize_file_glob("*/printing/**/*.java"),
            "*/printing/**/*.java"
        );
        assert_eq!(normalize_file_glob("*Route.java"), "*Route.java");
    }

    #[test]
    fn scope_falls_back_to_file_path_when_qn_absent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snap.bin");
        let mut backend = MemoryBackend::new();
        let n = Node::new(NodeType::Function, "ServeHTTP")
            .with_file_path("/tmp/pkg/util/handler.go");
        // No qualified_name — Go/TS style.
        backend.insert_node(n).unwrap();
        write_columnar_from_backend(&backend, &path).unwrap();
        let store = SnapshotNodeStore::open(&path).unwrap();
        let q = StructuredQuery::new(&store);
        let hit = q
            .find(
                Some("ServeHTTP"),
                &QueryFilters {
                    scope: Some("pkg/util".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(hit.total, 1);
        let miss = q
            .find(
                Some("ServeHTTP"),
                &QueryFilters {
                    scope: Some("pkg/other".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(miss.total, 0);
    }

    #[test]
    fn scope_php_backslash_matches_slash_qn() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snap.bin");
        let mut backend = MemoryBackend::new();
        let n = Node::new(NodeType::Class, "Controller")
            .with_qualified_name(r"App\Http\Controllers\Controller");
        backend.insert_node(n).unwrap();
        write_columnar_from_backend(&backend, &path).unwrap();
        let store = SnapshotNodeStore::open(&path).unwrap();
        let q = StructuredQuery::new(&store);
        let hit = q
            .find(
                Some("Controller"),
                &QueryFilters {
                    scope: Some("App/Http".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(hit.total, 1);
    }

    #[test]
    fn file_filter_accepts_basename() {
        let (_dir, store) = sample_store();
        let q = StructuredQuery::new(&store);
        let res = q
            .find(
                Some("*Service*"),
                &QueryFilters {
                    file_glob: Some("PRTRestServiceRoute.java".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(res.total, 1);
        assert_eq!(res.entities[0].name, "PRTRestServiceRoute");
    }

    #[test]
    fn reject_unknown_edge_and_type() {
        assert!(parse_edge_type("not_a_real_edge").is_err());
        assert!(parse_node_type("not_a_type").is_err());
        assert!(parse_edge_type("annotatedwith").is_ok());
        assert!(parse_node_type("import").is_ok());
    }
}
