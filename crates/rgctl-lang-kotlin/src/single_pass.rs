//! Single-pass AST extraction for Kotlin (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::KotlinPlugin;
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../kotlin-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn kotlin_grammar() -> tree_sitter::Language {
    tree_sitter_kotlin_ng::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn kotlin_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&kotlin_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn kotlin_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&kotlin_grammar(), MANIFEST_JSON);
        dispatch
    })
}

fn push_children<'a>(stack: &mut Vec<(Node<'a>, usize)>, node: Node<'a>, depth: usize) {
    let mut cursor = node.walk();
    let children: Vec<Node<'a>> = node.children(&mut cursor).collect();
    for child in children.into_iter().rev() {
        stack.push((child, depth + 1));
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
}

/// Single iterative DFS: symbols + deferred calls + complexity; then inheritance.
pub fn extract_all_single_pass(
    plugin: &KotlinPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("kotlin", kotlin_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let package = KotlinPlugin::package_name(root, source);
    let dispatch = kotlin_dispatch();

    let mut ctx = SinglePassCtx::new(dispatch.clone());
    let mut relations = Vec::new();
    let mut pending_calls: Vec<PendingCall<'_>> = Vec::new();
    let mut cognitive_nesting: u32 = 0;

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
                    continue;
                }

                let _mask = dispatch.lookup(node.kind_id());
                let kind = node.kind();
                let mut pushed_scope = false;

                match kind {
                    "class_declaration" | "object_declaration" | "companion_object" => {
                        let before = ctx.symbols.len();
                        plugin.emit_type_and_ctors(
                            node,
                            source,
                            &file_path_str,
                            package.as_deref(),
                            &mut ctx.symbols,
                        );
                        if ctx.symbols.len() > before {
                            let type_sym = &ctx.symbols[before];
                            let name = type_sym.name.clone();
                            let qn = type_sym
                                .qualified_name
                                .clone()
                                .unwrap_or_else(|| name.clone());
                            let scope_kind = match type_sym.symbol_type {
                                SymbolType::Interface => ScopeKind::Interface,
                                SymbolType::Enum => ScopeKind::Enum,
                                _ => ScopeKind::Class,
                            };
                            let depth_s = ctx.scopes.depth();
                            let mut frame = ScopeFrame::new(
                                scope_kind,
                                Some(name),
                                qn,
                                node.start_byte(),
                                node.end_byte(),
                                depth_s,
                            );
                            frame.symbol_idx = Some(before);
                            ctx.scopes.push(frame);
                            pushed_scope = true;
                        }
                    }
                    "function_declaration" => {
                        if let Some(sym) = plugin.emit_function(
                            node,
                            source,
                            &file_path_str,
                            package.as_deref(),
                        ) {
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
                                ScopeKind::Function,
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
                    }
                    "primary_constructor" | "secondary_constructor" => {
                        // Ctors are emitted with the enclosing type; bind scope by span.
                        let start_line = node.start_position().row + 1;
                        if let Some(idx) = ctx.symbols.iter().position(|s| {
                            s.location.start_line == start_line
                                && s.metadata.get("is_constructor")
                                    == Some(&serde_json::json!(true))
                        }) {
                            let name = ctx.symbols[idx].name.clone();
                            let qn = ctx.symbols[idx]
                                .qualified_name
                                .clone()
                                .unwrap_or_else(|| name.clone());
                            let params = ctx.symbols[idx].parameters.len() as u32;
                            let loc = ((node.end_position().row - node.start_position().row) + 1)
                                .max(1) as u32;
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
                    "import" => {
                        plugin.emit_import(node, source, &file_path_str, &mut ctx.symbols);
                    }
                    _ => {}
                }

                if KOTLIN_CALL_KINDS.contains(&kind) {
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
                    match kind {
                        "if_expression"
                        | "when_expression"
                        | "while_statement"
                        | "for_statement"
                        | "do_while_statement"
                        | "catch_block" => {
                            let delta = 1 + cognitive_nesting;
                            ctx.scopes.note_branch(delta);
                            if let Some(f) = ctx.scopes.current_function_mut() {
                                f.complexity.max_nesting =
                                    f.complexity.max_nesting.max(cognitive_nesting + 1);
                            }
                            cognitive_nesting = cognitive_nesting.saturating_add(1);
                            let depth_s = ctx.scopes.depth();
                            ctx.scopes.push(ScopeFrame::new(
                                ScopeKind::GuardBlock,
                                None,
                                String::new(),
                                node.start_byte(),
                                node.end_byte(),
                                depth_s,
                            ));
                            stack.push(Visit::LeaveGuard);
                        }
                        "when_entry" => {
                            let delta = 1 + cognitive_nesting;
                            ctx.scopes.note_branch(delta);
                        }
                        "return_expression" | "jump_expression" => {
                            if kind == "return_expression"
                                || node
                                    .utf8_text(source)
                                    .ok()
                                    .is_some_and(|t| t.trim_start().starts_with("return"))
                            {
                                ctx.scopes.note_return();
                            }
                        }
                        _ => {}
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
            "kotlin",
            &mut relations,
        );
    }

    plugin.collect_inheritance_relations(root, source, file_path, &mut relations);

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
