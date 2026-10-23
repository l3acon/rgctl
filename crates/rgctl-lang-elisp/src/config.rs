//! Static Tier-2 config for elisp (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// Emacs Lisp language configuration for [`rgctl_lang_runtime::QueryLanguagePlugin`].
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "elisp",
    extensions: &["el"],
    function_kinds: &["function_definition"],
    class_kinds: &[],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
