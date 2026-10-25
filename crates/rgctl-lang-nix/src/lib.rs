//! Nix Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_nix::LANGUAGE.into())
        .expect("compile Nix tags.scm");
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
        assert!(check_tags_scm("nix", TAGS_SCM).is_empty());
    }

    #[test]
    fn extract_nix_functions_and_applies() {
        let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_nix::LANGUAGE.into())
            .expect("compile");
        let source = b"let foo = x: x + 1; in foo 2\n";
        let result = plugin
            .extract_all(Path::new("default.nix"), source)
            .expect("extract_all");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "foo" && s.symbol_type == SymbolType::Function),
            "{:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| { r.relation_type == RelationType::Calls && r.to == "foo" }),
            "{:?}",
            result.relations
        );
    }
}
