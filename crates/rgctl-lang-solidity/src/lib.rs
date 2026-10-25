//! Solidity Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin =
        QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_solidity::LANGUAGE.into())
            .expect("compile Solidity tags.scm");
    registry.register_language_plugin(Arc::new(plugin));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgctl_ast_coverage::check_tags_scm;
    use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
    use std::path::Path;

    #[test]
    fn tags_scm_passes_ast_coverage_vocabulary_gate() {
        assert!(check_tags_scm("solidity", TAGS_SCM).is_empty());
    }

    #[test]
    fn extract_contract_functions_and_calls() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_solidity::LANGUAGE.into())
                .expect("compile");
        let source = br#"
pragma solidity ^0.8.0;
contract C {
    function f(uint x) public returns (uint) { return x; }
    function g(uint y) public returns (uint) { return f(y); }
}
"#;
        let result = plugin
            .extract_all(Path::new("C.sol"), source)
            .expect("extract_all");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "C" && s.symbol_type == SymbolType::Class)
        );
        let g_sym = result
            .symbols
            .iter()
            .find(|s| s.name == "g" && s.symbol_type == SymbolType::Function)
            .expect("function g");
        assert_eq!(g_sym.qualified_name.as_deref(), Some("C::g"));
        assert!(
            result.relations.iter().any(|r| {
                r.relation_type == RelationType::Calls && r.to == "f" && r.from == "g"
            }),
            "{:?}",
            result.relations
        );
    }
}
