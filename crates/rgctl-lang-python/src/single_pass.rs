//! Single-pass AST extraction for Python (`ScopeStack` + `KindDispatchTable`).

use crate::depth;
use crate::plugin::{instantiation_target, PythonPlugin};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use rgctl_plugin_helpers::{
    decorators_for_node, extract_python_class_extends_relations, extract_python_import_symbols,
    python_source_location,
};
use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../python-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn python_grammar() -> tree_sitter::Language {
    tree_sitter_python::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn python_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&python_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn python_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&python_grammar(), MANIFEST_JSON);
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

/// Pending call site: flush after the full symbol list exists (forward refs).
struct PendingCall<'a> {
    node: Node<'a>,
    from_symbol_idx: usize,
}

/// Pending subscript → `__getitem__` / `__setitem__` after symbols exist.
struct PendingSubscript<'a> {
    node: Node<'a>,
    from_symbol_idx: usize,
    is_store: bool,
}

enum Visit<'a> {
    Pre(Node<'a>, usize),
    /// Pop one scope frame pushed on the matching Pre.
    LeaveScope,
    /// Pop a control-flow GuardBlock and decrement cognitive nesting.
    LeaveGuard,
    /// Leave module-level typing / ImportError guard (binding harvest).
    LeaveTypingGuard,
}

fn decorator_from_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "class_definition" => node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok())
            .map(str::to_string),
        "function_definition" => {
            let method = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source).ok())
                .map(str::to_string)?;
            Some(
                rgctl_plugin_helpers::containing_class_name(node, source)
                    .map(|owner| format!("{owner}.{method}"))
                    .unwrap_or(method),
            )
        }
        _ => None,
    }
}

fn emit_decorators_for_node(
    node: Node,
    source: &[u8],
    file_path: &str,
    relations: &mut Vec<Relation>,
) {
    let Some(from) = decorator_from_name(node, source) else {
        return;
    };
    for (decorator_name, args) in decorators_for_node(node, source) {
        let mut meta = serde_json::json!({ "language": "python" });
        if let Some(args) = args {
            meta["arguments"] = serde_json::Value::String(args);
        }
        relations.push(Relation {
            from: from.clone(),
            to: decorator_name,
            relation_type: RelationType::AnnotatedWith,
            location: python_source_location(node, file_path),
            metadata: meta,
            to_qualified_hint: None,
            to_type_hint: None,
        });
    }
}

