//! Generic language plugins with per-crate static configuration.
//!
//! - [`TreeSitterLanguagePlugin`] — kind-walk Tier-2 / transitional plugins
//! - [`QueryLanguagePlugin`] — declarative `tags.scm` (`handler = "queries"`)
//! - [`RegexLanguagePlugin`] — regex-only fallback

pub mod config;
pub mod queries;
pub mod query_plugin;
pub mod regex_extract;
pub mod regex_plugin;
pub mod tree_sitter_plugin;

pub use config::{LanguageConfig, RegexPatternConfig};
pub use queries::{
    CaptureIndex, CaptureRole, capture_role, compile_tags_query, extract_capture_names,
    validate_tags_captures,
};
pub use query_plugin::QueryLanguagePlugin;
pub use regex_plugin::RegexLanguagePlugin;
pub use rgctl_ast_coverage::ALLOWED_TAG_CAPTURES;
pub use tree_sitter_plugin::TreeSitterLanguagePlugin;

/// Create a tree-sitter kind-walk plugin from a static config and grammar loader.
pub fn tree_sitter_plugin(
    config: &'static LanguageConfig,
    grammar: fn() -> tree_sitter::Language,
) -> TreeSitterLanguagePlugin {
    TreeSitterLanguagePlugin::from_config(config, grammar)
}

/// Create a `tags.scm` query plugin (`handler = "queries"`).
pub fn query_plugin(
    config: &'static LanguageConfig,
    grammar: fn() -> tree_sitter::Language,
) -> rgctl_plugin_api::Result<QueryLanguagePlugin> {
    QueryLanguagePlugin::from_config(config, grammar)
}

/// Create a regex plugin from a static config.
pub fn regex_plugin(config: &'static LanguageConfig) -> RegexLanguagePlugin {
    RegexLanguagePlugin::from_config(config)
}
