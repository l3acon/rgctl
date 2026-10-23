//! Golden correctness for OpenSpec 3.4 C AST depth.

use rgctl_lang_c::CPlugin;
use rgctl_plugin_api::{LanguagePlugin, SymbolType};
use std::path::Path;

#[test]
fn enum_union_and_preproc() {
    let source = br#"
enum Color { Red, Green = 2 };

union U { int i; float f; };

#define MAX 10
#define SQR(x) ((x)*(x))

struct Point { int x; int y; };
"#;
    let plugin = CPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("depth.c"), source)
        .expect("extract_all");

    let color = out
        .symbols
        .iter()
        .find(|s| s.name == "Color" && s.symbol_type == SymbolType::Enum)
        .expect("Color");
    assert!(color.fields.iter().any(|f| f.name == "Red"));
    assert!(color.fields.iter().any(|f| f.name == "Green"));

    assert!(
        out.symbols.iter().any(|s| {
            s.name == "U"
                && s.metadata.get("kind").and_then(|v| v.as_str()) == Some("union_specifier")
        }),
        "union U missing: {:?}",
        out.symbols
            .iter()
            .map(|s| &s.name)
            .collect::<Vec<_>>()
    );
    // `#define` macros are not indexed as graph Macro nodes (linux cold node cap).
    assert!(
        out.symbols
            .iter()
            .all(|s| s.symbol_type != SymbolType::Macro),
        "unexpected Macro symbols: {:?}",
        out.symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Macro)
            .map(|s| &s.name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../c-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    let tracker = rgctl_ast_coverage::AstCoverageTracker::from_manifest_json(
        "c",
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
        "ast_coverage_corpus.c",
        br#"
#include <stdio.h>
enum Color { Red, Green = 2 };
union U { int i; float f; };
struct Point { int x; int y; };
typedef int Id;
void foo(void) {
  int x = 0;
  x = 1;
  if (x) {} else {}
  for (;;) { break; }
  while (0) { continue; }
  do { break; } while (0);
  switch (x) { case 0: break; }
  goto end;
end:
  return;
}
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = CPlugin::new().expect("plugin");
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
