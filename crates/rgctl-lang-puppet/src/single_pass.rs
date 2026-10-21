//! Single-pass AST extraction for Puppet (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::{PuppetPlugin, PUPPET_BRANCH_KINDS};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../puppet-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn puppet_grammar() -> tree_sitter::Language {
    tree_sitter_puppet::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn puppet_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&puppet_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn puppet_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&puppet_grammar(), MANIFEST_JSON);
        dispatch
    })
}

fn push_children<'a>(stack: &mut Vec<(Node<'a>, usize)>, node: Node<'a>, depth: usize) {
    let mut c = node.walk();
    let children: Vec<Node> = node.children(&mut c).collect();
    for child in children.into_iter().rev() {
        stack.push((child, depth + 1));
    }
}

enum Visit<'a> {
    Pre(Node<'a>, usize),
    LeaveScope,
    LeaveGuard,
}

/// Single iterative DFS: symbols + relations + complexity on CFG hosts.
pub fn extract_all_single_pass(
    plugin: &PuppetPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("puppet", puppet_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = puppet_dispatch();

    let mut ctx = SinglePassCtx::new(dispatch.clone());
    let mut relations = Vec::new();
    let mut cognitive_nesting: u32 = 0;
    let mut depth_warned = false;

    let mut stack = vec![Visit::Pre(root, 0)];

    while let Some(visit) = stack.pop() {
        match visit {
            Visit::LeaveScope => {
                if let Some(frame) = ctx.leave_scope() {
                    ctx.harvest_function_complexity(&frame);
                }
            }
            Visit::LeaveGuard => {
                cognitive_nesting = cognitive_nesting.saturating_sub(1);
                let _ = ctx.leave_scope();
            }
            Visit::Pre(node, depth) => {
                if depth > AST_MAX_DEPTH {
                    if !depth_warned {
                        tracing::warn!(
                            file = %file_path.display(),
                            depth = AST_MAX_DEPTH,
                            "AST depth limit exceeded during single-pass extract; skipping deep branches"
                        );
                        depth_warned = true;
                    }
                    continue;
                }

                let _mask = dispatch.lookup(node.kind_id());
                let kind = node.kind();
                let before = ctx.symbols.len();
                plugin.collect_symbol_at(node, source, &file_path_str, &mut ctx.symbols);

                let mut pushed_scope = false;
                // Bind function scope to CFG host / function / lambda just pushed.
                if matches!(
                    kind,
                    "class_definition"
                        | "defined_resource_type"
                        | "node_definition"
                        | "function_declaration"
                        | "lambda"
                ) {
                    // CFG host is the last Function symbol pushed for this node (if any).
                    if let Some((idx, sym)) = ctx.symbols[before..]
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, s)| s.symbol_type == SymbolType::Function)
                        .map(|(i, s)| (before + i, s))
                    {
                        let params = sym.parameters.len() as u32;
                        let loc = ((node.end_position().row - node.start_position().row) + 1)
                            .max(1) as u32;
                        let name = sym.name.clone();
                        let qn = sym
                            .qualified_name
                            .clone()
                            .unwrap_or_else(|| name.clone());
                        let depth_s = ctx.scopes.depth();
                        let mut frame = ScopeFrame::new(
                            ScopeKind::Function,
                            Some(name),
                            qn,
                            node.start_byte(),
                            node.end_byte(),
                            depth_s,
                        );
                        frame.symbol_idx = Some(idx);
                        frame.complexity.cyclomatic = 1;
                        frame.complexity.param_count = params;
                        frame.complexity.loc = loc;
                        ctx.scopes.push(frame);
                        pushed_scope = true;
                    }
                }

                plugin.collect_relation_at(
                    node,
                    source,
                    &file_path_str,
                    &mut relations,
                );

                if ctx.scopes.current_function().is_some() {
                    if PUPPET_BRANCH_KINDS.contains(&kind) {
                        let delta = 1 + cognitive_nesting;
                        ctx.scopes.note_branch(delta);
                        if let Some(f) = ctx.scopes.current_function_mut() {
                            f.complexity.max_nesting =
                                f.complexity.max_nesting.max(cognitive_nesting + 1);
                        }
                        cognitive_nesting = cognitive_nesting.saturating_add(1);
                        let d = ctx.scopes.depth();
                        ctx.scopes.push(ScopeFrame::new(
                            ScopeKind::GuardBlock,
                            None,
                            String::new(),
                            node.start_byte(),
                            node.end_byte(),
                            d,
                        ));
                        stack.push(Visit::LeaveGuard);
                    }
                }

                if pushed_scope {
                    stack.push(Visit::LeaveScope);
                }
                let mut child_stack = Vec::new();
                push_children(&mut child_stack, node, depth);
                for (child, child_depth) in child_stack {
                    stack.push(Visit::Pre(child, child_depth));
                }
            }
        }
    }

    plugin.maybe_module_from_metadata(file_path, &mut ctx.symbols, &mut relations);
    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
