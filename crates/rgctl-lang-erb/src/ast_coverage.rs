//! AST coverage manifest vs pinned `tree-sitter-embedded-template` grammar.

use std::collections::{HashMap, HashSet};

const MANIFEST_JSON: &str = include_str!("../erb-ast-coverage.json");

const ALLOWED: &[&str] = &[
    "Symbol",
    "Relation",
    "CfgStatement",
    "AstSkeleton",
    "Skip",
    "Literal",
];

pub fn load_manifest() -> HashMap<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(MANIFEST_JSON).expect("erb-ast-coverage.json parse");
    let grammar = v["grammar"].as_str().unwrap_or("");
    assert!(
        grammar.starts_with("tree-sitter-embedded-template@"),
        "grammar prefix: {grammar}"
    );
    let handlers = v["handlers"].as_object().expect("handlers object");
    handlers
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("Skip").to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erb_ast_coverage_manifest_matches_grammar() {
        let manifest = load_manifest();
        assert!(!manifest.is_empty(), "manifest is empty");

        let language: tree_sitter::Language = tree_sitter_embedded_template::LANGUAGE.into();
        let node_types_json = tree_sitter_embedded_template::NODE_TYPES;
        let types: Vec<serde_json::Value> =
            serde_json::from_str(node_types_json).expect("parse NODE_TYPES");

        let named: HashSet<String> = types
            .iter()
            .filter(|t| t["named"].as_bool() == Some(true))
            .filter_map(|t| t["type"].as_str().map(String::from))
            .collect();

        let allowed: HashSet<&str> = ALLOWED.iter().copied().collect();

        for (kind, handler) in &manifest {
            assert!(
                allowed.contains(handler.as_str()),
                "handler {handler:?} for {kind:?} not in ALLOWED"
            );
        }

        let _ = language;
        let _ = named;
    }
}
