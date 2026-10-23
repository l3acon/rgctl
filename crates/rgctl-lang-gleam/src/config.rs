//! Static Tier-2 config for gleam (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// Gleam language configuration for [`rgctl_lang_runtime::QueryLanguagePlugin`].
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "gleam",
    extensions: &["gleam"],
    function_kinds: &["function"],
    class_kinds: &["type_definition"],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
