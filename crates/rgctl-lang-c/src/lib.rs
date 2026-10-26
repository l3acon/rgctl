//! Language plugin crate for rgctl — C Tier 1 extraction.
//!
//! ## Honesty limits
//!
//! - **Macros:** `#define` / function-like macros are **not** indexed as graph
//!   `Macro` nodes (kernel-scale `#define` counts explode node count); helpers in
//!   `depth` remain for opt-in tooling. Macro-expanded call sites are not rewritten —
//!   only tree-sitter-visible `call_expression` nodes produce `Calls` edges.
//! - **Function pointers:** indirect calls through variables/parameters are emitted with
//!   `metadata.unresolved` when the callee cannot be resolved statically.
//! - **Header / `.c` pairs:** symbols in `foo.h` and `foo.c` share the same
//!   `{file_stem}::{name}` qualified name; use `file_path` to disambiguate.
//! - **No native constructors:** C has no `.<init>` symbols (unlike C++ / Java).

use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

#[cfg(test)]
mod ast_coverage;
mod depth;
mod plugin;
mod single_pass;
pub use plugin::CPlugin;

/// Register this language plugin.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn register(registry: &mut LanguageRegistry) {
    registry.register_language_plugin(Arc::new(CPlugin::new().expect("init CPlugin")));
}
