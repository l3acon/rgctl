//! HTML Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_html::LANGUAGE.into())
        .expect("compile html tags.scm");
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
        let issues = check_tags_scm("html", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_html::LANGUAGE.into()).expect("compile");
        let source = br###"<html><body><script>void 0</script><div id="x"></div></body></html>"###;
        let result = plugin
            .extract_all(Path::new("index.html"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name.contains("script")),
            "expected symbol containing 'script', got {:?}",
            result.symbols
        );
    }
}
