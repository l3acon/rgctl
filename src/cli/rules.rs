//! `rgctl rules run` — post-index Kantra evaluation against the session snapshot.

use super::context::CliContext;
use super::kantra_discover::{
    preload_discovered_sources, resolve_kantra_catalog, run_kantra_index, run_kantra_violates,
};
use super::stage_profile::DiscoverStageReport;
use super::OutputFormat;
use anyhow::{Context, Result, bail};
use rgctl_graph::schema::{EdgeType, NodeType};
use rgctl_graph::snapshot::SNAPSHOT_FILE;
use rgctl_kantra::{
    EvalContext, EvalEdge, EvalGraph, EvalNode, KantraEngine, KantraFileCache, ViolationResolver,
    cache_dir, ruleset_hash,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// `rgctl rules run <DIR>` (and related flags).
pub fn run_rules(
    ctx: &CliContext,
    rules_dir: PathBuf,
    target: Option<String>,
    index_only: bool,
    catalog: Option<PathBuf>,
) -> Result<()> {
    if !rules_dir.is_dir() && catalog.is_none() {
        bail!("rules path is not a directory: {}", rules_dir.display());
    }
    let store = &ctx.repo;
    let snapshot = rgctl_graph::paths::artifact_path(store, SNAPSHOT_FILE);
    if !snapshot.exists() {
        bail!("Graph snapshot not found (run `rgctl discover` first)");
    }

    let rules_opt = if catalog.is_some() {
        None
    } else {
        Some(rules_dir.as_path())
    };
    let catalog_opt = catalog.as_deref();

    let mut profile = DiscoverStageReport::default();
    run_kantra_index(store, rules_opt, catalog_opt, &mut profile)?;

    if index_only {
        if ctx.format == OutputFormat::Json {
            let v = serde_json::json!({
                "schema_version": 1,
                "command": "rules",
                "action": "index-only",
                "kantra_index_secs": profile.kantra_index.secs,
            });
            ctx.emit_json_value(&v)?;
        } else {
            ctx.stdout_line("rules: indexed KantraRule nodes (eval skipped)")?;
        }
        return Ok(());
    }

    let catalog = resolve_kantra_catalog(rules_opt, catalog_opt)?;
    let (engine, _) = KantraEngine::from_catalog(catalog.clone(), target.as_deref())
        .map_err(|e| anyhow::anyhow!("kantra catalog: {e}"))?;

    let files = collect_source_files(&ctx.repo)?;
    let sources = preload_discovered_sources(&ctx.repo, &files, None);
    let graph = build_eval_graph_from_snapshot(&snapshot)?;
    let rs_hash = ruleset_hash(
        engine.catalog_id().unwrap_or("unknown"),
        catalog.rules.len(),
    );
    let mut file_cache = KantraFileCache::load(&cache_dir(store), &rs_hash);
    let mut eval_ctx = EvalContext {
        repo_root: &ctx.repo,
        files: &files,
        sources: &sources,
        graph: &graph,
        cache: Some(&mut file_cache),
        cached_files: HashSet::new(),
    };
    let (mut findings, _) = engine
        .evaluate(&mut eval_ctx)
        .map_err(|e| anyhow::anyhow!("kantra evaluate: {e}"))?;
    let resolver = ViolationResolver::from_eval_nodes(&graph.nodes);
    resolver.attach_node_ids(&mut findings.violations);
    findings.command = "rules_run".into();
    file_cache.save(&cache_dir(store))?;

    let out_path = rgctl_graph::paths::artifact_path(store, "kantra_findings.json");
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&findings).context("serialize kantra findings")?;
    fs::write(&out_path, &json).with_context(|| format!("write {}", out_path.display()))?;

    // Best-effort VIOLATES edges (snapshot rewrite).
    let _ = run_kantra_violates(store, &mut profile);

    if ctx.format == OutputFormat::Json {
        let v = serde_json::to_value(&findings)?;
        ctx.emit_json_value(&v)?;
    } else {
        ctx.stdout_line(&format!(
            "rules: {} violations, {} skipped → {}",
            findings.violations.len(),
            findings.skipped_rules.len(),
            out_path.display()
        ))?;
    }
    Ok(())
}

fn collect_source_files(repo: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in ignore::WalkBuilder::new(repo).git_ignore(true).build().flatten() {
        let path = entry.path();
        if path.is_file() {
            out.push(path.to_path_buf());
        }
    }
    Ok(out)
}

fn build_eval_graph_from_snapshot(snapshot: &Path) -> Result<EvalGraph> {
    let store = rgctl_graph::SnapshotNodeStore::open(snapshot)?;
    let mut graph = EvalGraph::default();
    for id in store.all_node_ids() {
        let Some(node) = store.get_node(id)? else {
            continue;
        };
        if !matches!(
            node.node_type,
            NodeType::Import
                | NodeType::Class
                | NodeType::Interface
                | NodeType::Enum
                | NodeType::Annotation
                | NodeType::Function
                | NodeType::Module
                | NodeType::File
        ) {
            continue;
        }
        graph.nodes.push(EvalNode {
            id: Some(node.id),
            node_type: format!("{:?}", node.node_type),
            name: node.name.to_string(),
            qualified_name: node.qualified_name.as_ref().map(|s| s.to_string()),
            file_path: node.file_path.as_ref().map(|s| s.to_string()),
            start_line: node.start_line,
            labels: node.labels.clone(),
        });
    }
    store.for_each_edge(|from, to, et| {
        let edge_name = match et {
            EdgeType::Extends => "EXTENDS",
            EdgeType::Implements => "IMPLEMENTS",
            EdgeType::AnnotatedWith => "ANNOTATED_WITH",
            _ => return Ok(()),
        };
        let Some(from_node) = store.get_node(from)? else {
            return Ok(());
        };
        let Some(to_node) = store.get_node(to)? else {
            return Ok(());
        };
        graph.edges.push(EvalEdge {
            edge_type: edge_name.to_string(),
            from_name: from_node.name.to_string(),
            from_qualified: from_node
                .qualified_name
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| from_node.name.to_string()),
            to_name: to_node.name.to_string(),
            to_qualified: to_node
                .qualified_name
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| to_node.name.to_string()),
            file_path: from_node
                .file_path
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            line: from_node.start_line.unwrap_or(1),
        });
        Ok(())
    })?;
    Ok(graph)
}
