//! Groovy language plugin — best-effort symbols/calls with dynamic-call honesty.

use rgctl_plugin_api::*;
use rgctl_plugin_api::{Error, Result};
use std::path::Path;
use tree_sitter::{Node, Parser};

const BRANCH_KINDS: &[&str] = &[
    "if_statement",
    "while_statement",
    "for_statement",
    "enhanced_for_statement",
    "do_statement",
    "switch_expression",
    "catch_clause",
];

/// Groovy Tier 1 plugin.
pub struct GroovyPlugin;

impl GroovyPlugin {
    /// Create a new Groovy plugin.
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    fn parse(&self, file_path: &Path, source: &[u8]) -> Result<tree_sitter::Tree> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_groovy::LANGUAGE.into())
            .map_err(|e| Error::PluginError(format!("Failed to set Groovy grammar: {e}")))?;
        parser.parse(source, None).ok_or_else(|| Error::ParseError {
            file: file_path.to_path_buf(),
            line: 1,
            message: "Failed to parse Groovy source".to_string(),
        })
    }

    fn package_name(root: Node, source: &[u8]) -> Option<String> {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if node.kind() == "package_declaration" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if matches!(
                        child.kind(),
                        "scoped_identifier" | "identifier" | "type_identifier"
                    ) && let Ok(t) = child.utf8_text(source)
                    {
                        return Some(t.trim().to_string());
                    }
                }
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor).collect::<Vec<_>>().into_iter().rev() {
                stack.push(child);
            }
        }
        None
    }

    fn qualify(package: Option<&str>, path: &str) -> String {
        match package {
            Some(pkg) if !pkg.is_empty() => format!("{pkg}.{path}"),
            _ => path.to_string(),
        }
    }

    fn loc(file_path: &str, node: Node) -> SourceLocation {
        SourceLocation {
            file: file_path.to_string(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            start_column: node.start_position().column,
            end_column: node.end_position().column,
        }
    }

    fn type_name(node: Node, source: &[u8]) -> Option<String> {
        node.child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok())
            .map(|s| s.trim().to_string())
            .or_else(|| {
                let mut cursor = node.walk();
                node.children(&mut cursor).find_map(|c| {
                    if matches!(c.kind(), "identifier" | "type_identifier") {
                        c.utf8_text(source).ok().map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                })
            })
    }

    fn enclosing_class(node: Node, source: &[u8]) -> Option<String> {
        let mut current = node.parent();
        while let Some(n) = current {
            if matches!(
                n.kind(),
                "class_declaration" | "interface_declaration" | "enum_declaration"
            ) {
                return Self::type_name(n, source);
            }
            current = n.parent();
        }
        None
    }

    fn extract_parameters(&self, node: Node, source: &[u8]) -> Vec<Parameter> {
        let Some(params) = node
            .child_by_field_name("parameters")
            .or_else(|| {
                let mut c = node.walk();
                node.children(&mut c)
                    .find(|ch| matches!(ch.kind(), "formal_parameters" | "inferred_parameters"))
            })
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            if child.kind() != "formal_parameter" {
                continue;
            }
            let name = child
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source).ok())
                .map(|s| s.trim().to_string())
                .or_else(|| {
                    let mut c = child.walk();
                    child.children(&mut c).find_map(|n| {
                        if n.kind() == "identifier" {
                            n.utf8_text(source).ok().map(|s| s.trim().to_string())
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| "_".into());
            let param_type = child
                .child_by_field_name("type")
                .and_then(|n| n.utf8_text(source).ok())
                .map(|s| s.trim().to_string());
            out.push(Parameter {
                name,
                param_type,
                default_value: None,
            });
        }
        out
    }

    fn symbols_from_tree(
        &self,
        root: Node,
        source: &[u8],
        file_path: &Path,
    ) -> Result<Vec<Symbol>> {
        let file = file_path.to_string_lossy();
        let package = Self::package_name(root, source);
        let mut symbols = Vec::with_capacity(32);
        let mut stack = vec![root];

        while let Some(node) = stack.pop() {
            match node.kind() {
                "class_declaration" | "interface_declaration" | "enum_declaration" => {
                    let Some(simple) = Self::type_name(node, source) else {
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor).collect::<Vec<_>>().into_iter().rev()
                        {
                            stack.push(child);
                        }
                        continue;
                    };
                    let symbol_type = match node.kind() {
                        "interface_declaration" => SymbolType::Interface,
                        "enum_declaration" => SymbolType::Enum,
                        _ => SymbolType::Class,
                    };
                    let qn = Self::qualify(package.as_deref(), &simple);
                    symbols.push(Symbol {
                        name: simple,
                        symbol_type,
                        qualified_name: Some(qn),
                        location: Self::loc(&file, node),
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
                }
                "method_declaration" | "function_definition" => {
                    let name = node
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source).ok())
                        .map(|s| s.trim().to_string())
                        .or_else(|| {
                            let mut c = node.walk();
                            node.children(&mut c).find_map(|n| {
                                if n.kind() == "identifier" {
                                    n.utf8_text(source).ok().map(|s| s.trim().to_string())
                                } else {
                                    None
                                }
                            })
                        });
                    let Some(name) = name else {
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor).collect::<Vec<_>>().into_iter().rev()
                        {
                            stack.push(child);
                        }
                        continue;
                    };
                    let enclosing = Self::enclosing_class(node, source);
                    // Groovy grammar often emits constructors as method_declaration
                    // named after the class (no return type) rather than constructor_declaration.
                    let is_ctor = enclosing.as_ref().is_some_and(|cls| cls == &name);
                    let (sym_name, qn, metadata) = if is_ctor {
                        let cls = enclosing.clone().unwrap_or_else(|| name.clone());
                        (
                            cls.clone(),
                            Self::qualify(package.as_deref(), &format!("{cls}.<init>")),
                            serde_json::json!({
                                "language": "groovy",
                                "is_constructor": true,
                            }),
                        )
                    } else {
                        let qn = if let Some(cls) = &enclosing {
                            Self::qualify(package.as_deref(), &format!("{cls}.{name}"))
                        } else {
                            Self::qualify(package.as_deref(), &name)
                        };
                        (
                            name,
                            qn,
                            serde_json::json!({ "language": "groovy" }),
                        )
                    };
                    symbols.push(Symbol {
                        name: sym_name,
                        symbol_type: SymbolType::Function,
                        qualified_name: Some(qn),
                        location: Self::loc(&file, node),
                        signature: node
                            .utf8_text(source)
                            .ok()
                            .map(|s| s.lines().next().unwrap_or("").trim().to_string()),
                        return_type: if is_ctor {
                            None
                        } else {
                            node.child_by_field_name("type")
                                .and_then(|n| n.utf8_text(source).ok())
                                .map(|s| s.trim().to_string())
                        },
                        parameters: self.extract_parameters(node, source),
                        fields: vec![],
                        modifiers: vec![],
                        documentation: None,
                        metadata,
                    });
                }
                "constructor_declaration" | "compact_constructor_declaration" => {
                    let cls = Self::enclosing_class(node, source)
                        .or_else(|| Self::type_name(node, source))
                        .unwrap_or_else(|| "Unknown".into());
                    let qn = Self::qualify(package.as_deref(), &format!("{cls}.<init>"));
                    symbols.push(Symbol {
                        name: cls,
                        symbol_type: SymbolType::Function,
                        qualified_name: Some(qn),
                        location: Self::loc(&file, node),
                        signature: node
                            .utf8_text(source)
                            .ok()
                            .map(|s| s.lines().next().unwrap_or("").trim().to_string()),
                        return_type: None,
                        parameters: self.extract_parameters(node, source),
                        fields: vec![],
                        modifiers: vec![],
                        documentation: None,
                        metadata: serde_json::json!({
                            "language": "groovy",
                            "is_constructor": true,
                        }),
                    });
                }
                "import_declaration" => {
                    let text = node.utf8_text(source).unwrap_or("").trim();
                    let imported = text
                        .trim_start_matches("import")
                        .trim()
                        .trim_end_matches(".*")
                        .trim()
                        .trim_end_matches(';')
                        .trim();
                    if !imported.is_empty() {
                        let simple = imported.rsplit('.').next().unwrap_or(imported).to_string();
                        symbols.push(Symbol {
                            name: simple,
                            symbol_type: SymbolType::Import,
                            qualified_name: Some(imported.to_string()),
                            location: Self::loc(&file, node),
                            signature: Some(text.to_string()),
                            return_type: None,
                            parameters: vec![],
                            fields: vec![],
                            modifiers: vec![],
                            documentation: None,
                            metadata: serde_json::json!({ "language": "groovy" }),
                        });
                    }
                }
                _ => {}
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor).collect::<Vec<_>>().into_iter().rev() {
                stack.push(child);
            }
        }
        Ok(symbols)
    }

    fn relations_from_tree(
        &self,
        root: Node,
        source: &[u8],
        file_path: &Path,
        symbols: &[Symbol],
    ) -> Result<Vec<Relation>> {
        let mut relations = Vec::new();
        // method_invocation is Java-shaped; walk_calls uses call_kinds
        walk_calls(
            root,
            source,
            file_path,
            symbols,
            &["method_invocation", "juxt_function_call", "object_creation_expression"],
            "groovy",
            &mut relations,
        );

        // Extends / implements from class header text (best-effort)
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if node.kind() == "class_declaration"
                && let Some(from) = Self::type_name(node, source)
            {
                    let package = Self::package_name(root, source);
                    let from_qn = Self::qualify(package.as_deref(), &from);
                    let header = node
                        .utf8_text(source)
                        .ok()
                        .map(|s| s.lines().next().unwrap_or("").to_string())
                        .unwrap_or_default();
                    if let Some(rest) = header.split_once("extends").map(|(_, r)| r) {
                        let parent = rest
                            .split(['{', ',', 'i'])
                            .next()
                            .unwrap_or("")
                            .trim();
                        if !parent.is_empty() {
                            relations.push(Relation {
                                from: from_qn.clone(),
                                to: parent.to_string(),
                                relation_type: RelationType::Extends,
                                location: Self::loc(&file_path.to_string_lossy(), node),
                                metadata: serde_json::json!({ "language": "groovy" }),
                                to_qualified_hint: None,
                                to_type_hint: None,
                            });
                        }
                    }
                    if let Some(rest) = header.split_once("implements").map(|(_, r)| r) {
                        for iface in rest.split(['{', ',']).map(str::trim).filter(|s| !s.is_empty())
                        {
                            relations.push(Relation {
                                from: from_qn.clone(),
                                to: iface.to_string(),
                                relation_type: RelationType::Implements,
                                location: Self::loc(&file_path.to_string_lossy(), node),
                                metadata: serde_json::json!({ "language": "groovy" }),
                                to_qualified_hint: None,
                                to_type_hint: None,
                            });
                        }
                    }
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor).collect::<Vec<_>>().into_iter().rev() {
                stack.push(child);
            }
        }
        Ok(relations)
    }

    fn calculate_cyclomatic(&self, node: Node) -> usize {
        let mut complexity = 1usize;
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if BRANCH_KINDS.contains(&n.kind()) {
                complexity += 1;
            }
            let mut cursor = n.walk();
            for child in n.children(&mut cursor).collect::<Vec<_>>().into_iter().rev() {
                stack.push(child);
            }
        }
        complexity
    }
}

