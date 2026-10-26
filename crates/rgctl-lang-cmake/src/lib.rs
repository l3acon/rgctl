//! CMake Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_cmake::LANGUAGE.into())
        .expect("compile cmake tags.scm");
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
        let issues = check_tags_scm("cmake", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_cmake::LANGUAGE.into())
                .expect("compile");
        let source = br###"function(add_numbers a b)
  message(STATUS "hi")
endfunction()
"###;
        let result = plugin
            .extract_all(Path::new("CMakeLists.cmake"), source)
            .expect("extract_all");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name.contains("add_numbers")),
            "expected symbol containing 'add_numbers', got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "add_numbers" && r.to == "message"),
            "expected call add_numbers -> message, got relations={:?} symbols={:?}",
            result.relations,
            result.symbols
        );
        assert!(
            !result
                .relations
                .iter()
                .any(|r| r.from == "add_numbers" && r.to == "add_numbers"),
            "must not emit self-call add_numbers -> add_numbers: {:?}",
            result.relations
        );
    }
}
