//! Groovy language plugin for rgctl (Tier 1).
//!
//! Honesty: [`docs/groovy-extract-honesty.md`](../../../docs/groovy-extract-honesty.md).

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[cfg(test)]
mod ast_coverage;
mod plugin;
mod single_pass;
pub use plugin::GroovyPlugin;

/// Register the Groovy language plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(GroovyPlugin::new().expect("init GroovyPlugin")));
}
