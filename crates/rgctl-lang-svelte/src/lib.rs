//! Svelte Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_svelte_ng::LANGUAGE.into())
        .expect("compile svelte tags.scm");
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
        let issues = check_tags_scm("svelte", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_svelte_ng::LANGUAGE.into()).expect("compile");
        let source = br###"<script>export let name = "world";</script>
<h1>Hello {name}</h1>
"###;
        let result = plugin
            .extract_all(Path::new("App.svelte"), source)
            .expect("extract_all");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "script" && s.symbol_type == rgctl_plugin_api::SymbolType::Function),
            "expected Function symbol 'script', got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == "h1"),
            "expected symbol 'h1', got {:?}",
            result.symbols
        );
    }
}
