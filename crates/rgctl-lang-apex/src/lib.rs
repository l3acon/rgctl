//! Salesforce Apex Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    if let Ok(plugin) =
        QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_sfapex::apex::LANGUAGE.into())
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
        let issues = check_tags_scm("apex", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_sfapex::apex::LANGUAGE.into())
                .expect("compile");
        let source = br###"
public class Foo extends Bar {
  public Integer add(Integer a) {
    return helper(a);
  }
}
"###;
        let result = plugin
            .extract_all(Path::new("Foo.cls"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name == "Foo"),
            "expected Foo class symbol, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == "add"),
            "expected add method symbol, got {:?}",
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
    }
}
