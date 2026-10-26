//! VHDL Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_vhdl::LANGUAGE.into())
        .expect("compile vhdl tags.scm");
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
        let issues = check_tags_scm("vhdl", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_vhdl::LANGUAGE.into())
                .expect("compile");
        let source = br###"entity adder is
end entity;
architecture rtl of adder is
  procedure step is
  begin
    helper;
  end procedure;
begin
end architecture;
"###;
        let result = plugin
            .extract_all(Path::new("adder.vhd"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name == "adder"),
            "expected symbol 'adder', got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == "step"),
            "expected procedure 'step', got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "step" && r.to == "helper"),
            "expected call step -> helper, got {:?}",
            result.relations
        );
    }
}
