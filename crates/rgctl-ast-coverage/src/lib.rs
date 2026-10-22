//! Compare `{lang}-ast-coverage.json` manifests to the live tree-sitter grammar.
//!
//! Used by `rgctl-languages` `build.rs` so `cargo check` / `cargo build` can
//! **warn** when a grammar bump introduces new named kinds (or removes old ones)
//! before unit tests are run. Set `RGCTL_AST_COVERAGE_STRICT=1` to fail the build.
//!
//! [`check_plugin_manifest_sync`] additionally verifies bi-directional agreement
//! between a plugin's [`AstHandlerTable`] and the coverage JSON (when plugins
//! expose handlers via `LanguagePlugin::declared_ast_handlers`).

mod tracker;

pub use tracker::{
    clear_hit_callback, install_hit_callback, AstCoverageTracker,
};

use rgctl_plugin_api::{AstHandlerTable, HandlerMask};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Allowed handler labels in coverage manifests.
pub const ALLOWED_HANDLERS: &[&str] = &[
    "Symbol",
    "Relation",
    "CfgStatement",
    "AstSkeleton",
    "Skip",
    "Literal",
];

/// One bundled language to validate.
pub struct CoverageSpec {
    /// Language id (`java`, `kotlin`, …).
    pub id: &'static str,
    /// Crate directory name under `crates/` (`rgctl-lang-java`).
    pub crate_dir: &'static str,
    /// Manifest filename inside that crate.
    pub manifest_file: &'static str,
    /// Expected `grammar` field prefix (`tree-sitter-java@`).
    pub grammar_prefix: &'static str,
    /// Live grammar.
    pub language: fn() -> tree_sitter::Language,
}

/// All Tier 1 (+ markdown) coverage specs shipped in-tree.
pub fn bundled_specs() -> &'static [CoverageSpec] {
    &[
        CoverageSpec {
            id: "c",
            crate_dir: "rgctl-lang-c",
            manifest_file: "c-ast-coverage.json",
            grammar_prefix: "tree-sitter-c@",
            language: || tree_sitter_c::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "cpp",
            crate_dir: "rgctl-lang-cpp",
            manifest_file: "cpp-ast-coverage.json",
            grammar_prefix: "tree-sitter-cpp@",
            language: || tree_sitter_cpp::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "csharp",
            crate_dir: "rgctl-lang-csharp",
            manifest_file: "csharp-ast-coverage.json",
            grammar_prefix: "tree-sitter-c-sharp@",
            language: || tree_sitter_c_sharp::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "go",
            crate_dir: "rgctl-lang-go",
            manifest_file: "go-ast-coverage.json",
            grammar_prefix: "tree-sitter-go@",
            language: || tree_sitter_go::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "groovy",
            crate_dir: "rgctl-lang-groovy",
            manifest_file: "groovy-ast-coverage.json",
            grammar_prefix: "tree-sitter-groovy@",
            language: || tree_sitter_groovy::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "java",
            crate_dir: "rgctl-lang-java",
            manifest_file: "java-ast-coverage.json",
            grammar_prefix: "tree-sitter-java@",
            language: || tree_sitter_java::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "javascript",
            crate_dir: "rgctl-lang-javascript",
            manifest_file: "javascript-ast-coverage.json",
            grammar_prefix: "tree-sitter-javascript@",
            language: || tree_sitter_javascript::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "kotlin",
            crate_dir: "rgctl-lang-kotlin",
            manifest_file: "kotlin-ast-coverage.json",
            grammar_prefix: "tree-sitter-kotlin-ng@",
            language: || tree_sitter_kotlin_ng::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "markdown",
            crate_dir: "rgctl-lang-markdown",
            manifest_file: "markdown-ast-coverage.json",
            grammar_prefix: "tree-sitter-md@",
            language: || tree_sitter_md::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "php",
            crate_dir: "rgctl-lang-php",
            manifest_file: "php-ast-coverage.json",
            grammar_prefix: "tree-sitter-php@",
            language: || tree_sitter_php::LANGUAGE_PHP.into(),
        },
        CoverageSpec {
            id: "puppet",
            crate_dir: "rgctl-lang-puppet",
            manifest_file: "puppet-ast-coverage.json",
            grammar_prefix: "tree-sitter-puppet@",
            language: || tree_sitter_puppet::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "erb",
            crate_dir: "rgctl-lang-erb",
            manifest_file: "erb-ast-coverage.json",
            grammar_prefix: "tree-sitter-embedded-template@",
            language: || tree_sitter_embedded_template::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "python",
            crate_dir: "rgctl-lang-python",
            manifest_file: "python-ast-coverage.json",
            grammar_prefix: "tree-sitter-python@",
            language: || tree_sitter_python::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "ruby",
            crate_dir: "rgctl-lang-ruby",
            manifest_file: "ruby-ast-coverage.json",
            grammar_prefix: "tree-sitter-ruby@",
            language: || tree_sitter_ruby::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "rust",
            crate_dir: "rgctl-lang-rust",
            manifest_file: "rust-ast-coverage.json",
            grammar_prefix: "tree-sitter-rust@",
            language: || tree_sitter_rust::LANGUAGE.into(),
        },
        CoverageSpec {
            id: "typescript",
            crate_dir: "rgctl-lang-typescript",
            manifest_file: "typescript-ast-coverage.json",
            grammar_prefix: "tree-sitter-typescript@",
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        },
    ]
}