/// Single iterative DFS: symbols + depth relations + deferred calls + complexity.
pub fn extract_all_single_pass(
    plugin: &PythonPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("python", python_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = python_dispatch();

    let mut ctx = SinglePassCtx::new(dispatch.clone());
    let mut relations = Vec::new();
    let mut pending_calls: Vec<PendingCall<'_>> = Vec::new();
    let mut pending_subscripts: Vec<PendingSubscript<'_>> = Vec::new();
    let mut cognitive_nesting: u32 = 0;
    let mut typing_guard_depth: u32 = 0;
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
            Visit::LeaveTypingGuard => {
                typing_guard_depth = typing_guard_depth.saturating_sub(1);
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

                // --- Symbols + scope enter ---
                let mut pushed_scope = false;
                match kind {
                    "function_definition" => {
                        let sym = plugin.extract_function(node, source, &file_path_str)?;
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
                        emit_decorators_for_node(node, source, &file_path_str, &mut relations);
                        pushed_scope = true;
                    }
                    "class_definition" => {
                        let sym = plugin.extract_class(node, source, &file_path_str)?;
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
                        relations.extend(extract_python_class_extends_relations(
                            node,
                            source,
                            &file_path_str,
                            "python",
                        ));
                        emit_decorators_for_node(node, source, &file_path_str, &mut relations);
                        pushed_scope = true;
                    }
                    "import_statement" | "import_from_statement" => {
                        let mut imports = extract_python_import_symbols(
                            node,
                            source,
                            &file_path_str,
                            "python",
                        );
                        if typing_guard_depth > 0 {
                            for sym in &mut imports {
                                sym.metadata["guard"] = serde_json::json!("typing");
                            }
                        }
                        ctx.symbols.extend(imports);
                    }
                    "type_alias_statement" => {
                        if let Some(sym) =
                            depth::extract_type_alias_statement(node, source, &file_path_str)
                        {
                            ctx.symbols.push(sym);
                        }
                    }
                    "as_pattern_target" => {
                        if let Some(sym) =
                            depth::extract_as_pattern_target(node, source, &file_path_str)
                        {
                            ctx.symbols.push(sym);
                        }
                    }
                    "named_expression" => {
                        if let Some(sym) =
                            depth::extract_named_expression(node, source, &file_path_str)
                        {
                            ctx.symbols.push(sym);
                        }
                    }
                    "lambda" => {
                        if let Some(sym) =
                            depth::extract_lambda_symbol(node, source, &file_path_str)
                        {
                            ctx.symbols.push(sym);
                        }
                    }
                    "assignment"
                        if typing_guard_depth > 0 && ctx.scopes.current_function().is_none() =>
                    {
                        if let Some(mut sym) = depth::extract_simple_binding(
                            node,
                            source,
                            &file_path_str,
                            "guard_binding",
                        ) {
                            sym.metadata["guard"] = serde_json::json!("typing");
                            ctx.symbols.push(sym);
                        }
                    }
                    _ => {}
                }

                // Module-level typing / ImportError guards (harvest bindings inside).
                if ctx.scopes.current_function().is_none() {
                    let enter_typing = match kind {
                        "if_statement" => depth::is_typing_guard_if(node, source),
                        "try_statement" => depth::is_import_error_try(node, source),
                        _ => false,
                    };
                    if enter_typing {
                        typing_guard_depth = typing_guard_depth.saturating_add(1);
                        stack.push(Visit::LeaveTypingGuard);
                    }
                }

                // --- Calls (deferred) ---
                if PYTHON_CALL_KINDS.contains(&kind) {
                    if let Some(frame) = ctx.scopes.current_function() {
                        if let Some(idx) = frame.symbol_idx {
                            pending_calls.push(PendingCall {
                                node,
                                from_symbol_idx: idx,
                            });
                        }
                    }
                }

                // --- Subscript → __getitem__ / __setitem__ (deferred) ---
                if kind == "subscript" {
                    if let Some(frame) = ctx.scopes.current_function() {
                        if let Some(idx) = frame.symbol_idx {
                            pending_subscripts.push(PendingSubscript {
                                node,
                                from_symbol_idx: idx,
                                is_store: depth::subscript_is_store(node),
                            });
                        }
                    }
                }

                // --- Complexity on control-flow / returns ---
                // Matches existing calculate_cyclomatic / calculate_cognitive, plus
                // match/with as structural nesting and assert/boolean as flat branches.
                if ctx.scopes.current_function().is_some() {
                    match kind {
                        "if_statement"
                        | "while_statement"
                        | "for_statement"
                        | "match_statement"
                        | "with_statement" => {
                            let delta = 1 + cognitive_nesting;
                            ctx.scopes.note_branch(delta);
                            if let Some(f) = ctx.scopes.current_function_mut() {
                                f.complexity.max_nesting =
                                    f.complexity.max_nesting.max(cognitive_nesting + 1);
                            }
                            cognitive_nesting = cognitive_nesting.saturating_add(1);
                            let scope_depth = ctx.scopes.depth();
                            ctx.scopes.push(ScopeFrame::new(
                                ScopeKind::GuardBlock,
                                None,
                                String::new(),
                                node.start_byte(),
                                node.end_byte(),
                                scope_depth,
                            ));
                            stack.push(Visit::LeaveGuard);
                        }
                        "elif_clause" | "except_clause" | "assert_statement"
                        | "boolean_operator" => {
                            let delta = 1 + cognitive_nesting;
                            ctx.scopes.note_branch(delta);
                        }
                        "return_statement" => {
                            ctx.scopes.note_return();
                        }
                        _ => {}
                    }
                }

                // Children + leave marker (post-order leave after children).
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

    // Flush calls + instantiations with the complete symbol list (forward-ref safe).
    let class_names: HashSet<String> = ctx
        .symbols
        .iter()
        .filter(|s| s.symbol_type == SymbolType::Class)
        .map(|s| s.name.clone())
        .collect();

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
            "python",
            &mut relations,
        );
        if let Some(target) = instantiation_target(pending.node, source, &class_names) {
            let from = from_fn
                .qualified_name
                .clone()
                .unwrap_or_else(|| from_fn.name.clone());
            relations.push(Relation {
                from,
                to: target.clone(),
                relation_type: RelationType::Instantiates,
                location: python_source_location(pending.node, &file_path_str),
                metadata: serde_json::json!({ "language": "python" }),
                to_qualified_hint: Some(target),
                to_type_hint: None,
            });
        }
    }

    for pending in pending_subscripts {
        let Some(from_fn) = ctx.symbols.get(pending.from_symbol_idx) else {
            continue;
        };
        if let Some(rel) = depth::maybe_subscript_call_relation(
            pending.node,
            source,
            &file_path_str,
            from_fn,
            pending.is_store,
        ) {
            relations.push(rel);
        }
    }

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
