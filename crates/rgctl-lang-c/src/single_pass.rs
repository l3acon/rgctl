//! Single-pass AST extraction for C (`ScopeStack` + `KindDispatchTable`).

use crate::depth;
use crate::plugin::{
    extract_struct_fields, include_path_from_node, source_location, struct_name,
    type_definition_typedef_name, CPlugin,
};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../c-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn c_grammar() -> tree_sitter::Language {
    tree_sitter_c::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn c_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&c_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn c_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&c_grammar(), MANIFEST_JSON);
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

/// Single iterative DFS: symbols + deferred calls + in-pass complexity.
pub fn extract_all_single_pass(
    plugin: &CPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("c", c_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = c_dispatch();

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

                let mut pushed_scope = false;
                match kind {
                    "function_definition" | "declaration" => {
                        if let Some(sym) = plugin.extract_function(node, source, &file_path_str)? {
                            let params = sym.parameters.len() as u32;
                            let loc = ((node.end_position().row - node.start_position().row) + 1)
                                .max(1) as u32;
                            let name = sym.name.clone();
                            let qn = sym
                                .qualified_name
                                .clone()
                                .unwrap_or_else(|| name.clone());
                            ctx.symbols.push(sym);
                            // Prototypes (`declaration`) have no body — no function scope.
                            if kind == "function_definition" {
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
                    }
                    "struct_specifier" => {
                        if struct_name(node, source).is_some() {
                            let sym = plugin.extract_struct(node, source, &file_path_str, None)?;
                            let name = sym.name.clone();
                            let qn = sym
                                .qualified_name
                                .clone()
                                .unwrap_or_else(|| name.clone());
                            ctx.symbols.push(sym);
                            ctx.enter_scope(
                                ScopeKind::Struct,
                                Some(name),
                                qn,
                                node.start_byte(),
                                node.end_byte(),
                                true,
                            );
                            pushed_scope = true;
                        }
                    }
                    "enum_specifier" => {
                        if node.child_by_field_name("name").is_some() {
                            let sym = plugin.extract_enum(node, source, &file_path_str)?;
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
                    }
                    "type_definition" => {
                        let typedef_name = type_definition_typedef_name(node, source);
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            if child.kind() == "struct_specifier"
                                && (struct_name(child, source).is_some() || typedef_name.is_some())
                            {
                                ctx.symbols.push(plugin.extract_struct(
                                    child,
                                    source,
                                    &file_path_str,
                                    typedef_name.as_deref(),
                                )?);
                            } else if child.kind() == "enum_specifier"
                                && (child.child_by_field_name("name").is_some()
                                    || typedef_name.is_some())
                            {
                                ctx.symbols.push(plugin.extract_enum_with_typedef(
                                    child,
                                    source,
                                    &file_path_str,
                                    typedef_name.as_deref(),
                                )?);
                            }
                        }
                    }
                    "union_specifier" => {
                        if node.child_by_field_name("name").is_some() {
                            let fields = extract_struct_fields(node, source).unwrap_or_default();
                            if let Some(sym) = depth::extract_union_specifier(
                                node,
                                source,
                                &file_path_str,
                                fields,
                            ) {
                                let name = sym.name.clone();
                                let qn = sym
                                    .qualified_name
                                    .clone()
                                    .unwrap_or_else(|| name.clone());
                                ctx.symbols.push(sym);
                                ctx.enter_scope(
                                    ScopeKind::Struct,
                                    Some(name),
                                    qn,
                                    node.start_byte(),
                                    node.end_byte(),
                                    true,
                                );
                                pushed_scope = true;
                            }
                        }
                    }
                    "preproc_def" => {
                        if let Some(sym) =
                            depth::extract_preproc_def(node, source, &file_path_str)
                        {
                            ctx.symbols.push(sym);
                        }
                    }
                    "preproc_function_def" => {
                        if let Some(sym) =
                            depth::extract_preproc_function_def(node, source, &file_path_str)
                        {
                            ctx.symbols.push(sym);
                        }
                    }
                    "preproc_include" | "preprocessor_include" => {
                        let path = include_path_from_node(node, source)?;
                        ctx.symbols.push(Symbol {
                            name: path.clone(),
                            symbol_type: SymbolType::Import,
                            qualified_name: None,
                            location: source_location(node, &file_path_str),
                            signature: None,
                            return_type: None,
                            parameters: vec![],
                            fields: vec![],
                            modifiers: vec![],
                            documentation: None,
                            metadata: serde_json::json!({
                                "language": "c",
                                "kind": "include",
                            }),
                        });
                    }
                    _ => {}
                }

                if C_CALL_KINDS.contains(&kind) {
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
                        | "do_statement"
                        | "conditional_expression" => {
                            let delta = 1 + cognitive_nesting;
                            ctx.scopes.note_branch(delta);
                            if let Some(f) = ctx.scopes.current_function_mut() {
                                f.complexity.max_nesting =
                                    f.complexity.max_nesting.max(cognitive_nesting + 1);
                            }
                            cognitive_nesting = cognitive_nesting.saturating_add(1);
                            let depth = ctx.scopes.depth();
                            ctx.scopes.push(ScopeFrame::new(
                                ScopeKind::GuardBlock,
                                None,
                                String::new(),
                                node.start_byte(),
                                node.end_byte(),
                                depth,
                            ));
                            stack.push(Visit::LeaveGuard);
                        }
                        "case_statement" => {
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
            "c",
            &mut relations,
        );
    }

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
