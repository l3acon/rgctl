//! Language plugin crate for rgctl

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[cfg(test)]
mod ast_coverage;
mod plugin;
mod single_pass;
pub use plugin::JavaPlugin;

/// Register this language plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(JavaPlugin::new().expect("init JavaPlugin")));
}
