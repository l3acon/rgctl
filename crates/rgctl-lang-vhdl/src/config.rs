//! Static Tier-2 config for vhdl (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// VHDL language configuration for [`rgctl_lang_runtime::QueryLanguagePlugin`].
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "vhdl",
    extensions: &["vhd", "vhdl"],
    function_kinds: &["subprogram_definition"],
    class_kinds: &["component_declaration"],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
