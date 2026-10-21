//! Single-pass AST extraction for PHP (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::{
    import_map_from_symbols, normalize_namespace, php_grammar, push_php_call_relation, PhpPlugin,
    PHP_COMPLEXITY_BRANCH_KINDS, PHP_FUNCTION_KINDS,
};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../php-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

/// Process-wide AST handler table (coverage sync + introspection).
pub fn php_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&php_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn php_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&php_grammar(), MANIFEST_JSON);
        dispatch
    })
}

fn push_children<'a>(stack: &mut Vec<(Node<'a>, usize)>, node: Node<'a>, depth: usize) {
    for i in (0..node.child_count()).rev() {
        if let Some(child) = node.child(i) {
            stack.push((child, depth + 1));
        }
    }
}

struct PendingCall<'a> {
    node: Node<'a>,
    /// Enclosing function at call site (forward-ref flush uses full symbol list).
    #[allow(dead_code)]
    from_symbol_idx: usize,
}

enum Visit<'a> {
    Pre(Node<'a>, usize),
    LeaveScope,
    LeaveGuard,
}

/// Single iterative DFS: symbols + deferred PHP calls + inheritance + complexity.
pub fn extract_all_single_pass(
    plugin: &PhpPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("php", php_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = php_dispatch();

    let mut ctx = SinglePassCtx::new(dispatch.clone());
    let mut relations = Vec::new();
    let mut pending_calls: Vec<PendingCall<'_>> = Vec::new();
    let mut namespace: Option<String> = None;
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

                if kind == "namespace_definition" {
                    if let Some(ns) = node
                        .child_by_field_name("name")
                        .and_then(|n| normalize_namespace(n, source))
                    {
                        namespace = Some(ns);
                    }
                }

                match kind {
                    "function_definition" => {
                        if let Some(sym) =
                            plugin.extract_function(node, source, &file_path_str, namespace.as_deref())?
                        {
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
                        plugin.extract_embedded_symbols(
                            node,
                            source,
                            &file_path_str,
                            namespace.as_deref(),
                            &mut ctx.symbols,
                        )?;
                    }
                    "method_declaration" => {
                        // Prior extract only pulled methods from `class_declaration` bodies
                        // (not interface/trait/enum) plus orphan methods with no enclosing class.
                        let type_frame = ctx.scopes.current_type();
                        let in_class = type_frame.is_some_and(|t| t.kind == ScopeKind::Class);
                        let owner = if in_class {
                            type_frame.map(|t| t.qualified_prefix.clone())
                        } else {
                            None
                        };
                        if in_class
                            || (type_frame.is_none()
                                && plugin.find_enclosing_class_name(node, source).is_none())
                        {
                            if let Some(sym) = plugin.extract_method(
                                node,
                                source,
                                &file_path_str,
                                namespace.as_deref(),
                                owner.as_deref(),
                            )? {
                                let params = sym.parameters.len() as u32;
                                let loc =
                                    ((node.end_position().row - node.start_position().row) + 1)
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
                            plugin.extract_embedded_symbols(
                                node,
                                source,
                                &file_path_str,
                                namespace.as_deref(),
                                &mut ctx.symbols,
                            )?;
                        }
                    }
                    "arrow_function" | "anonymous_function" => {
                        if let Some(sym) = plugin.extract_anonymous_function(
                            node,
                            source,
                            &file_path_str,
                            namespace.as_deref(),
                        )? {
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
                                ScopeKind::Lambda,
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
                    "class_declaration" => {
                        let sym =
                            plugin.extract_class(node, source, &file_path_str, namespace.as_deref())?;
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
                        let sym = plugin.extract_interface(
                            node,
                            source,
                            &file_path_str,
                            namespace.as_deref(),
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
                    "trait_declaration" => {
                        let sym =
                            plugin.extract_trait(node, source, &file_path_str, namespace.as_deref())?;
                        let name = sym.name.clone();
                        let qn = sym
                            .qualified_name
                            .clone()
                            .unwrap_or_else(|| name.clone());
                        ctx.symbols.push(sym);
                        ctx.enter_scope(
                            ScopeKind::Trait,
                            Some(name),
                            qn,
                            node.start_byte(),
                            node.end_byte(),
                            true,
                        );
                        pushed_scope = true;
                    }
                    "enum_declaration" => {
                        let sym =
                            plugin.extract_enum(node, source, &file_path_str, namespace.as_deref())?;
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
                    "namespace_use_declaration" => {
                        plugin.extract_namespace_imports(
                            node,
                            source,
                            &file_path_str,
                            &mut ctx.symbols,
                        )?;
                    }
                    _ => {}
                }

                if PHP_CALL_KINDS.contains(&kind) {
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
                    if PHP_COMPLEXITY_BRANCH_KINDS.contains(&kind) {
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
                    } else if kind == "return_statement" {
                        ctx.scopes.note_return();
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

    let import_map = import_map_from_symbols(&ctx.symbols);
    let function_symbols: Vec<&Symbol> = ctx
        .symbols
        .iter()
        .filter(|s| s.symbol_type == SymbolType::Function)
        .collect();
    for pending in pending_calls {
        push_php_call_relation(
            pending.node,
            source,
            file_path,
            &ctx.symbols,
            &function_symbols,
            &import_map,
            &mut relations,
        );
    }

    plugin.extract_inheritance(root, source, file_path, None, &mut relations)?;

    let _ = PHP_FUNCTION_KINDS;
    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
