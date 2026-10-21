//! Single-pass AST extraction for Groovy (`ScopeStack` + `KindDispatchTable`).

use crate::plugin::{GroovyPlugin, GROOVY_BRANCH_KINDS, GROOVY_CALL_KINDS};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../groovy-ast-coverage.json");
const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn groovy_grammar() -> tree_sitter::Language {
    tree_sitter_groovy::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn groovy_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        let (handlers, _) = coverage_tables_from_manifest(&groovy_grammar(), MANIFEST_JSON);
        handlers
    })
}

fn groovy_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        let (_, dispatch) = coverage_tables_from_manifest(&groovy_grammar(), MANIFEST_JSON);
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

struct PendingCall<'a> {
    node: Node<'a>,
    from_symbol_idx: usize,
}

enum Visit<'a> {
    Pre(Node<'a>, usize),
    LeaveScope,
    LeaveGuard,
}

/// Single iterative DFS: symbols + deferred calls + extends/annotations + complexity.
pub fn extract_all_single_pass(
    plugin: &GroovyPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("groovy", groovy_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 1,
        message: e,
    })?;
    let root = tree.root_node();
    let file = file_path.to_string_lossy();
    let package = GroovyPlugin::package_name(root, source);
    let dispatch = groovy_dispatch();

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
                    "class_declaration" | "interface_declaration" | "enum_declaration"
                    | "annotation_type_declaration" => {
                        if let Some(simple) = GroovyPlugin::type_name(node, source) {
                            let symbol_type = match kind {
                                "interface_declaration" => SymbolType::Interface,
                                "enum_declaration" => SymbolType::Enum,
                                "annotation_type_declaration" => SymbolType::Annotation,
                                _ => SymbolType::Class,
                            };
                            let scope_kind = match kind {
                                "interface_declaration" => ScopeKind::Interface,
                                "enum_declaration" => ScopeKind::Enum,
                                _ => ScopeKind::Class,
                            };
                            let qn = GroovyPlugin::qualify(package.as_deref(), &simple);
                            ctx.symbols.push(Symbol {
                                name: simple.clone(),
                                symbol_type,
                                qualified_name: Some(qn.clone()),
                                location: GroovyPlugin::loc(&file, node),
                                signature: node
                                    .utf8_text(source)
                                    .ok()
                                    .map(|s| s.lines().next().unwrap_or("").trim().to_string()),
                                return_type: None,
                                parameters: vec![],
                                fields: vec![],
                                modifiers: vec![],
                                documentation: None,
                                metadata: serde_json::json!({ "language": "groovy" }),
                            });
                            ctx.enter_scope(
                                scope_kind,
                                Some(simple),
                                qn,
                                node.start_byte(),
                                node.end_byte(),
                                true,
                            );
                            pushed_scope = true;
                        }
                    }
                    "method_declaration" | "function_definition" => {
                        if let Some(sym) =
                            plugin.extract_method_symbol(node, source, &file, package.as_deref())
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
                    }
                    "constructor_declaration" | "compact_constructor_declaration" => {
                        if let Some(sym) = plugin
                            .extract_constructor_symbol(node, source, &file, package.as_deref())
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
                    }
                    "import_declaration" => {
                        if let Some(sym) = GroovyPlugin::extract_import_symbol(node, source, &file)
                        {
                            ctx.symbols.push(sym);
                        }
                    }
                    _ => {}
                }

                if GROOVY_CALL_KINDS.contains(&kind) {
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
                    if GROOVY_BRANCH_KINDS.contains(&kind) {
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
            "groovy",
            &mut relations,
        );
    }

    // Extends / implements / AnnotatedWith (same as prior relations_from_tree post-pass).
    plugin.append_type_and_annotation_relations(root, source, file_path, &mut relations)?;

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
