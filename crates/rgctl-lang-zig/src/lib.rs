//! Zig Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register the Zig query plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_zig::LANGUAGE.into())
        .expect("compile Zig tags.scm");
    registry.register_language_plugin(Arc::new(plugin));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgctl_ast_coverage::check_tags_scm;
    use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
    use std::path::Path;

    #[test]
    fn tags_scm_passes_ast_coverage_vocabulary_gate() {
        let issues = check_tags_scm("zig", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_functions_calls_and_imports() {
        let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_zig::LANGUAGE.into())
            .expect("compile");
        let source = br#"
const std = @import("std");
pub fn helper(x: i32) i32 { return x + 1; }
pub fn add(a: i32, b: i32) i32 {
    const h = helper(a);
    return h + b;
}
"#;
        let result = plugin
            .extract_all(Path::new("math.zig"), source)
            .expect("extract_all");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "add" && s.symbol_type == SymbolType::Function)
        );
        assert!(result.symbols.iter().any(|s| s.name == "helper"));
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.symbol_type == SymbolType::Import && s.name == "std"),
            "expected Import std, got {:?}",
            result.symbols
        );
        assert!(
            result.relations.iter().any(|r| {
                r.relation_type == RelationType::Calls && r.to == "helper" && r.from == "add"
            }),
            "expected Calls add->helper, got {:?}",
            result.relations
        );
    }
}
