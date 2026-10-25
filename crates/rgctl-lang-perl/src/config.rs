//! Static Tier-2 config for perl (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// Perl language configuration for [`rgctl_lang_runtime::QueryLanguagePlugin`].
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "perl",
    extensions: &["pl", "pm", "t"],
    function_kinds: &[
        "subroutine_declaration_statement",
        "method_declaration_statement",
    ],
    class_kinds: &["package_statement"],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
