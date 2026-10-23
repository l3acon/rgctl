//! Language plugin crate for rgctl

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[cfg(test)]
mod ast_coverage;
mod plugin;
mod single_pass;
pub use plugin::SwiftPlugin;

/// Register this language plugin.
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(SwiftPlugin::new().expect("init SwiftPlugin")));
}
