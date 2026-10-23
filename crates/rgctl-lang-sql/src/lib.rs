//! Language plugin crate for rgctl

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[allow(dead_code)]
mod ast_coverage;
mod plugin;
mod single_pass;
pub use plugin::Plugin;

/// Register this language plugin.
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(Plugin::new().expect("init Plugin")));
}
