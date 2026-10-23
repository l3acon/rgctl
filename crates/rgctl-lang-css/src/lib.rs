//! CSS Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_css::LANGUAGE.into())
        .expect("compile css tags.scm");
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
        let issues = check_tags_scm("css", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_css::LANGUAGE.into()).expect("compile");
        let source = br###"@import "base.css";
.foo { color: red; }
@keyframes spin { from { opacity: 0; } }
"###;
        let result = plugin
            .extract_all(Path::new("app.css"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name == "spin"),
            "expected keyframes 'spin', got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == ".foo"),
            "expected selector '.foo', got {:?}",
            result.symbols
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "base.css" && s.symbol_type == rgctl_plugin_api::SymbolType::Import),
            "expected import 'base.css', got {:?}",
            result.symbols
        );
    }
}
