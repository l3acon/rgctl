//! Single-pass AST extraction for C# (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::{CSharpPlugin, ExtractCtx};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../csharp-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn csharp_grammar() -> tree_sitter::Language {
    tree_sitter_c_sharp::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn csharp_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&csharp_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn csharp_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&csharp_grammar(), MANIFEST_JSON);
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
    LeaveNamespace,
}

/// Single iterative DFS: symbols + deferred calls + complexity; then non-call relations.
pub fn extract_all_single_pass(
    plugin: &CSharpPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("csharp", csharp_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = csharp_dispatch();

    let mut extract_ctx = ExtractCtx::default();
    plugin.seed_ctx_from_tree(root, source, &mut extract_ctx);

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
            Visit::LeaveNamespace => {
                extract_ctx.pop_namespace();
            }
            Visit::Pre(node, depth) => {
                if depth > AST_MAX_DEPTH {
                    continue;
                }

                let _mask = dispatch.lookup(node.kind_id());
                let kind = node.kind();

                if kind == "namespace_declaration" {
                    if let Some(ns) = crate::plugin::namespace_name(node, source) {
                        extract_ctx.push_qualified_namespace(&ns);
                        stack.push(Visit::LeaveNamespace);
                        if let Some(body) = node.child_by_field_name("body") {
                            let mut child_stack = Vec::new();
                            push_children(&mut child_stack, body, depth);
                            for (child, child_depth) in child_stack {
                                stack.push(Visit::Pre(child, child_depth));
                            }
                        }
                    }
                    continue;
                }
                if kind == "file_scoped_namespace_declaration" {
                    // Seeded into extract_ctx; do not re-enter.
                    continue;
                }

                let mut pushed_scope = false;

                match kind {
                    "method_declaration" | "local_function_statement" => {
                        let sym =
                            plugin.extract_method(node, source, &file_path_str, &extract_ctx)?;
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
                    "constructor_declaration" => {
                        let sym = plugin.extract_constructor(
                            node,
                            source,
                            &file_path_str,
                            &extract_ctx,
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
                    "delegate_declaration" => {
                        ctx.symbols.push(plugin.extract_delegate(
                            node,
                            source,
                            &file_path_str,
                            &extract_ctx,
                        )?);
                    }
                    "class_declaration" | "struct_declaration" => {
                        let sym = plugin.extract_type(
                            node,
                            source,
                            &file_path_str,
                            SymbolType::Class,
                            &extract_ctx,
                            serde_json::json!({}),
                        )?;
                        let name = sym.name.clone();
                        let qn = sym
                            .qualified_name
                            .clone()
                            .unwrap_or_else(|| name.clone());
                        ctx.symbols.push(sym);
                        ctx.enter_scope(
                            ScopeKind::Class,
                            Some(name),
                            qn,
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        pushed_scope = true;
                    }
                    "record_declaration" => {
                        let sym = plugin.extract_type(
                            node,
                            source,
                            &file_path_str,
                            SymbolType::Class,
                            &extract_ctx,
                            serde_json::json!({ "is_record": true }),
                        )?;
                        let name = sym.name.clone();
                        let qn = sym
                            .qualified_name
                            .clone()
                            .unwrap_or_else(|| name.clone());
                        ctx.symbols.push(sym);
                        ctx.enter_scope(
                            ScopeKind::Class,
                            Some(name),
                            qn,
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        pushed_scope = true;
                    }
                    "interface_declaration" => {
                        let sym = plugin.extract_type(
                            node,
                            source,
                            &file_path_str,
                            SymbolType::Interface,
                            &extract_ctx,
                            serde_json::json!({}),
                        )?;
                        let name = sym.name.clone();
                        let qn = sym
                            .qualified_name
                            .clone()
                            .unwrap_or_else(|| name.clone());
                        ctx.symbols.push(sym);
                        ctx.enter_scope(
                            ScopeKind::Interface,
                            Some(name),
                            qn,
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        pushed_scope = true;
                    }
                    "enum_declaration" => {
                        let sym = plugin.extract_type(
                            node,
                            source,
                            &file_path_str,
                            SymbolType::Enum,
                            &extract_ctx,
                            serde_json::json!({}),
                        )?;
                        let name = sym.name.clone();
                        let qn = sym
                            .qualified_name
                            .clone()
                            .unwrap_or_else(|| name.clone());
                        ctx.symbols.push(sym);
                        ctx.enter_scope(
                            ScopeKind::Enum,
                            Some(name),
                            qn,
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        pushed_scope = true;
                    }
                    "using_directive" => {
                        let text = node.utf8_text(source)?.trim().to_string();
                        extract_ctx.register_using(&text);
                        ctx.symbols.push(Symbol {
                            name: text.clone(),
                            symbol_type: SymbolType::Import,
                            qualified_name: None,
                            location: crate::plugin::source_location(node, &file_path_str),
                            signature: None,
                            return_type: None,
                            parameters: vec![],
                            fields: vec![],
                            modifiers: vec![],
                            documentation: None,
                            metadata: serde_json::json!({ "language": "csharp" }),
                        });
                    }
                    _ => {}
                }

                if CSHARP_CALL_KINDS.contains(&kind) {
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
                        "if_statement"
                        | "while_statement"
                        | "for_statement"
                        | "foreach_statement"
                        | "catch_clause"
                        | "conditional_expression"
                        | "switch_expression"
                        | "switch_statement" => {
                            let delta = 1 + cognitive_nesting;
                            ctx.scopes.note_branch(delta);
                            if let Some(f) = ctx.scopes.current_function_mut() {
                                f.complexity.max_nesting =
                                    f.complexity.max_nesting.max(cognitive_nesting + 1);
                            }
                            if matches!(
                                kind,
                                "if_statement"
                                    | "while_statement"
                                    | "for_statement"
                                    | "foreach_statement"
                                    | "switch_statement"
                                    | "switch_expression"
                            ) {
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
                        }
                        "switch_section" => {
                            let delta = 1 + cognitive_nesting;
                            ctx.scopes.note_branch(delta);
                        }
                        "return_statement" => {
                            ctx.scopes.note_return();
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
            "csharp",
            &mut relations,
        );
    }

    plugin.collect_non_call_relations(
        root,
        source,
        file_path,
        &ctx.symbols,
        &extract_ctx,
        &mut relations,
    )?;

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
