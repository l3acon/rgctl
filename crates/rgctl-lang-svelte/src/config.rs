//! Static Tier-2 config for svelte (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// Svelte language configuration for [`rgctl_lang_runtime::QueryLanguagePlugin`].
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "svelte",
    extensions: &["svelte"],
    function_kinds: &["script_element"],
    class_kinds: &["element"],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
