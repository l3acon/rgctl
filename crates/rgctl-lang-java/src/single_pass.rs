//! Single-pass AST extraction for Java (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::{ExtractCtx, JavaPlugin, BRANCH_KINDS, JAVA_CALL_KINDS};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../java-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn java_grammar() -> tree_sitter::Language {
    tree_sitter_java::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn java_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&java_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn java_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&java_grammar(), MANIFEST_JSON);
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

/// Single iterative DFS: symbols + deferred calls + complexity; then non-call relations.
pub fn extract_all_single_pass(
    plugin: &JavaPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("java", java_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let extract_ctx = ExtractCtx::new(root, source);
    let dispatch = java_dispatch();

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
                    "method_declaration" => {
                        let sym =
                            plugin.extract_method(node, source, &file_path_str, &extract_ctx)?;
                        enter_function_scope(&mut ctx, node, sym, ScopeKind::Method);
                        pushed_scope = true;
                    }
                    "constructor_declaration" => {
                        let sym = plugin.extract_constructor(
                            node,
                            source,
                            &file_path_str,
                            &extract_ctx,
                        )?;
                        enter_function_scope(&mut ctx, node, sym, ScopeKind::Function);
                        pushed_scope = true;
                    }
                    "compact_constructor_declaration" => {
                        let sym = plugin.extract_compact_constructor(
                            node,
                            source,
                            &file_path_str,
                            &extract_ctx,
                        )?;
                        enter_function_scope(&mut ctx, node, sym, ScopeKind::Function);
                        pushed_scope = true;
                    }
                    "static_initializer" => {
                        let sym = plugin.extract_static_initializer(
                            node,
                            source,
                            &file_path_str,
                            &extract_ctx,
                        )?;
                        enter_function_scope(&mut ctx, node, sym, ScopeKind::Function);
                        pushed_scope = true;
                    }
                    "block" if node.parent().map(|p| p.kind()) == Some("class_body") => {
                        let sym = plugin.extract_instance_initializer(
                            node,
                            source,
                            &file_path_str,
                            &extract_ctx,
                        )?;
                        enter_function_scope(&mut ctx, node, sym, ScopeKind::Function);
                        pushed_scope = true;
                    }
                    "class_declaration" => {
                        let sym = plugin.extract_type(
                            node,
                            source,
                            &file_path_str,
                            SymbolType::Class,
                            &extract_ctx,
                        )?;
                        enter_type_scope(&mut ctx, node, sym, ScopeKind::Class);
                        pushed_scope = true;
                    }
                    "interface_declaration" => {
                        let sym = plugin.extract_type(
                            node,
                            source,
                            &file_path_str,
                            SymbolType::Interface,
                            &extract_ctx,
                        )?;
                        enter_type_scope(&mut ctx, node, sym, ScopeKind::Interface);
                        pushed_scope = true;
                    }
                    "enum_declaration" => {
                        let sym = plugin.extract_type(
                            node,
                            source,
                            &file_path_str,
                            SymbolType::Enum,
                            &extract_ctx,
                        )?;
                        enter_type_scope(&mut ctx, node, sym, ScopeKind::Enum);
                        pushed_scope = true;
                    }
                    "annotation_type_declaration" => {
                        let sym = plugin.extract_type(
                            node,
                            source,
                            &file_path_str,
                            SymbolType::Annotation,
                            &extract_ctx,
                        )?;
                        enter_type_scope(&mut ctx, node, sym, ScopeKind::Interface);
                        pushed_scope = true;
                    }
                    "record_declaration" => {
                        let sym =
                            plugin.extract_record(node, source, &file_path_str, &extract_ctx)?;
                        enter_type_scope(&mut ctx, node, sym, ScopeKind::Class);
                        pushed_scope = true;
                    }
                    "annotation_type_element_declaration" => {
                        ctx.symbols.push(plugin.extract_annotation_element(
                            node,
                            source,
                            &file_path_str,
                            &extract_ctx,
                        )?);
                    }
                    "lambda_expression" => {
                        let sym =
                            plugin.extract_lambda(node, source, &file_path_str, &extract_ctx)?;
                        enter_function_scope(&mut ctx, node, sym, ScopeKind::Lambda);
                        pushed_scope = true;
                    }
                    "package_declaration" => {
                        ctx.symbols
                            .push(plugin.extract_package_symbol(node, source, &file_path_str)?);
                    }
                    "module_declaration" => {
                        let sym = plugin.extract_module_symbol(node, source, &file_path_str)?;
                        let name = sym.name.clone();
                        let qn = sym
                            .qualified_name
                            .clone()
                            .unwrap_or_else(|| name.clone());
                        ctx.symbols.push(sym);
                        ctx.enter_scope(
                            ScopeKind::Module,
                            Some(name),
                            qn,
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        pushed_scope = true;
                    }
                    "import_declaration" => {
                        let text = node.utf8_text(source)?.trim().to_string();
                        ctx.symbols.push(Symbol {
                            name: text.clone(),
                            symbol_type: SymbolType::Import,
                            qualified_name: None,
                            location: SourceLocation {
                                file: file_path_str.to_string(),
                                start_line: node.start_position().row + 1,
                                end_line: node.end_position().row + 1,
                                start_column: 0,
                                end_column: 0,
                            },
                            signature: None,
                            return_type: None,
                            parameters: vec![],
                            fields: vec![],
                            modifiers: vec![],
                            documentation: None,
                            metadata: serde_json::json!({ "language": "java" }),
                        });
                    }
                    _ => {}
                }

                if JAVA_CALL_KINDS.contains(&kind) {
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
                        k if BRANCH_KINDS.contains(&k) => {
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
                                    | "enhanced_for_statement"
                                    | "do_statement"
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
                        "binary_expression" => {
                            if let Some(op) = node.child_by_field_name("operator")
                                && matches!(op.kind(), "&&" | "||")
                            {
                                ctx.scopes.note_branch(1);
                            }
                        }
                        "switch_label" => {
                            let mut cursor = node.walk();
                            if node.children(&mut cursor).next().is_some() {
                                ctx.scopes.note_branch(1 + cognitive_nesting);
                            }
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
            "java",
            &mut relations,
        );
    }

    plugin.collect_non_call_relations(
        root,
        source,
        file_path,
        &extract_ctx,
        &ctx.symbols,
        &mut relations,
    )?;

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}

fn enter_function_scope(
    ctx: &mut SinglePassCtx,
    node: Node,
    sym: Symbol,
    kind: ScopeKind,
) {
    let params = sym.parameters.len() as u32;
    let loc = ((node.end_position().row - node.start_position().row) + 1).max(1) as u32;
    let name = sym.name.clone();
    let qn = sym
        .qualified_name
        .clone()
        .unwrap_or_else(|| name.clone());
    ctx.symbols.push(sym);
    ctx.enter_scope(
        kind,
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
}

fn enter_type_scope(ctx: &mut SinglePassCtx, node: Node, sym: Symbol, kind: ScopeKind) {
    let name = sym.name.clone();
    let qn = sym
        .qualified_name
        .clone()
        .unwrap_or_else(|| name.clone());
    ctx.symbols.push(sym);
    ctx.enter_scope(
        kind,
        Some(name),
        qn,
        node.start_byte(),
        node.end_byte(),
        true,
    );
}