/// Named kinds from a live grammar.
pub fn grammar_named_kinds(lang: &tree_sitter::Language) -> HashSet<String> {
    let mut set = HashSet::new();
    for i in 0..lang.node_kind_count() {
        if lang.node_kind_is_named(i as u16)
            && let Some(k) = lang.node_kind_for_id(i as u16)
        {
            set.insert(k.to_string());
        }
    }
    set
}

/// Drift / validity issues for one manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageIssue {
    /// Language id.
    pub language: String,
    /// Human-readable problem.
    pub message: String,
}

/// Validate one JSON manifest against a live grammar.
pub fn check_manifest(
    language_id: &str,
    json: &str,
    grammar_prefix: &str,
    lang: &tree_sitter::Language,
) -> Vec<CoverageIssue> {
    let mut issues = Vec::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        issues.push(CoverageIssue {
            language: language_id.into(),
            message: "manifest JSON failed to parse".into(),
        });
        return issues;
    };

    let grammar = v["grammar"].as_str().unwrap_or("");
    if !grammar.starts_with(grammar_prefix) {
        issues.push(CoverageIssue {
            language: language_id.into(),
            message: format!(
                "grammar pin `{grammar}` does not start with `{grammar_prefix}` — bump or fix the manifest"
            ),
        });
    }

    let Some(handlers_obj) = v["handlers"].as_object() else {
        issues.push(CoverageIssue {
            language: language_id.into(),
            message: "manifest missing `handlers` object".into(),
        });
        return issues;
    };

    let handlers: HashMap<String, String> = handlers_obj
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("Skip").to_string()))
        .collect();

    for (kind, handler) in &handlers {
        if !ALLOWED_HANDLERS.contains(&handler.as_str()) {
            issues.push(CoverageIssue {
                language: language_id.into(),
                message: format!("kind `{kind}` has invalid handler `{handler}`"),
            });
        }
    }

    let kinds = grammar_named_kinds(lang);
    for kind in &kinds {
        if !handlers.contains_key(kind) {
            issues.push(CoverageIssue {
                language: language_id.into(),
                message: format!(
                    "grammar kind `{kind}` missing from ast-coverage.json — add a handler (often `Skip`)"
                ),
            });
        }
    }
    for key in handlers.keys() {
        if !kinds.contains(key) {
            issues.push(CoverageIssue {
                language: language_id.into(),
                message: format!(
                    "manifest key `{key}` not in grammar named kinds — remove stale entry after grammar bump"
                ),
            });
        }
    }
    issues
}

/// Bi-directional sync: plugin dispatch table ↔ `{lang}-ast-coverage.json` handlers.
///
/// - Every kind the plugin handles with a semantic mask (`Symbol` / `Relation` /
///   `CfgStatement` / `AstSkeleton`) MUST be marked non-`Skip` in the manifest.
/// - Every non-`Skip` / non-`Literal` manifest kind MUST appear in the plugin table
///   with a compatible semantic mask.
///
/// Call this from language crate tests once `declared_ast_handlers()` is wired.
pub fn check_plugin_manifest_sync(
    language_id: &str,
    manifest_json: &str,
    table: &AstHandlerTable,
    lang: &tree_sitter::Language,
) -> Vec<CoverageIssue> {
    let mut issues = Vec::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(manifest_json) else {
        issues.push(CoverageIssue {
            language: language_id.into(),
            message: "manifest JSON failed to parse".into(),
        });
        return issues;
    };
    let Some(handlers_obj) = v["handlers"].as_object() else {
        issues.push(CoverageIssue {
            language: language_id.into(),
            message: "manifest missing `handlers` object".into(),
        });
        return issues;
    };

    let handlers: HashMap<String, String> = handlers_obj
        .iter()
        .map(|(k, val)| (k.clone(), val.as_str().unwrap_or("Skip").to_string()))
        .collect();

    // Plugin → manifest: semantic handlers must not be Skip in JSON.
    for (kind_id, kind_name, mask) in table.iter() {
        if mask.is_passive() {
            continue;
        }
        match handlers.get(kind_name).map(|s| s.as_str()) {
            None => issues.push(CoverageIssue {
                language: language_id.into(),
                message: format!(
                    "plugin handles `{kind_name}` (kind_id={kind_id}, {}) but it is missing from ast-coverage.json",
                    mask.to_coverage_label()
                ),
            }),
            Some("Skip") => issues.push(CoverageIssue {
                language: language_id.into(),
                message: format!(
                    "plugin handles `{kind_name}` as {} but ast-coverage.json marks it `Skip`",
                    mask.to_coverage_label()
                ),
            }),
            Some(label) => {
                let Some(expected) = HandlerMask::from_coverage_label(label) else {
                    continue;
                };
                if !mask.intersects(expected) && !expected.is_passive() {
                    issues.push(CoverageIssue {
                        language: language_id.into(),
                        message: format!(
                            "plugin `{kind_name}` mask {} does not include manifest handler `{label}`",
                            mask.to_coverage_label()
                        ),
                    });
                }
            }
        }
    }

    // Manifest → plugin: active JSON kinds must be registered.
    for (kind_name, label) in &handlers {
        let Some(expected) = HandlerMask::from_coverage_label(label) else {
            continue;
        };
        if expected.is_passive() {
            continue;
        }
        match table.get_by_name(kind_name) {
            None => {
                let kind_id = lang.id_for_node_kind(kind_name, true);
                issues.push(CoverageIssue {
                    language: language_id.into(),
                    message: format!(
                        "ast-coverage.json marks `{kind_name}` as `{label}` (kind_id={kind_id}) but plugin AstHandlerTable has no entry"
                    ),
                });
            }
            Some((_id, mask)) if mask.is_passive() || !mask.intersects(expected) => {
                issues.push(CoverageIssue {
                    language: language_id.into(),
                    message: format!(
                        "ast-coverage.json marks `{kind_name}` as `{label}` but plugin table has {}",
                        mask.to_coverage_label()
                    ),
                });
            }
            Some(_) => {}
        }
    }

    issues
}

