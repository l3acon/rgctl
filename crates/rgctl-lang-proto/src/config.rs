//! Static Tier-2 config for Protocol Buffers (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// Static `LanguageConfig` for `QueryLanguagePlugin`.
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "proto",
    extensions: &["proto"],
    function_kinds: &["rpc"],
    class_kinds: &["message"],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
