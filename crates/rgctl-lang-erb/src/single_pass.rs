//! Single-pass AST extraction for ERB (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::ErbPlugin;
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../erb-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn erb_grammar() -> tree_sitter::Language {
    tree_sitter_embedded_template::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn erb_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&erb_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn erb_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&erb_grammar(), MANIFEST_JSON);
        dispatch
    })
}

fn push_children<'a>(stack: &mut Vec<(Node<'a>, usize)>, node: Node<'a>, depth: usize) {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();
    for child in children.into_iter().rev() {
        stack.push((child, depth + 1));
    }
}

enum Visit<'a> {
    Pre(Node<'a>, usize),
    LeaveScope,
}

/// Single iterative DFS: ERB blocks as symbols/relations + complexity metadata.
pub fn extract_all_single_pass(
    plugin: &ErbPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("erb", erb_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = erb_dispatch();

    let mut ctx = SinglePassCtx::new(dispatch.clone());
    let mut relations = Vec::new();
    let mut depth_warned = false;

    let mut stack = vec![Visit::Pre(root, 0)];

    while let Some(visit) = stack.pop() {
        match visit {
            Visit::LeaveScope => {
                if let Some(frame) = ctx.leave_scope() {
                    ctx.harvest_function_complexity(&frame);
                }
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
                plugin.collect_block_at(
                    node,
                    source,
                    &file_path_str,
                    &mut ctx.symbols,
                    &mut relations,
                );

                let mut pushed_scope = false;
                if matches!(kind, "output_directive" | "directive" | "comment_directive")
                    && ctx.symbols.len() > before
                {
                    let sym = &ctx.symbols[before];
                    let loc = ((node.end_position().row - node.start_position().row) + 1).max(1) as u32;
                    let name = sym.name.clone();
                    let qn = sym
                        .qualified_name
                        .clone()
                        .unwrap_or_else(|| name.clone());
                    ctx.enter_scope(
                        ScopeKind::Lambda,
                        Some(name),
                        qn,
                        node.start_byte(),
                        node.end_byte(),
                        true,
                    );
                    if let Some(f) = ctx.scopes.current_function_mut() {
                        f.complexity.cyclomatic = 1;
                        f.complexity.loc = loc;
                    }
                    pushed_scope = true;
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

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
