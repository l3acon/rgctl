//! Static Tier-2 config for css (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// CSS language configuration for [`rgctl_lang_runtime::QueryLanguagePlugin`].
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "css",
    extensions: &["css"],
    function_kinds: &["keyframes_statement"],
    class_kinds: &["rule_set"],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