impl Default for GroovyPlugin {
    fn default() -> Self {
        Self::new().expect("Failed to create GroovyPlugin")
    }
}

impl LanguagePlugin for GroovyPlugin {
    fn language_id(&self) -> &str {
        "groovy"
    }

    fn file_extensions(&self) -> Vec<&str> {
        // `.gradle` scripts (not `build.gradle` basename — Manifest wins in registry)
        vec!["groovy", "gradle"]
    }

    fn grammar(&self) -> Option<tree_sitter::Language> {
        Some(tree_sitter_groovy::LANGUAGE.into())
    }

    fn extract_symbols(&self, file_path: &Path, source: &[u8]) -> Result<Vec<Symbol>> {
        let tree = self.parse(file_path, source)?;
        self.symbols_from_tree(tree.root_node(), source, file_path)
    }

    fn extract_relations(
        &self,
        file_path: &Path,
        source: &[u8],
        symbols: &[Symbol],
    ) -> Result<Vec<Relation>> {
        let tree = self.parse(file_path, source)?;
        self.relations_from_tree(tree.root_node(), source, file_path, symbols)
    }

    fn extract_all(&self, file_path: &Path, source: &[u8]) -> Result<ExtractAllResult> {
        let tree = self.parse(file_path, source)?;
        let root = tree.root_node();
        let symbols = self.symbols_from_tree(root, source, file_path)?;
        let relations = self.relations_from_tree(root, source, file_path, &symbols)?;
        Ok(ExtractAllResult::from_parts(symbols, relations))
    }

