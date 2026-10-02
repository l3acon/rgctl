//! Structured query CLI: `find`, `callers`, `callees`, `relations`, `inventory`.

use super::context::CliContext;
use super::OutputFormat;
use anyhow::{Context, Result};
use rgctl_error::Error as GraphError;
use rgctl_graph::{
    parse_edge_type, parse_node_type, InventoryBy, QueryFilters, RelationDirection, ScopeMode,
    STRUCTURED_QUERY_SCHEMA_VERSION, StructuredQuery,
};

/// Shared filter flags for structured query verbs.
#[derive(Debug, Clone, Default)]
pub struct SharedQueryArgs {
    pub file: Option<String>,
    pub class: Option<String>,
    pub line: Option<usize>,
    pub scope: Option<String>,
    pub scope_mode: Option<String>,
    pub exclude_scope: bool,
    pub lang: Option<String>,
    pub limit: Option<usize>,
    /// Package coords (`--package`); resolved to import prefix / scope.
    pub package: Option<String>,
    /// Optional method name filters for callers (`--methods`).
    pub methods: Option<Vec<String>>,
}

impl SharedQueryArgs {
    fn into_filters(self, node_type: Option<rgctl_graph::schema::NodeType>) -> Result<QueryFilters> {
        let scope_mode = if self.exclude_scope {
            ScopeMode::Outside
        } else if let Some(m) = self.scope_mode.as_deref() {
            ScopeMode::parse(m).map_err(|e| anyhow::anyhow!("{e}"))?
        } else {
            ScopeMode::Inside
        };
        Ok(QueryFilters {
            node_type,
            file_glob: self.file,
            lang: self.lang,
            scope: self.scope,
            scope_mode,
            class: self.class,
            line: self.line,
            limit: self.limit,
            count_only: false,
            exact: false,
            annotation_names: None,
            show_attributes: false,
            annotation_arg_index: None,
        })
    }
}

fn open_store(ctx: &CliContext) -> Result<std::sync::Arc<rgctl_graph::SnapshotNodeStore>> {
    ctx.open_snapshot_store()?
        .context("Graph snapshot not found (run `rgctl discover` first)")
}

/// Load `.rgctl/annotation_args.json` into a lookup keyed by `source_id\0annotation`.
fn load_annotation_arg_index(repo: &std::path::Path) -> Option<std::collections::HashMap<String, String>> {
    let path = rgctl_graph::paths::artifact_path(repo, "annotation_args.json");
    let bytes = std::fs::read(&path).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let entries = v.get("entries")?.as_array()?;
    let mut map = std::collections::HashMap::new();
    for e in entries {
        let source_id = e.get("source_id")?.as_str()?;
        let annotation = e.get("annotation")?.as_str()?;
        let arguments = e.get("arguments")?.as_str()?;
        let simple = rgctl_graph::parse_annotation_list(annotation)
            .into_iter()
            .next()
            .unwrap_or_else(|| annotation.to_string());
        map.insert(format!("{source_id}\0{simple}"), arguments.to_string());
    }
    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

fn emit_json<T: serde::Serialize>(ctx: &CliContext, value: &T) -> Result<()> {
    let v = serde_json::to_value(value)?;
    ctx.emit_json_value(&v)?;
    Ok(())
}

fn emit_text_entities(ctx: &CliContext, names: impl IntoIterator<Item = String>) -> Result<()> {
    for name in names {
        ctx.stdout_line(&name)?;
    }
    Ok(())
}

/// Map graph errors; under `-f json`, emit an `ambiguous_symbol` envelope on stdout.
fn map_sq_err(ctx: &CliContext, err: GraphError) -> anyhow::Error {
    if let GraphError::AmbiguousSymbol {
        name,
        count,
        candidates,
    } = &err
    {
        if ctx.format == OutputFormat::Json {
            let envelope = serde_json::json!({
                "schema_version": STRUCTURED_QUERY_SCHEMA_VERSION,
                "error": "ambiguous_symbol",
                "name": name,
                "count": count,
                "candidates": candidates,
            });
            let _ = ctx.emit_json_value(&envelope);
        } else if !candidates.is_empty() {
            eprintln!("Ambiguous symbol '{name}': {count} matches. Candidates:");
            for c in candidates.iter().take(20) {
                let file = c.file.as_deref().unwrap_or("?");
                let line = c
                    .line
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into());
                let qn = c.qualified_name.as_deref().unwrap_or("");
                eprintln!(
                    "  - id={} type={} file={file}:{line} name={} {qn}",
                    c.id, c.node_type, c.name
                );
            }
            eprintln!("Disambiguate with --file, --class, and/or --line.");
        }
    }
    anyhow::anyhow!("{err}")
}

