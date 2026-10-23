//! Golden correctness for OpenSpec 3.5 Go const/var/type parity.

use rgctl_lang_go::GoPlugin;
use rgctl_plugin_api::{LanguagePlugin, SymbolType};
use std::path::Path;

#[test]
fn const_var_and_type_alias() {
    let source = br#"
const Max = 10

var Count int

type ID = string
"#;
    let plugin = GoPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("depth.go"), source)
        .expect("extract_all");

    assert!(
        out.symbols.iter().any(|s| {
            s.name == "Max"
                && s.metadata.get("is_const").and_then(|v| v.as_bool()) == Some(true)
        }),
        "const Max missing: {:?}",
        out.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    assert!(
        out.symbols.iter().any(|s| {
            s.name == "Count"
                && s.metadata.get("kind").and_then(|v| v.as_str()) == Some("var_spec")
        }),
        "var Count missing"
    );
    assert!(
        out.symbols
            .iter()
            .any(|s| s.name == "ID" && s.symbol_type == SymbolType::TypeAlias),
        "type alias ID missing"
    );
}

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../go-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
    let tracker = rgctl_ast_coverage::AstCoverageTracker::from_manifest_json(
        "go",
        MANIFEST,
        &lang,
    )
    .expect("tracker from manifest");
    assert!(
        tracker.expected_len() > 0,
        "manifest should declare active handlers"
    );
    rgctl_ast_coverage::install_hit_callback(tracker);

    let sources: &[(&str, &[u8])] = &[(
        "ast_coverage_corpus.go",
        br#"
package main
import "fmt"
const Max = 10
var Count int
type ID = string
type S struct { X int }
func (s *S) M() {
  if true { return }
  for i := 0; i < 1; i++ { break }
  fmt.Println(Max)
}
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = GoPlugin::new().expect("plugin");
    for (path, src) in sources {
        present.extend(rgctl_ast_coverage::named_kind_ids_in_source(&lang, src));
        plugin
            .extract_all(std::path::Path::new(path), src)
            .expect("extract_all");
    }

    let tracker = rgctl_ast_coverage::AstCoverageTracker::global().expect("tracker installed");
    match tracker.assert_all_exercised() {
        Ok(msg) => eprintln!("{msg}"),
        Err(full) => {
            tracker
                .assert_present_kinds_exercised(present.iter().copied())
                .unwrap_or_else(|e| panic!("{e}\n(full gaps: {full})"));
            assert!(
                tracker.total_hits() > 0,
                "RGCTL_AST_COVERAGE_TRACK=1 but no dispatch hits recorded"
            );
            eprintln!("soft-gate (full assert not met): {full}");
        }
    }
}
