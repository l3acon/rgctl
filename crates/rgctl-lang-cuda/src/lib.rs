//! CUDA Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_cuda::LANGUAGE.into())
        .expect("compile cuda tags.scm");
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
        let issues = check_tags_scm("cuda", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_cuda::LANGUAGE.into())
                .expect("compile");
        let source = br###"__global__ void kernel(int *a) {
  helper(a);
}
"###;
        let result = plugin
            .extract_all(Path::new("kernel.cu"), source)
            .expect("extract_all");
        assert!(
            result.symbols.iter().any(|s| s.name.contains("kernel")),
            "expected symbol containing 'kernel', got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "kernel" && r.to == "helper"),
            "expected call kernel -> helper, got relations={:?} symbols={:?}",
            result.relations,
            result.symbols
        );
        assert!(
            !result
                .relations
                .iter()
                .any(|r| r.from == "kernel" && r.to == "kernel"),
            "must not emit self-call kernel -> kernel: {:?}",
            result.relations
        );
    }
}