/// `rgctl find`
pub fn run_find(
    ctx: &CliContext,
    pattern: Option<String>,
    type_name: Option<String>,
    shared: SharedQueryArgs,
    exact: bool,
    count_only: bool,
    annotation: Option<String>,
    show_attributes: bool,
) -> Result<()> {
    let package_coords = shared.package.clone();
    let mut pattern = pattern;
    let mut shared = shared;
    let mut package_resolution = None;
    if let Some(ref coords) = package_coords {
        let resolved = rgctl_security::resolve_package(coords)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if pattern.is_none() {
            // Prefer first import prefix as find pattern (glob-friendly).
            if let Some(prefix) = resolved.import_prefixes.first() {
                pattern = Some(format!("{prefix}*"));
            }
        }
        if shared.scope.is_none() {
            if let Some(prefix) = resolved.import_prefixes.first() {
                shared.scope = Some(prefix.clone());
            }
        }
        package_resolution = Some(resolved);
    }

    let store = open_store(ctx)?;
    let node_type = type_name
        .as_deref()
        .map(parse_node_type)
        .transpose()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut filters = shared.into_filters(node_type)?;
    filters.exact = exact;
    filters.count_only = count_only;
    filters.show_attributes = show_attributes;
    if let Some(raw) = annotation {
        let list = rgctl_graph::parse_annotation_list(&raw);
        if list.is_empty() {
            anyhow::bail!("--annotation requires at least one name (e.g. @MessageDriven)");
        }
        filters.annotation_names = Some(list);
    }
    if show_attributes {
        filters.annotation_arg_index = load_annotation_arg_index(&ctx.repo);
    }
    // Default limit for find when not counting
    if filters.limit.is_none() && !count_only {
        filters.limit = Some(50);
    }
    let q = StructuredQuery::new(store.as_ref());
    let result = q
        .find(pattern.as_deref(), &filters)
        .map_err(|e| map_sq_err(ctx, e))?;
    if ctx.format == OutputFormat::Json {
        let mut v = serde_json::to_value(&result)?;
        if let Some(obj) = v.as_object_mut() {
            if let Some(res) = package_resolution {
                obj.insert("package".into(), serde_json::to_value(&res)?);
                if result.total == 0 {
                    // Attempt bundled presence via deps manifests/jars is CLI-orchestrated;
                    // here we only flag that imports are empty so agents check deps.
                    obj.insert(
                        "bundled_presence".into(),
                        serde_json::json!({
                            "present": serde_json::Value::Null,
                            "honesty": "Zero import hits for package — run `deps check --include-jars` \
                                        before treating the library as absent"
                        }),
                    );
                }
            }
        }
        return ctx.emit_json_value(&v);
    }
    if count_only {
        ctx.stdout_line(&format!("{}", result.total))?;
    } else {
        emit_text_entities(ctx, result.entities.into_iter().map(|e| e.name))?;
    }
    Ok(())
}

