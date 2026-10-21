//! Single-pass AST extraction for JavaScript (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::{source_location, JavaScriptPlugin};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use rgctl_plugin_helpers::{
    extract_cjs_require_symbols, extract_class_extends_relations, extract_import_symbols,
    is_ecmascript_function_node, is_function_expression_kind, simple_type_name, type_name_from_node,
};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../javascript-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn js_grammar() -> tree_sitter::Language {
    tree_sitter_javascript::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn javascript_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&js_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn js_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&js_grammar(), MANIFEST_JSON);
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

fn function_scope_kind(node: Node) -> ScopeKind {
    match node.kind() {
        "method_definition" => ScopeKind::Method,
        k if is_function_expression_kind(k)
            || (k == "function" && node.is_named() && node.child_by_field_name("name").is_none()) =>
        {
            ScopeKind::Lambda
        }
        _ => ScopeKind::Function,
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

/// Single iterative DFS: symbols + deferred calls + heritage/new + complexity.
pub fn extract_all_single_pass(
    plugin: &JavaScriptPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("javascript", js_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = js_dispatch();

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

                if is_ecmascript_function_node(node) {
                    let sym = plugin.extract_function(node, source, &file_path_str)?;
                    let params = sym.parameters.len() as u32;
                    let loc = ((node.end_position().row - node.start_position().row) + 1).max(1) as u32;
                    let name = sym.name.clone();
                    let qn = sym
                        .qualified_name
                        .clone()
                        .unwrap_or_else(|| name.clone());
                    let sk = function_scope_kind(node);
                    ctx.symbols.push(sym);
                    ctx.enter_scope(
                        sk,
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
                    plugin.emit_decorators_for_node(
                        node,
                        source,
                        file_path,
                        &mut relations,
                    );
                }

                match kind {
                    "class_declaration" => {
                        let sym = plugin.extract_class(node, source, &file_path_str)?;
                        let name = sym.name.clone();
                        ctx.symbols.push(sym);
                        ctx.enter_scope(
                            ScopeKind::Class,
                            Some(name.clone()),
                            name.clone(),
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        pushed_scope = true;
                        extract_class_extends_relations(
                            node,
                            source,
                            file_path,
                            &name,
                            "javascript",
                            &mut relations,
                        );
                        plugin.emit_decorators_for_node(
                            node,
                            source,
                            file_path,
                            &mut relations,
                        );
                    }
                    "import_statement" => {
                        ctx.symbols.extend(extract_import_symbols(
                            node,
                            source,
                            &file_path_str,
                            "javascript",
                        ));
                    }
                    "export_statement" => {
                        if let Some(sym) = plugin.extract_export_symbol(node, source, &file_path_str)
                        {
                            ctx.symbols.push(sym);
                        }
                    }
                    "export_specifier" => {
                        plugin.collect_export_specifier_relations(
                            node,
                            source,
                            &file_path_str,
                            &mut relations,
                        );
                    }
                    "new_expression" => {
                        if let Some(from_fn) =
                            containing_function_from_scope(&ctx.scopes, &ctx.symbols)
                        {
                            let from = from_fn
                                .qualified_name
                                .clone()
                                .unwrap_or_else(|| from_fn.name.clone());
                            if let Some(ctor) = node.child_by_field_name("constructor") {
                                if let Some(target) = type_name_from_node(ctor, source) {
                                    relations.push(Relation {
                                        from,
                                        to: simple_type_name(&target),
                                        relation_type: RelationType::Instantiates,
                                        location: source_location(node, &file_path_str),
                                        metadata: serde_json::json!({ "language": "javascript" }),
                                        to_qualified_hint: Some(target),
                                        to_type_hint: None,
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }

                if JS_CALL_KINDS.contains(&kind) {
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
                        | "switch_statement"
                        | "while_statement"
                        | "for_statement"
                        | "catch_clause"
                        | "ternary_expression" => {
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
                        "case" => {
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

    // Flush calls (forward-ref safe). Prefer shared helper; enrich JS unresolved.
    for pending in pending_calls {
        let Some(from_fn) = ctx.symbols.get(pending.from_symbol_idx) else {
            continue;
        };
        let before = relations.len();
        plugin.emit_js_call(
            pending.node,
            source,
            file_path,
            &ctx.symbols,
            from_fn,
            &mut relations,
        );
        // Fallback if custom emitter skipped — shared path for ordinary calls.
        if relations.len() == before {
            push_call_relation_from(
                pending.node,
                source,
                file_path,
                &ctx.symbols,
                from_fn,
                "javascript",
                &mut relations,
            );
        }
    }

    ctx.symbols.extend(extract_cjs_require_symbols(
        root,
        source,
        &file_path_str,
        "javascript",
    ));

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
