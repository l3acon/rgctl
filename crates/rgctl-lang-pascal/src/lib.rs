//! Pascal / Delphi Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    if let Ok(plugin) =
        QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_pascal::LANGUAGE.into())
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
        let issues = check_tags_scm("pascal", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_pascal::LANGUAGE.into())
                .expect("compile");
        let source = br###"
unit Math;
interface
uses SysUtils;
type
  TFoo = class
    procedure Bar;
  end;
implementation
procedure TFoo.Bar;
begin
  Helper();
end;
procedure Standalone;
begin
  SubCall();
end;
end.
"###;
        let result = plugin
            .extract_all(Path::new("Math.pas"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name == "Math"),
            "expected Math unit symbol, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == "TFoo"),
            "expected TFoo class symbol, got {:?}",
            result.symbols
        );
        assert!(
            result.symbols.iter().any(|s| s.name == "Bar"),
            "expected Bar procedure/method symbol, got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| (r.from == "Math::Bar" || r.from == "Bar") && r.to == "Helper"),
            "expected Math::Bar -> Helper call relation, got {:?}",
            result.relations
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "Standalone" && r.to == "SubCall"),
            "expected Standalone -> SubCall call relation, got {:?}",
            result.relations
        );
    }
}