/// `rgctl callers` / `callees`
pub fn run_call_neighbors(
    ctx: &CliContext,
    symbol: String,
    incoming: bool,
    depth: usize,
    shared: SharedQueryArgs,
) -> Result<()> {
    let methods = shared.methods.clone().unwrap_or_default();
    let package_coords = shared.package.clone();
    let mut shared = shared;
    let mut package_resolution = None;
    let mut symbols = vec![symbol];

    if let Some(ref coords) = package_coords {
        let resolved = rgctl_security::resolve_package(coords)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if shared.scope.is_none() {
            if let Some(prefix) = resolved.import_prefixes.first() {
                shared.scope = Some(prefix.clone());
            }
        }
        package_resolution = Some(resolved);
    }

    // When --methods is set, treat each method as an additional seed (or replace bare symbol).
    if !methods.is_empty() {
        symbols = methods.clone();
    }

    let store = open_store(ctx)?;
    let mut filters = shared.into_filters(None)?;
    if filters.limit.is_none() {
        filters.limit = Some(50);
    }
    let q = StructuredQuery::new(store.as_ref());
    let mut all_neighbors = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for sym in &symbols {
        let result = q
            .call_neighbors(sym, incoming, depth, &filters)
            .map_err(|e| map_sq_err(ctx, e))?;
        for n in result.neighbors {
            if seen.insert(n.name.clone()) {
                all_neighbors.push(n);
            }
        }
    }

    if ctx.format == OutputFormat::Json {
        let key = if incoming { "callers" } else { "callees" };
        let mut obj = serde_json::Map::new();
        obj.insert(
            "schema_version".into(),
            serde_json::json!(STRUCTURED_QUERY_SCHEMA_VERSION),
        );
        obj.insert(key.into(), serde_json::to_value(&all_neighbors)?);
        obj.insert("total".into(), serde_json::json!(all_neighbors.len()));
        if let Some(res) = package_resolution {
            obj.insert("package".into(), serde_json::to_value(&res)?);
        }
        if !methods.is_empty() {
            obj.insert("methods_filter".into(), serde_json::to_value(&methods)?);
        }
        return ctx.emit_json_value(&serde_json::Value::Object(obj));
    }
    emit_text_entities(ctx, all_neighbors.into_iter().map(|e| e.name))?;
    Ok(())
}

/// `rgctl relations`
pub fn run_relations(
    ctx: &CliContext,
    symbol: Option<String>,
    edge: String,
    direction: String,
    from_type: Option<String>,
    to_type: Option<String>,
    depth: usize,
    shared: SharedQueryArgs,
) -> Result<()> {
    let store = open_store(ctx)?;
    let edge_ty = parse_edge_type(&edge).map_err(|e| anyhow::anyhow!("{e}"))?;
    let dir = RelationDirection::parse(&direction).map_err(|e| anyhow::anyhow!("{e}"))?;
    let from_ty = from_type
        .as_deref()
        .map(parse_node_type)
        .transpose()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let to_ty = to_type
        .as_deref()
        .map(parse_node_type)
        .transpose()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut filters = shared.into_filters(None)?;
    if filters.limit.is_none() {
        filters.limit = Some(50);
    }
    if symbol.is_none() && from_ty.is_none() && to_ty.is_none() && filters.scope.is_none() {
        // Allow but warn via stderr — still run (seedless full scan).
        eprintln!("note: seedless relations without --from-type/--to-type/--scope may return a large edge set");
    }
    let q = StructuredQuery::new(store.as_ref());
    let result = q
        .relations(
            symbol.as_deref(),
            edge_ty,
            dir,
            from_ty,
            to_ty,
            depth,
            &filters,
        )
        .map_err(|e| map_sq_err(ctx, e))?;
    if ctx.format == OutputFormat::Json {
        return emit_json(ctx, &result);
    }
    for e in result.edges {
        let occ = if e.occurrences > 1 {
            format!(" x{}", e.occurrences)
        } else {
            String::new()
        };
        ctx.stdout_line(&format!(
            "{} -[{}]-> {}{occ}",
            e.source.name, e.edge, e.target.name
        ))?;
    }
    Ok(())
}

/// `rgctl inventory`
pub fn run_inventory(
    ctx: &CliContext,
    by: String,
    shared: SharedQueryArgs,
) -> Result<()> {
    let store = open_store(ctx)?;
    let dim = InventoryBy::parse(&by).map_err(|e| anyhow::anyhow!("{e}"))?;
    let filters = shared.into_filters(None)?;
    let q = StructuredQuery::new(store.as_ref());
    let result = q
        .inventory(dim, &filters)
        .map_err(|e| map_sq_err(ctx, e))?;
    if ctx.format == OutputFormat::Json {
        return emit_json(ctx, &result);
    }
    for c in result.counts {
        if let Some(occ) = c.occurrences {
            if occ != c.count {
                ctx.stdout_line(&format!("{}\t{}\toccurrences={}", c.key, c.count, occ))?;
            } else {
                ctx.stdout_line(&format!("{}\t{}", c.key, c.count))?;
            }
        } else {
            ctx.stdout_line(&format!("{}\t{}", c.key, c.count))?;
        }
    }
    Ok(())
}

/// `rgctl query <verb> …` alias — argv after `query` is re-parsed by clap parent; this helper
/// is unused when parent dispatches directly. Kept for documentation of serve offload.
#[allow(dead_code)]
pub fn serve_offload_contract() -> &'static str {
    StructuredQuery::serve_offload_note()
}
