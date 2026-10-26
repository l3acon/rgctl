//! Odin Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_odin::LANGUAGE.into())
        .expect("compile odin tags.scm");
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
        let issues = check_tags_scm("odin", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_odin::LANGUAGE.into())
                .expect("compile");
        let source = br###"package math

add :: proc(a, b: int) -> int {
    return helper(a) + b
}
"###;
        let result = plugin
            .extract_all(Path::new("math.odin"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name.contains("add")),
            "expected symbol containing 'add', got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "add" && r.to == "helper"),
            "expected call add -> helper, got relations={:?} symbols={:?}",
            result.relations,
            result.symbols
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
