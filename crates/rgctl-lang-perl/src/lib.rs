//! Perl Tier-2 language plugin (`handler = "queries"`).
//!
//! Uses `ts-parser-perl` because `tree-sitter-perl` 1.1 depends on
//! `tree-sitter ^0.26` and conflicts with the workspace `tree-sitter = "0.25"` pin.

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    if let Ok(plugin) =
        QueryLanguagePlugin::from_config(&CONFIG, || ts_parser_perl::LANGUAGE.into())
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
        let issues = check_tags_scm("perl", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin = QueryLanguagePlugin::from_config(&CONFIG, || ts_parser_perl::LANGUAGE.into())
            .expect("compile");
        let source = br###"
package Math;
use strict;
sub add {
  my ($a, $b) = @_;
  return helper($a, $b);
}
1;
"###;
        let result = plugin
            .extract_all(Path::new("Math.pm"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name == "Math"),
            "expected Math package symbol, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == "add"),
            "expected add subroutine symbol, got {:?}",
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
