//! GLSL Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_glsl::LANGUAGE_GLSL.into())
        .expect("compile glsl tags.scm");
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
        let issues = check_tags_scm("glsl", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_glsl::LANGUAGE_GLSL.into()).expect("compile");
        let source = br###"void main() {
  helper();
}
"###;
        let result = plugin
            .extract_all(Path::new("main.vert"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name.contains("main")),
            "expected symbol containing 'main', got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "main" && r.to == "helper"),
            "expected call main -> helper, got relations={:?} symbols={:?}",
            result.relations,
            result.symbols
        );
        assert!(
            !result.relations.iter().any(|r| r.from == "main" && r.to == "main"),
            "must not emit self-call main -> main: {:?}",
            result.relations
        );

    }
}
