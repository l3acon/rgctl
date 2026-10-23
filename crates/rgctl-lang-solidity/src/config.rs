//! Static Tier-2 config for Solidity (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// Static `LanguageConfig` for `QueryLanguagePlugin`.
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "solidity",
    extensions: &["sol"],
    function_kinds: &["function_definition"],
    class_kinds: &["contract_declaration"],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
