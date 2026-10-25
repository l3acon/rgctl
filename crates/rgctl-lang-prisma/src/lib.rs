//! Prisma Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_prisma::LANGUAGE.into())
        .expect("compile prisma tags.scm");
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
        let issues = check_tags_scm("prisma", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_prisma::LANGUAGE.into())
                .expect("compile");
        let source = br###"datasource db { provider = "sqlite" }
model User { id Int @id }
enum Role { ADMIN }
"###;
        let result = plugin
            .extract_all(Path::new("schema.prisma"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name.contains("User")),
            "expected symbol containing 'User', got {:?}",
            result.symbols
        );
    }
}
