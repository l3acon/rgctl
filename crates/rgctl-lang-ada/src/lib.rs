//! Ada Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    if let Ok(plugin) =
        QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_ada::LANGUAGE.into())
    {
        registry.register_language_plugin(Arc::new(plugin));
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use rgctl_ast_coverage::check_tags_scm;
    use rgctl_plugin_api::LanguagePlugin;
    use std::path::Path;

    #[test]
    fn tags_scm_passes_ast_coverage_vocabulary_gate() {
        let issues = check_tags_scm("ada", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_ada::LANGUAGE.into())
            .expect("compile");
        let source = br###"
with Ada.Text_IO;
package body Math is
  function Add (A, B : Integer) return Integer is
  begin
    return Helper(A, B);
  end Add;
end Math;
"###;
        let result = plugin
            .extract_all(Path::new("math.adb"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name == "Math"),
            "expected Math package symbol, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == "Add"),
            "expected Add function symbol, got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "Add" && r.to == "Helper"),
            "expected Add -> Helper call relation, got {:?}",
            result.relations
        );
    }
}
