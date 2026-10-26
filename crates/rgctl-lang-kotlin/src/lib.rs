//! Kotlin language plugin for rgctl (Tier 1).
//!
//! Honesty limits: no reflection/reified generics, best-effort call targets,
//! suspend/coroutine interprocedural CFG not modeled. See `docs/kotlin-extract-honesty.md`.

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[allow(dead_code)]
mod ast_coverage;
mod plugin;
mod single_pass;
pub use plugin::KotlinPlugin;

/// Register the Kotlin language plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(KotlinPlugin::new().expect("init KotlinPlugin")));
}
