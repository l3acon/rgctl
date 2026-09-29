//! `rgctl inspect` — raw CFG / PDG / dominance debugging.

use super::args::{InspectLayer, OutputFormat, PdgEdgeLayer};
use super::context::{CliContext, language_from_path};
use super::inspect_output::{inspect_cfg_json, inspect_dom_json, inspect_pdg_json};
use super::markup::markup_context_unsupported;
use crate::analysis::{DominatorTree, ProgramDependenceGraph, build_cfg_for_function};
use anyhow::Result;
use rgctl_graph::backend::GraphBackend;
use std::path::Path;

pub struct InspectArgs {
    pub symbol: String,
    pub layer: InspectLayer,
}

pub fn run(ctx: &CliContext, args: InspectArgs) -> Result<()> {
    let (node, source) = resolve_symbol_function(ctx, &args.symbol)?;
    let file = node.file_path.as_deref().unwrap_or(".");
    if let Some(msg) = markup_context_unsupported("inspect", Path::new(file)) {
        anyhow::bail!(msg);
    }
    let lang = language_from_path(Path::new(file));
    let display_name = node.name.as_str();
    let mut cfg = build_cfg_for_function(&lang, &source, display_name)?;
    let pdg = ProgramDependenceGraph::build(&cfg, source.as_bytes())?;
    let dom = DominatorTree::build(&cfg);

    match args.layer {
        InspectLayer::Cfg { prune } => {
            if prune {
                cfg.prune_unreachable_blocks();
            }
            match ctx.format {
                OutputFormat::Json => {
                    let response = inspect_cfg_json(display_name, &cfg, prune);
                    ctx.emit_json_value(&serde_json::to_value(&response)?)?;
                }
                OutputFormat::Mermaid => {
                    ctx.emit(&cfg_to_mermaid(&cfg))?;
                }
                OutputFormat::Graphviz => {
                    ctx.emit(&cfg_to_dot(&cfg))?;
                }
                OutputFormat::Text => {
                    println!(
                        "CFG for {}: {} blocks, {} edges",
                        display_name,
                        cfg.blocks.len(),
                        cfg.edges.len()
                    );
                }
            }
        }
        InspectLayer::Pdg {
            edge_layer,
            def_use,
        } => {
            let (data, control) = match edge_layer {
                PdgEdgeLayer::All => (pdg.data_deps.len(), pdg.control_deps.len()),
                PdgEdgeLayer::Data => (pdg.data_deps.len(), 0),
                PdgEdgeLayer::Control => (0, pdg.control_deps.len()),
            };
            if ctx.format == OutputFormat::Json {
                let response = inspect_pdg_json(display_name, &pdg, def_use, data, control);
                ctx.emit_json_value(&serde_json::to_value(&response)?)?;
            } else {
                println!(
                    "PDG for {}: {} nodes, {} data deps, {} control deps",
                    display_name,
                    pdg.nodes.len(),
                    data,
                    control
                );
            }
        }
        InspectLayer::Dom { frontiers } => {
            if ctx.format == OutputFormat::Json {
                let response = inspect_dom_json(display_name, &cfg, &dom, frontiers);
                ctx.emit_json_value(&serde_json::to_value(&response)?)?;
            } else if ctx.format == OutputFormat::Mermaid {
                ctx.emit(&dom_to_mermaid(&dom))?;
            } else {
                println!("Dominators for {}: {} blocks", display_name, dom.idom.len());
                if frontiers {
                    for (block, frontier) in &dom.frontiers {
                        if !frontier.is_empty() {
                            println!("  DF({block:?}): {frontier:?}");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn cfg_to_dot(cfg: &crate::analysis::ControlFlowGraph) -> String {
    let mut out = String::from("digraph cfg {\n");
    for edge in &cfg.edges {
        out.push_str(&format!("  {:?} -> {:?};\n", edge.from, edge.to));
    }
    out.push_str("}\n");
    out
}

fn cfg_to_mermaid(cfg: &crate::analysis::ControlFlowGraph) -> String {
    let mut out = String::from("flowchart TD\n");
    for edge in &cfg.edges {
        out.push_str(&format!("  {:?} --> {:?}\n", edge.from, edge.to));
    }
    out
}

fn dom_to_mermaid(dom: &DominatorTree) -> String {
    let mut out = String::from("flowchart TD\n");
    for (child, parent) in &dom.idom {
        out.push_str(&format!("  {:?} --> {:?}\n", parent, child));
    }
    out
}

fn resolve_symbol_function(
    ctx: &CliContext,
    symbol: &str,
) -> Result<(rgctl_graph::schema::Node, String)> {
    use rgctl_analysis::{candidates_from_backend, parse_fqn_symbol, resolve_symbol_uuid};
    use rgctl_graph::schema::NodeType;
    use std::fs;

    let parsed = parse_fqn_symbol(symbol, None, None);
    let graph = ctx.load_graph()?;
    let backend = graph.backend();

    // Prefer shared FQN resolution (path::symbol / Class::method) used by blast-radius.
    let candidates = candidates_from_backend(backend, &parsed.target_name)?;
    if !candidates.is_empty() {
        match resolve_symbol_uuid(&candidates, &parsed) {
            Ok(id) => {
                let node = backend
                    .get_node(id)?
                    .ok_or_else(|| anyhow::anyhow!("function symbol not found: {symbol}"))?;
                let file = node
                    .file_path
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("function has no file path"))?;
                let source = fs::read_to_string(&file)?;
                return Ok((node, source));
            }
            Err(rgctl_error::Error::AmbiguousSymbol { name, count }) => {
                anyhow::bail!(
                    "Symbol '{name}' is ambiguous. Found {count} matches. \
                     Refine with path syntax: rgctl inspect \"path/to/file.ts::{name}\" cfg"
                );
            }
            Err(rgctl_error::Error::NotFound(_)) => {}
            Err(e) => return Err(e.into()),
        }
    }

    // Legacy fallback: bare name or suffix match when FQN filters yield nothing.
    let matches = backend.find_nodes_by_name(&parsed.target_name)?;
    let node = matches
        .into_iter()
        .find(|n| n.node_type == NodeType::Function)
        .or_else(|| {
            backend
                .all_nodes()
                .ok()?
                .into_iter()
                .find(|n| {
                    n.node_type == NodeType::Function
                        && (n.name == parsed.target_name
                            || n.name.ends_with(&parsed.target_name))
                })
        })
        .ok_or_else(|| anyhow::anyhow!("function symbol not found: {symbol}"))?;
    let file = node
        .file_path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("function has no file path"))?;
    let source = fs::read_to_string(&file)?;
    Ok((node, source))
}
