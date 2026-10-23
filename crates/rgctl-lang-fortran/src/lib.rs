//! Fortran Tier-2 language plugin (`handler = "queries"`).

mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

/// Register this Tier-2 query plugin.
pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_fortran::LANGUAGE.into())
        .expect("compile fortran tags.scm");
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
        let issues = check_tags_scm("fortran", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_fortran::LANGUAGE.into()).expect("compile");
        let source = br###"module math
  use iso_c_binding
contains
  function add(a, b) result(c)
    integer :: a, b, c
    call log_op(a)
    c = helper(a, b)
  end function
end module
"###;
        let result = plugin
            .extract_all(Path::new("math.f90"), source)
            .expect("extract_all");
        assert!(
            result
                .symbols
                .iter()
                .any(|s| s.name == "add" && s.qualified_name.as_deref() == Some("math::add")),
            "expected math::add with full module/function spans, got {:?}",
            result.symbols
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "add" && r.to == "helper"),
            "expected add -> helper function call relation, got {:?}",
            result.relations
        );
        assert!(
            result
                .relations
                .iter()
                .any(|r| r.from == "add" && r.to == "log_op"),
            "expected add -> log_op subroutine call relation, got {:?}",
            result.relations
        );
    }
}
