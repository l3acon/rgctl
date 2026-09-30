//! Puppet language plugin for rgctl (Tier 1).
//!
//! Honesty limits: no catalog compiler / modulepath resolution, no ERB/Hiera/facts
//! translation. See `docs/puppet-extract-honesty.md`.

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[cfg(test)]
mod ast_coverage;
mod plugin;
pub use plugin::PuppetPlugin;

/// Register the Puppet language plugin.
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(
        PuppetPlugin::new().expect("init PuppetPlugin"),
    ));
}