    fn calculate_complexity(
        &self,
        symbol: &Symbol,
        source: &[u8],
    ) -> Result<Option<ComplexityMetrics>> {
        if symbol.symbol_type != SymbolType::Function {
            return Ok(None);
        }
        let tree = self.parse(Path::new(&symbol.location.file), source)?;
        let target_line = symbol.location.start_line.saturating_sub(1);
        let mut found = None;
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if matches!(
                node.kind(),
                "method_declaration" | "function_definition" | "constructor_declaration"
            ) && node.start_position().row == target_line
            {
                found = Some(node);
                break;
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor).collect::<Vec<_>>().into_iter().rev() {
                stack.push(child);
            }
        }
        let Some(node) = found else {
            return Ok(None);
        };
        let cyclomatic = self.calculate_cyclomatic(node);
        Ok(Some(ComplexityMetrics {
            cyclomatic,
            cognitive: cyclomatic.saturating_sub(1),
            loc: symbol
                .location
                .end_line
                .saturating_sub(symbol.location.start_line)
                + 1,
            parameters: symbol.parameters.len(),
            nesting_depth: 0,
            returns: 0,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_class_and_method() {
        let src = b"package com.example\nclass OrderService {\n  String findById(Long id) {\n    return id.toString()\n  }\n}\n";
        let plugin = GroovyPlugin::new().unwrap();
        let symbols = plugin
            .extract_symbols(Path::new("OrderService.groovy"), src)
            .unwrap();
        assert!(
            symbols.iter().any(|s| {
                s.symbol_type == SymbolType::Class
                    && s.qualified_name.as_deref() == Some("com.example.OrderService")
            }),
            "{symbols:?}"
        );
        assert!(
            symbols.iter().any(|s| s.name == "findById"),
            "{symbols:?}"
        );
    }

    #[test]
    fn extracts_same_class_calls() {
        let src = b"class OrderService {\n  def validate() {}\n  def findAll() { validate() }\n}\n";
        let plugin = GroovyPlugin::new().unwrap();
        let all = plugin
            .extract_all(Path::new("OrderService.groovy"), src)
            .unwrap();
        assert!(
            all.relations
                .iter()
                .any(|r| r.relation_type == RelationType::Calls),
            "expected Calls: {:?}",
            all.relations
        );
    }
}
