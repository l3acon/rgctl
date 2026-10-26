//! Protocol Buffers Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_proto::LANGUAGE.into())
        .expect("compile Proto tags.scm");
    registry.register_language_plugin(Arc::new(plugin));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgctl_ast_coverage::check_tags_scm;
    use rgctl_plugin_api::{LanguagePlugin, SymbolType};
    use std::path::Path;

    #[test]
    fn tags_scm_passes_ast_coverage_vocabulary_gate() {
        assert!(check_tags_scm("proto", TAGS_SCM).is_empty());
    }

    #[test]
    fn extract_messages_services_rpcs() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_proto::LANGUAGE.into())
                .expect("compile");
        let source = br#"
syntax = "proto3";
message Foo { string name = 1; }
service Bar { rpc Baz (Foo) returns (Foo); }
"#;
        let result = plugin
            .extract_all(Path::new("api.proto"), source)
            .expect("extract_all");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "Foo" && s.symbol_type == SymbolType::Class)
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "Bar" && s.symbol_type == SymbolType::Interface)
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "Baz" && s.symbol_type == SymbolType::Function)
        );
    }
}
