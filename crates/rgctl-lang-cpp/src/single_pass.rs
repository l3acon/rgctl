//! Single-pass AST extraction for C++ (`ScopeStack` + `KindDispatchTable`).

use crate::depth;
use crate::plugin::{
    collect_base_relations, enrich_cpp_namespace_hint, scoped_type_name, source_location,
    template_enclosed_type, type_definition_typedef_name, type_name, type_name_from_type_node,
    using_import_names, CppPlugin,
};
use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::Node;

const MANIFEST_JSON: &str = include_str!("../cpp-ast-coverage.json");

/// Plugin-handled AST kinds (source of truth for declared_ast_handlers).
const HANDLED_KINDS: &[(&str, HandlerMask)] = &[
    ("alias_declaration", HandlerMask::SYMBOL),
    ("assignment_expression", HandlerMask::AST_SKELETON),
    ("base_class_clause", HandlerMask::RELATION),
    ("break_statement", HandlerMask::CFG_STMT),
    ("call_expression", HandlerMask::RELATION),
    ("case_statement", HandlerMask::CFG_STMT),
    ("catch_clause", HandlerMask::CFG_STMT),
    ("class_specifier", HandlerMask::SYMBOL),
    ("co_await_expression", HandlerMask::CFG_STMT),
    ("co_return_statement", HandlerMask::CFG_STMT),
    ("co_yield_statement", HandlerMask::CFG_STMT),
    ("compound_statement", HandlerMask::CFG_STMT),
    ("concept_definition", HandlerMask::SYMBOL),
    ("continue_statement", HandlerMask::CFG_STMT),
    ("do_statement", HandlerMask::CFG_STMT),
    ("else_clause", HandlerMask::CFG_STMT),
    ("enum_specifier", HandlerMask::SYMBOL),
    ("enumerator", HandlerMask::SYMBOL),
    ("field_declaration", HandlerMask::SYMBOL),
    ("for_range_loop", HandlerMask::CFG_STMT),
    ("for_statement", HandlerMask::CFG_STMT),
    ("function_definition", HandlerMask::SYMBOL),
    ("goto_statement", HandlerMask::CFG_STMT),
    ("if_statement", HandlerMask::CFG_STMT),
    ("namespace_definition", HandlerMask::SYMBOL),
    ("new_expression", HandlerMask::RELATION),
    ("preproc_def", HandlerMask::SYMBOL),
    ("preproc_function_def", HandlerMask::SYMBOL),
    ("preproc_if", HandlerMask::CFG_STMT),
    ("preproc_ifdef", HandlerMask::CFG_STMT),
    ("preproc_include", HandlerMask::RELATION),
    ("return_statement", HandlerMask::CFG_STMT),
    ("struct_specifier", HandlerMask::SYMBOL),
    ("switch_statement", HandlerMask::CFG_STMT),
    ("template_function", HandlerMask::RELATION),
    ("template_method", HandlerMask::RELATION),
    ("throw_statement", HandlerMask::CFG_STMT),
    ("try_statement", HandlerMask::CFG_STMT),
    ("type_definition", HandlerMask::SYMBOL),
    ("union_specifier", HandlerMask::SYMBOL),
    ("using_declaration", HandlerMask::RELATION),
    ("while_statement", HandlerMask::CFG_STMT),
];

const AST_MAX_DEPTH: usize = 2048;

static HANDLERS: OnceLock<AstHandlerTable> = OnceLock::new();
static DISPATCH: OnceLock<KindDispatchTable> = OnceLock::new();

fn cpp_grammar() -> tree_sitter::Language {
    tree_sitter_cpp::LANGUAGE.into()
}

/// Process-wide AST handler table (coverage sync + introspection).
pub fn cpp_ast_handlers() -> &'static AstHandlerTable {
    HANDLERS.get_or_init(|| {
        
        handler_table_from_kinds(&cpp_grammar(), HANDLED_KINDS)
    })
}

fn cpp_dispatch() -> &'static KindDispatchTable {
    DISPATCH.get_or_init(|| {
        dispatch_table_from_manifest(&cpp_grammar(), MANIFEST_JSON)
    })
}

