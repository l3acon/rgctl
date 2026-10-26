//! Language plugin crate for rgctl

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[cfg(test)]
mod ast_coverage;
mod extract_depth;
mod plugin;
mod single_pass;
pub use plugin::RustPlugin;

/// Register this language plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(RustPlugin::new().expect("init RustPlugin")));
}
