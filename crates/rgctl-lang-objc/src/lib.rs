//! Objective-C Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_objc::LANGUAGE.into())
        .expect("compile objc tags.scm");
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
        let issues = check_tags_scm("objc", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_objc::LANGUAGE.into()).expect("compile");
        let source = br###"@interface Foo : NSObject
- (void)bar;
@end
@implementation Foo
- (void)bar {
    [self helper];
}
@end
"###;
        let result = plugin
            .extract_all(Path::new("Foo.m"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name == "Foo"),
            "expected Foo class symbol, got {:?}",
            result.symbols
        );
        assert!(
            !result.symbols.iter().any(|s| s.name == "NSObject"),
            "superclass NSObject must not be extracted as a defined class: {:?}",
            result.symbols
        );
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "bar" && s.qualified_name.as_deref() == Some("Foo::bar")),
            "expected Foo::bar method symbol, got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "bar" && r.to == "helper"),
            "expected bar -> helper message send relation, got {:?}",
            result.relations
        );
    }
}
