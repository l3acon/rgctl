//! ERB (Embedded Ruby) template language plugin for rgctl.
//!
//! Extracts variable references, fact lookups, scope accesses, and control flow
//! from `.erb` templates. Designed primarily for Puppet ERB templates but works
//! with any ERB file.
//!
//! Each ERB block (`<%= %>`, `<% %>`, `<%# %>`) is analyzed to extract:
//! - `@variable` references → `PuppetVariable` symbols
//! - `@facts[...]` references → `PuppetFact` symbols with `UsesFact` edges
//! - `scope['class::param']` → `References` edges to Puppet classes
//! - Control flow constructs (`if`, `each`, `unless`) for complexity metrics
//!
//! Block metadata includes a `translation_tier` (1–4) and, for tiers 1–3,
//! a `jinja2_pattern` hint for downstream ERB→Jinja2 translation.

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[cfg(test)]
mod ast_coverage;
mod plugin;
pub use plugin::ErbPlugin;

/// Register the ERB language plugin.
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(ErbPlugin::new().expect("init ErbPlugin")));
}