/// Validate every bundled spec under `crates_dir` (parent of `rgctl-lang-*`).
pub fn check_crates_dir(crates_dir: &Path) -> Vec<CoverageIssue> {
    let mut all = Vec::new();
    for spec in bundled_specs() {
        let path = crates_dir.join(spec.crate_dir).join(spec.manifest_file);
        match std::fs::read_to_string(&path) {
            Ok(json) => {
                let lang = (spec.language)();
                all.extend(check_manifest(spec.id, &json, spec.grammar_prefix, &lang));
            }
            Err(e) => all.push(CoverageIssue {
                language: spec.id.into(),
                message: format!("cannot read {}: {e}", path.display()),
            }),
        }
    }
    all
}

/// Paths that should trigger a rebuild of consumers (`cargo:rerun-if-changed=`).
pub fn rerun_if_changed_paths(crates_dir: &Path) -> Vec<PathBuf> {
    bundled_specs()
        .iter()
        .map(|s| crates_dir.join(s.crate_dir).join(s.manifest_file))
        .collect()
}

/// Format issues as `cargo:warning=` lines (and optional hard failure).
pub fn emit_cargo_warnings(issues: &[CoverageIssue], strict: bool) -> Result<(), String> {
    if issues.is_empty() {
        return Ok(());
    }
    for issue in issues {
        println!(
            "cargo:warning=AST coverage drift [{}]: {}",
            issue.language, issue.message
        );
    }
    println!(
        "cargo:warning=AST coverage: {} issue(s) — update `*-ast-coverage.json` after grammar bumps (RGCTL_AST_COVERAGE_STRICT=1 fails the build)",
        issues.len()
    );
    if strict {
        return Err(format!(
            "RGCTL_AST_COVERAGE_STRICT=1: {} AST coverage issue(s)",
            issues.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_manifest_in_workspace_matches_grammar() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let path = crates.join("rgctl-lang-java/java-ast-coverage.json");
        let json = std::fs::read_to_string(&path).expect("java manifest");
        let lang = tree_sitter_java::LANGUAGE.into();
        let issues = check_manifest("java", &json, "tree-sitter-java@", &lang);
        assert!(
            issues.is_empty(),
            "java coverage drift: {issues:?}"
        );
    }

    #[test]
    fn plugin_manifest_sync_detects_skip_drift() {
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let kind = "function_item";
        let kind_id = lang.id_for_node_kind(kind, true);
        assert_ne!(kind_id, 0, "rust grammar must know function_item");

        let mut table = AstHandlerTable::new();
        table.insert(kind_id, kind, HandlerMask::SYMBOL);

        let manifest = r#"{
            "grammar": "tree-sitter-rust@0",
            "handlers": { "function_item": "Skip" }
        }"#;
        let issues = check_plugin_manifest_sync("rust", manifest, &table, &lang);
        assert!(
            issues.iter().any(|i| i.message.contains("Skip")),
            "expected Skip drift, got {issues:?}"
        );
    }

    #[test]
    fn plugin_manifest_sync_detects_missing_plugin_entry() {
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let table = AstHandlerTable::new();
        let manifest = r#"{
            "grammar": "tree-sitter-rust@0",
            "handlers": { "function_item": "Symbol" }
        }"#;
        let issues = check_plugin_manifest_sync("rust", manifest, &table, &lang);
        assert!(
            issues.iter().any(|i| i.message.contains("no entry")),
            "expected missing plugin entry, got {issues:?}"
        );
    }

    #[test]
    fn bundled_specs_cover_expected_languages() {
        let ids: HashSet<_> = bundled_specs().iter().map(|s| s.id).collect();
        for need in ["java", "kotlin", "groovy", "markdown", "ruby"] {
            assert!(ids.contains(need), "missing {need}");
        }
    }
}