struct PendingCall<'a> {
    node: Node<'a>,
    from_symbol_idx: usize,
    scope: Option<String>,
}

enum Visit<'a> {
    Pre(Node<'a>, usize, Option<String>),
    LeaveScope,
    LeaveGuard,
}

/// Single iterative DFS: symbols + deferred calls + inheritance/new + complexity.
pub fn extract_all_single_pass(
    plugin: &CppPlugin,
    file_path: &Path,
    source: &[u8],
) -> Result<ExtractAllResult> {
    let tree = parse_with_pool("cpp", cpp_grammar(), source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        line: 0,
        message: e,
    })?;
    let root = tree.root_node();
    let file_path_str = file_path.to_string_lossy();
    let dispatch = cpp_dispatch();

    let mut ctx = SinglePassCtx::new(dispatch);
    let mut relations = Vec::new();
    let mut pending_calls: Vec<PendingCall<'_>> = Vec::new();
    let mut depth_warned = false;

    let mut stack = vec![Visit::Pre(root, 0, None)];

    while let Some(visit) = stack.pop() {
        match visit {
            Visit::LeaveScope => {
                if let Some(frame) = ctx.leave_scope() {
                    ctx.harvest_function_complexity(&frame);
                }
            }
            Visit::LeaveGuard => {
                ctx.scopes.leave_branch();
                let _ = ctx.leave_scope();
            }
            Visit::Pre(node, depth, scope) => {
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

                let mask = rgctl_plugin_api::dispatch_lookup_tracked(dispatch, node.kind_id());
                let _ = mask; // keep for future passive skip
                let kind = node.kind();

                let next_scope = match kind {
                    "namespace_definition" => node
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source).ok().map(str::to_string))
                        .map(|name| {
                            scope
                                .as_ref()
                                .map(|s| format!("{s}::{name}"))
                                .unwrap_or(name)
                        }),
                    "class_specifier" | "struct_specifier" => type_name(node, source),
                    _ => None,
                };
                let child_scope = next_scope.clone().or_else(|| scope.clone());
                let scope_for_fn = child_scope.as_deref();

                let mut pushed_scope = false;
                match kind {
                    "function_definition" | "declaration" => {
                        let is_prototype =
                            kind == "declaration" && node.child_by_field_name("body").is_none();
                        if !is_prototype {
                            if let Some(mut sym) =
                                plugin.extract_function(node, source, &file_path_str, scope_for_fn)?
                            {
                                sym.modifiers = crate::plugin::method_modifiers(node, source);
                                let params = sym.parameters.len() as u32;
                                let loc = ((node.end_position().row - node.start_position().row)
                                    + 1)
                                .max(1) as u32;
                                let name = sym.name.clone();
                                let qn = sym
                                    .qualified_name
                                    .clone()
                                    .unwrap_or_else(|| name.clone());
                                ctx.symbols.push(sym);
                                // Enter function scope when a body is present.
                                if kind == "function_definition"
                                    || node.child_by_field_name("body").is_some()
                                {
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
                    }
                    "template_declaration" => {
                        if let Some(mut sym) =
                            plugin.extract_function(node, source, &file_path_str, scope_for_fn)?
                        {
                            sym.modifiers = crate::plugin::method_modifiers(node, source);
                            sym.metadata["is_template"] = serde_json::Value::Bool(true);
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
                        } else if let Some(child) = template_enclosed_type(node) {
                            if type_name(child, source).is_some() {
                                let st = if child.kind() == "class_specifier" {
                                    SymbolType::Class
                                } else {
                                    SymbolType::Struct
                                };
                                let mut sym =
                                    plugin.extract_class(child, source, &file_path_str, st)?;
                                sym.metadata["is_template"] = serde_json::Value::Bool(true);
                                let name = sym.name.clone();
                                ctx.symbols.push(sym);
                                let sk = if st == SymbolType::Class {
                                    ScopeKind::Class
                                } else {
                                    ScopeKind::Struct
                                };
                                ctx.enter_scope(
                                    sk,
                                    Some(name.clone()),
                                    name,
                                    child.start_byte(),
                                    child.end_byte(),
                                    true,
                                );
                                pushed_scope = true;
                            }
                        }
                    }
                    "class_specifier" => {
                        if type_name(node, source).is_some() {
                            let sym = plugin.extract_class(
                                node,
                                source,
                                &file_path_str,
                                SymbolType::Class,
                            )?;
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

                            let mut cursor = node.walk();
                            for child in node.children(&mut cursor) {
                                if child.kind() == "base_class_clause" {
                                    collect_base_relations(
                                        &name,
                                        child,
                                        source,
                                        file_path,
                                        &mut relations,
                                    );
                                }
                            }
                        }
                    }
                    "struct_specifier" => {
                        if type_name(node, source).is_some() {
                            let sym = plugin.extract_class(
                                node,
                                source,
                                &file_path_str,
                                SymbolType::Struct,
                            )?;
                            let name = sym.name.clone();
                            ctx.symbols.push(sym);
                            ctx.enter_scope(
                                ScopeKind::Struct,
                                Some(name.clone()),
                                name.clone(),
                                node.start_byte(),
                                node.end_byte(),
                                true,
                            );
                            pushed_scope = true;

                            let mut cursor = node.walk();
                            for child in node.children(&mut cursor) {
                                if child.kind() == "base_class_clause" {
                                    collect_base_relations(
                                        &name,
                                        child,
                                        source,
                                        file_path,
                                        &mut relations,
                                    );
                                }
                            }
                        }
                    }
                    "enum_specifier" => {
                        if type_name(node, source).is_some() {
                            let sym = plugin.extract_class(
                                node,
                                source,
                                &file_path_str,
                                SymbolType::Enum,
                            )?;
                            let name = sym.name.clone();
                            ctx.symbols.push(sym);
                            ctx.enter_scope(
                                ScopeKind::Enum,
                                Some(name.clone()),
                                name,
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
                            if matches!(child.kind(), "class_specifier" | "struct_specifier")
                                && type_name(child, source).is_some()
                            {
                                let st = if child.kind() == "class_specifier" {
                                    SymbolType::Class
                                } else {
                                    SymbolType::Struct
                                };
                                ctx.symbols.push(plugin.extract_class(
                                    child,
                                    source,
                                    &file_path_str,
                                    st,
                                )?);
                            } else if child.kind() == "union_specifier"
                                && (type_name(child, source).is_some() || typedef_name.is_some())
                            {
                                if let Some(sym) = depth::extract_union_specifier_named(
                                    child,
                                    source,
                                    &file_path_str,
                                    &ctx.scopes,
                                    typedef_name.as_deref(),
                                ) {
                                    ctx.symbols.push(sym);
                                }
                            }
                        }
                    }
                    "namespace_definition" => {
                        if let Some(sym) =
                            depth::extract_namespace_definition(node, source, &file_path_str)
                        {
                            ctx.symbols.push(sym);
                        }
                        let name = next_scope.clone();
                        let qn = name.clone().unwrap_or_default();
                        ctx.enter_scope(
                            ScopeKind::Module,
                            name,
                            qn,
                            node.start_byte(),
                            node.end_byte(),
                            false,
                        );
                        pushed_scope = true;
                    }
                    "alias_declaration" => {
                        if let Some(sym) = depth::extract_alias_declaration(
                            node,
                            source,
                            &file_path_str,
                            &ctx.scopes,
                        ) {
                            ctx.symbols.push(sym);
                        }
                    }
                    "concept_definition" => {
                        if let Some(sym) = depth::extract_concept_definition(
                            node,
                            source,
                            &file_path_str,
                            &ctx.scopes,
                        ) {
                            ctx.symbols.push(sym);
                        }
                    }
                    "union_specifier" => {
                        if let Some(sym) = depth::extract_union_specifier(
                            node,
                            source,
                            &file_path_str,
                            &ctx.scopes,
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
                    "preproc_include" => {
                        let text = node.utf8_text(source)?.trim().to_string();
                        ctx.symbols.push(Symbol {
                            name: text,
                            symbol_type: SymbolType::Import,
                            qualified_name: None,
                            location: source_location(node, &file_path_str),
                            signature: None,
                            return_type: None,
                            parameters: vec![],
                            fields: vec![],
                            modifiers: vec![],
                            documentation: None,
                            metadata: serde_json::json!({ "language": "cpp" }),
                        });
                    }
                    "using_declaration" => {
                        let (name, qualified) = using_import_names(node, source);
                        ctx.symbols.push(Symbol {
                            name: name.clone(),
                            symbol_type: SymbolType::Import,
                            qualified_name: qualified,
                            location: source_location(node, &file_path_str),
                            signature: Some(node.utf8_text(source)?.trim().to_string()),
                            return_type: None,
                            parameters: vec![],
                            fields: vec![],
                            modifiers: vec![],
                            documentation: None,
                            metadata: serde_json::json!({ "language": "cpp", "import_kind": "using" }),
                        });
                        if let Some(rel) =
                            depth::using_declaration_relation(node, source, &file_path_str)
                        {
                            relations.push(rel);
                        }
                    }
                    "new_expression" => {
                        if let Some(from_fn) =
                            containing_function_from_scope(&ctx.scopes, &ctx.symbols)
                        {
                            let from = from_fn
                                .qualified_name
                                .clone()
                                .unwrap_or_else(|| from_fn.name.clone());
                            if let Some(type_node) = node.child_by_field_name("type") {
                                if let Some(target) = type_name_from_type_node(type_node, source) {
                                    let scoped = scoped_type_name(type_node, source)
                                        .unwrap_or_else(|| target.clone());
                                    relations.push(Relation {
                                        from,
                                        to: target,
                                        relation_type: RelationType::Instantiates,
                                        location: source_location(node, &file_path_str),
                                        metadata: serde_json::json!({
                                            "language": "cpp",
                                            "is_array": node.child_by_field_name("declarator").is_some(),
                                        }),
                                        to_qualified_hint: Some(scoped),
                                        to_type_hint: None,
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }

                if CPP_CALL_KINDS.contains(&kind) {
                    if let Some(frame) = ctx.scopes.current_function() {
                        if let Some(idx) = frame.symbol_idx {
                            pending_calls.push(PendingCall {
                                node,
                                from_symbol_idx: idx,
                                scope: child_scope.clone(),
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
                        | "for_range_loop"
                        | "do_statement"
                        | "conditional_expression"
                        | "catch_clause" => {
                            let delta = 1 + ctx
                                .scopes
                                .current_function()
                                .map(|f| f.complexity.cognitive_nesting)
                                .unwrap_or(0);
                            ctx.scopes.note_branch(delta);
                            ctx.scopes.enter_branch();
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
                        "case_statement" => {
                            let delta = 1 + ctx
                                .scopes
                                .current_function()
                                .map(|f| f.complexity.cognitive_nesting)
                                .unwrap_or(0);
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

                for i in (0..node.child_count()).rev() {
                    if let Some(child) = node.child(i) {
                        stack.push(Visit::Pre(child, depth + 1, child_scope.clone()));
                    }
                }
            }
        }
    }

    for pending in pending_calls {
        let Some(from_fn) = ctx.symbols.get(pending.from_symbol_idx) else {
            continue;
        };
        let before = relations.len();
        push_call_relation_from(
            pending.node,
            source,
            file_path,
            &ctx.symbols,
            from_fn,
            "cpp",
            &mut relations,
        );
        if relations.len() > before {
            if let Some(rel) = relations.last_mut() {
                if let Some(scope) = pending.scope.as_deref() {
                    if let Some(func) = pending.node.child_by_field_name("function") {
                        if rel.to_qualified_hint.is_none() && func.kind() == "identifier" {
                            let callee = func.utf8_text(source).ok().unwrap_or("").trim();
                            if !callee.is_empty() {
                                rel.to_qualified_hint = Some(format!("{scope}::{callee}"));
                            }
                        } else if let Some(hint) = rel.to_qualified_hint.clone() {
                            if let Some(enriched) = enrich_cpp_namespace_hint(&hint, scope) {
                                rel.to_qualified_hint = Some(enriched);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(ExtractAllResult::from_parts(ctx.symbols, relations))
}
