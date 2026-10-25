//! Julia Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_julia::LANGUAGE.into())
        .expect("compile julia tags.scm");
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
        let issues = check_tags_scm("julia", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_julia::LANGUAGE.into())
                .expect("compile");
        let source = br###"module M
using LinearAlgebra

struct Point
    x
    y
end

function add(a, b)
    return helper(a, b)
end
end
"###;
        let result = plugin
            .extract_all(Path::new("math.jl"), source)
            .expect("extract_all");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "add" && s.qualified_name.as_deref() == Some("M::add")),
            "expected M::add function symbol, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == "Point"),
            "expected Point struct symbol, got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "add" && r.to == "helper"),
            "expected add -> helper call relation, got {:?}",
            result.relations
        );
        assert!(
            !result
                .relations
                .iter()
                .any(|r| r.from == "add" && r.to == "add"),
            "signature call_expression must not emit self-call add -> add: {:?}",
            result.relations
        );
    }
}
