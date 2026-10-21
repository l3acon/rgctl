//! Single-pass AST extraction for Go (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::GoPlugin;
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../go-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn go_grammar() -> tree_sitter::Language {
    tree_sitter_go::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn go_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&go_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn go_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&go_grammar(), MANIFEST_JSON);
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

enum Visit<'a> {
    Pre(Node<'a>, usize),
    /// Pop one scope frame pushed on the matching Pre.
    LeaveScope,
    /// Pop a control-flow GuardBlock and decrement cognitive nesting.
    LeaveGuard,
}

fn extract_type_declaration_symbols(
    plugin: &GoPlugin,
    node: Node,
    source: &[u8],
    file_path: &str,
    symbols: &mut Vec<Symbol>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "type_spec" {
            continue;
        }
        let mut handled = false;
        let mut spec_cursor = child.walk();
        for spec_child in child.children(&mut spec_cursor) {
            match spec_child.kind() {
                "struct_type" => {
                    let mut st = plugin.extract_struct(child, source, file_path)?;
                    if let Some(tp) = plugin.type_params_of(child, source) {
                        st.metadata["type_params"] = serde_json::Value::String(tp);
                    }
                    symbols.push(st);
                    handled = true;
                    break;
                }
                "interface_type" => {
                    let iface = plugin.extract_interface(child, source, file_path)?;
                    let iface_name = iface.name.clone();
                    symbols.push(iface);
                    let methods = plugin.extract_interface_methods(
                        spec_child,
                        &iface_name,
                        source,
                        file_path,
                        symbols,
                    )?;
                    symbols.extend(methods);
                    handled = true;
                    break;
                }
                _ => {}
            }
        }
        if !handled {
            if let Some(alias) = plugin.extract_type_alias(child, source, file_path)? {
                symbols.push(alias);
            }
        }
    }
    Ok(())
}

/// Single iterative DFS: symbols + deferred calls + complexity + implements/embeds.
pub fn extract_all_single_pass(
    plugin: &GoPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("go", go_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = go_dispatch();

    let mut ctx = SinglePassCtx::new(dispatch.clone());
    let mut relations = Vec::new();
    let mut pending_calls: Vec<PendingCall<'_>> = Vec::new();
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

                // --- Symbols + scope enter ---
                let mut pushed_scope = false;
                match kind {
                    "type_declaration" => {
                        extract_type_declaration_symbols(
                            plugin,
                            node,
                            source,
                            &file_path_str,
                            &mut ctx.symbols,
                        )?;
                    }
                    "import_declaration" => {
                        ctx.symbols
                            .extend(plugin.extract_imports(node, source, &file_path_str)?);
                    }
                    "const_declaration" => {
                        ctx.symbols
                            .extend(plugin.extract_consts(node, source, &file_path_str)?);
                    }
                    "function_declaration" | "method_declaration" => {
                        let mut func = plugin.extract_function(node, source, &file_path_str)?;
                        if let Some(tp) = plugin.type_params_of(node, source) {
                            func.metadata["type_params"] = serde_json::Value::String(tp);
                        }
                        let params = func.parameters.len() as u32;
                        let loc = ((node.end_position().row - node.start_position().row) + 1)
                            .max(1) as u32;
                        let name = func.name.clone();
                        let qn = func
                            .qualified_name
                            .clone()
                            .unwrap_or_else(|| name.clone());
                        ctx.symbols.push(func);
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
                    _ => {}
                }

                // --- Calls (deferred) ---
                if GO_CALL_KINDS.contains(&kind) {
                    if let Some(frame) = ctx.scopes.current_function() {
                        if let Some(idx) = frame.symbol_idx {
                            pending_calls.push(PendingCall {
                                node,
                                from_symbol_idx: idx,
                            });
                        }
                    }
                }

                // --- Complexity on control-flow / returns ---
                // Approximate existing calculate_cyclomatic / calculate_cognitive.
                if ctx.scopes.current_function().is_some() {
                    match kind {
                        "if_statement" | "for_statement" => {
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
                        "expression_switch_statement"
                        | "type_switch_statement"
                        | "select_statement" => {
                            // Cognitive: +1+nesting without increasing nesting (matches
                            // calculate_cognitive). Cyclomatic also +1 here; cases add more.
                            let delta = 1 + cognitive_nesting;
                            ctx.scopes.note_branch(delta);
                        }
                        "expression_case"
                        | "type_case"
                        | "default_case"
                        | "communication_case" => {
                            // Cyclomatic only (calculate_cognitive does not count cases).
                            if let Some(f) = ctx.scopes.current_function_mut() {
                                f.complexity.cyclomatic =
                                    f.complexity.cyclomatic.saturating_add(1);
                            }
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

    // Flush calls with the complete symbol list (forward-reference safe).
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
            "go",
            &mut relations,
        );
    }

    plugin.emit_implements_and_embeds(&ctx.symbols, file_path, &mut relations);

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
