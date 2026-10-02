//! Structured query CLI: `find`, `callers`, `callees`, `relations`, `inventory`.

use super::context::CliContext;
use super::OutputFormat;
use anyhow::{Context, Result};
use rgctl_graph::{
    parse_edge_type, parse_node_type, InventoryBy, QueryFilters, RelationDirection, ScopeMode,
    StructuredQuery,
};

/// Shared filter flags for structured query verbs.
#[derive(Debug, Clone, Default)]
pub struct SharedQueryArgs {
    pub file: Option<String>,
    pub class: Option<String>,
    pub scope: Option<String>,
    pub scope_mode: Option<String>,
    pub exclude_scope: bool,
    pub lang: Option<String>,
    pub limit: Option<usize>,
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
            limit: self.limit,
            count_only: false,
            exact: false,
        })
    }
}

fn open_store(ctx: &CliContext) -> Result<std::sync::Arc<rgctl_graph::SnapshotNodeStore>> {
    ctx.open_snapshot_store()?
        .context("Graph snapshot not found (run `rgctl discover` first)")
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

/// `rgctl find`
pub fn run_find(
    ctx: &CliContext,
    pattern: Option<String>,
    type_name: Option<String>,
    shared: SharedQueryArgs,
    exact: bool,
    count_only: bool,
) -> Result<()> {
    let store = open_store(ctx)?;
    let node_type = type_name
        .as_deref()
        .map(parse_node_type)
        .transpose()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut filters = shared.into_filters(node_type)?;
    filters.exact = exact;
    filters.count_only = count_only;
    // Default limit for find when not counting
    if filters.limit.is_none() && !count_only {
        filters.limit = Some(50);
    }
    let q = StructuredQuery::new(store.as_ref());
    let result = q
        .find(pattern.as_deref(), &filters)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if ctx.format == OutputFormat::Json {
        return emit_json(ctx, &result);
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
    let store = open_store(ctx)?;
    let mut filters = shared.into_filters(None)?;
    if filters.limit.is_none() {
        filters.limit = Some(50);
    }
    let q = StructuredQuery::new(store.as_ref());
    let result = q
        .call_neighbors(&symbol, incoming, depth, &filters)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if ctx.format == OutputFormat::Json {
        // Shape as callers/callees field name for agents
        let mut v = serde_json::to_value(&result)?;
        if let Some(obj) = v.as_object_mut() {
            let key = if incoming { "callers" } else { "callees" };
            if let Some(n) = obj.remove("neighbors") {
                obj.insert(key.into(), n);
            }
        }
        return ctx.emit_json_value(&v);
    }
    emit_text_entities(ctx, result.neighbors.into_iter().map(|e| e.name))?;
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
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if ctx.format == OutputFormat::Json {
        return emit_json(ctx, &result);
    }
    for e in result.edges {
        ctx.stdout_line(&format!(
            "{} -[{}]-> {}",
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
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if ctx.format == OutputFormat::Json {
        return emit_json(ctx, &result);
    }
    for c in result.counts {
        ctx.stdout_line(&format!("{}\t{}", c.key, c.count))?;
    }
    Ok(())
}

/// `rgctl query <verb> …` alias — argv after `query` is re-parsed by clap parent; this helper
/// is unused when parent dispatches directly. Kept for documentation of serve offload.
#[allow(dead_code)]
pub fn serve_offload_contract() -> &'static str {
    StructuredQuery::serve_offload_note()
}
