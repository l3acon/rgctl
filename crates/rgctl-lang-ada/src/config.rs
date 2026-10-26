//! Static Tier-2 config for ada (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// Ada language configuration for [`rgctl_lang_runtime::QueryLanguagePlugin`].
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "ada",
    extensions: &["adb", "ads", "ada"],
    function_kinds: &["subprogram_body", "subprogram_declaration"],
    class_kinds: &[
        "full_type_declaration",
        "task_type_declaration",
        "protected_type_declaration",
    ],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
