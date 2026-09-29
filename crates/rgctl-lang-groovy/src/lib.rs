//! Groovy language plugin for rgctl (Tier 1).
//!
//! Honesty: [`docs/groovy-extract-honesty.md`](../../../docs/groovy-extract-honesty.md).

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[cfg(test)]
mod ast_coverage;
mod plugin;
pub use plugin::GroovyPlugin;

/// Register the Groovy language plugin.
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(
        GroovyPlugin::new().expect("init GroovyPlugin"),
    ));
}
