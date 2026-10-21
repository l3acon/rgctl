//! Single-pass AST extraction for Ruby (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::{RubyPlugin, RUBY_COMPLEXITY_BRANCH_KINDS, RUBY_FUNCTION_KINDS};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../ruby-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn ruby_grammar() -> tree_sitter::Language {
    tree_sitter_ruby::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn ruby_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&ruby_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn ruby_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&ruby_grammar(), MANIFEST_JSON);
        dispatch
    })
}

fn push_children<'a>(stack: &mut Vec<(Node<'a>, usize)>, node: Node<'a>, depth: usize) {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();
    for child in children.into_iter().rev() {
        if child.is_named() {
            stack.push((child, depth + 1));
        }
    }
}

struct PendingCall<'a> {
    node: Node<'a>,
    from_symbol_idx: usize,
}

enum Visit<'a> {
    Pre(Node<'a>, usize),
    LeaveScope,
    LeaveGuard,
    /// Pop a name from the Ruby namespace path (`Shop::Order`).
    LeaveNameScope,
}

/// Single iterative DFS: symbols + deferred calls + mixin edges + complexity.
pub fn extract_all_single_pass(
    plugin: &RubyPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("ruby", ruby_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = ruby_dispatch();

    let mut ctx = SinglePassCtx::new(dispatch.clone());
    let mut relations = Vec::new();
    let mut pending_calls: Vec<PendingCall<'_>> = Vec::new();
    let mut name_scope: Vec<String> = Vec::new();
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
            Visit::LeaveNameScope => {
                name_scope.pop();
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
                let mut pushed_scope = false;
                let mut pushed_name = false;

                let owner_type_qn = ctx
                    .scopes
                    .current_type()
                    .map(|t| t.qualified_prefix.clone());

                match kind {
                    "class" => {
                        let (sym, type_qn) = plugin.extract_class_or_module(
                            node,
                            source,
                            &file_path_str,
                            &name_scope,
                            false,
                        )?;
                        let name = sym.name.clone();
                        ctx.symbols.push(sym);
                        name_scope.push(name.clone());
                        pushed_name = true;
                        ctx.enter_scope(
                            ScopeKind::Class,
                            Some(name),
                            type_qn,
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        pushed_scope = true;
                    }
                    "module" => {
                        let (sym, type_qn) = plugin.extract_class_or_module(
                            node,
                            source,
                            &file_path_str,
                            &name_scope,
                            true,
                        )?;
                        let name = sym.name.clone();
                        ctx.symbols.push(sym);
                        name_scope.push(name.clone());
                        pushed_name = true;
                        ctx.enter_scope(
                            ScopeKind::Module,
                            Some(name),
                            type_qn,
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        pushed_scope = true;
                    }
                    "singleton_class" => {
                        let outer = owner_type_qn
                            .clone()
                            .unwrap_or_else(|| RubyPlugin::join_qn(&name_scope, "Object"));
                        ctx.enter_scope(
                            ScopeKind::Impl,
                            None,
                            outer,
                            node.start_byte(),
                            node.end_byte(),
                            false,
                        );
                        pushed_scope = true;
                    }
                    "method" | "singleton_method" => {
                        let singleton = kind == "singleton_method";
                        let sym = plugin.extract_method(
                            node,
                            source,
                            &file_path_str,
                            &name_scope,
                            owner_type_qn.as_deref(),
                            singleton,
                        )?;
                        let params = sym.parameters.len() as u32;
                        let loc = ((node.end_position().row - node.start_position().row) + 1)
                            .max(1) as u32;
                        let name = sym.name.clone();
                        let qn = sym
                            .qualified_name
                            .clone()
                            .unwrap_or_else(|| name.clone());
                        ctx.symbols.push(sym);
                        ctx.enter_scope(
                            ScopeKind::Method,
                            Some(name),
                            qn,
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        if let Some(f) = ctx.scopes.current_function_mut() {
                            f.complexity.cyclomatic = 1;
                            f.complexity.param_count = params;
                            f.complexity.loc = loc;
                        }
                        pushed_scope = true;
                    }
                    "call" => {
                        if let Some(callee) = ruby_call_callee(node, source) {
                            if matches!(callee.as_str(), "require" | "require_relative") {
                                if let Some(path) = plugin.require_path_arg(node, source) {
                                    ctx.symbols.push(Symbol {
                                        name: path.clone(),
                                        symbol_type: SymbolType::Import,
                                        qualified_name: Some(path),
                                        location: RubyPlugin::loc(node, &file_path_str),
                                        signature: None,
                                        return_type: None,
                                        parameters: vec![],
                                        fields: vec![],
                                        modifiers: vec![],
                                        documentation: None,
                                        metadata: serde_json::json!({
                                            "language": "ruby",
                                            "kind": callee,
                                        }),
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }

                if RUBY_CALL_KINDS.contains(&kind) {
                    if let Some(frame) = ctx.scopes.current_function() {
                        if let Some(idx) = frame.symbol_idx {
                            pending_calls.push(PendingCall {
                                node,
                                from_symbol_idx: idx,
                            });
                        }
                    }
                }

                if ctx.scopes.current_function().is_some() {
                    if RUBY_COMPLEXITY_BRANCH_KINDS.contains(&kind) {
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
                    } else if kind == "return" {
                        ctx.scopes.note_return();
                    }
                }

                if pushed_name {
                    stack.push(Visit::LeaveNameScope);
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

    for pending in pending_calls {
        let Some(from_fn) = ctx.symbols.get(pending.from_symbol_idx) else {
            continue;
        };
        push_call_relation_from(
            pending.node,
            source,
            file_path,
            &ctx.symbols,
            from_fn,
            "ruby",
            &mut relations,
        );
    }

    plugin.extract_mixin_and_new_edges(
        root,
        source,
        &file_path_str,
        &ctx.symbols,
        &mut relations,
    )?;

    let _ = RUBY_FUNCTION_KINDS;
    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
