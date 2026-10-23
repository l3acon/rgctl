//! F# Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_fsharp::LANGUAGE_FSHARP.into())
        .expect("compile fsharp tags.scm");
    registry.register_language_plugin(Arc::new(plugin));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgctl_ast_coverage::check_tags_scm;
    use rgctl_plugin_api::LanguagePlugin;
    use std::path::Path;

    #[test]
    fn tags_scm_passes_ast_coverage_vocabulary_gate() {
        let issues = check_tags_scm("fsharp", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_fsharp::LANGUAGE_FSHARP.into()).expect("compile");
        let source = br###"module Math
open System

type Point = { X: int; Y: int }

let add a b =
    helper a + b
"###;
        let result = plugin
            .extract_all(Path::new("Math.fs"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name == "add"),
            "expected symbol 'add', got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == "Point"),
            "expected type 'Point', got {:?}",
            result.symbols
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "System" && s.symbol_type == rgctl_plugin_api::SymbolType::Import),
            "expected import 'System', got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "add" && r.to == "helper"),
            "expected call add -> helper, got {:?}",
            result.relations
        );
        assert!(
            !result
                .relations
                .iter()
                .any(|r| r.from == "add" && r.to == "add"),
            "must not emit self-call add -> add: {:?}",
            result.relations
        );
    }
}
