//! Static Tier-2 config for pascal (`handler = "queries"`).

use rgctl_lang_runtime::LanguageConfig;

/// Embedded `queries/tags.scm`.
pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

/// Pascal / Delphi language configuration for [`rgctl_lang_runtime::QueryLanguagePlugin`].
pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "pascal",
    // Omit `.pp` — that extension is Puppet manifests in this repo; Free Pascal
    // units use `.pas` / `.lpr`. Force Free Pascal `.pp` with `-l pascal` if needed.
    extensions: &["pas", "dpr", "dpk", "lpr"],
    function_kinds: &["declProc", "defProc"],
    class_kinds: &["declClass", "declIntf"],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
