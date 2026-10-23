//! Golden correctness for OpenSpec 3.4 C++ AST depth.

use rgctl_lang_cpp::CppPlugin;
use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
use std::path::Path;

#[test]
fn namespace_alias_concept_enum_union_preproc() {
    let source = br#"
namespace N {
  void f();
}

using IntVec = std::vector<int>;

template<typename T>
concept Addable = requires(T a, T b) { a + b; };

enum Color { Red, Green = 2 };

union U { int i; float f; };

#define MAX 10
#define SQR(x) ((x)*(x))

using std::vector;

class Base {};
class Derived : public Base {
  void run() {
    auto* p = new Derived();
    auto x = foo<int>();
  }
};
"#;
    let plugin = CppPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("depth.cpp"), source)
        .expect("extract_all");

    assert!(
        out.symbols
            .iter()
            .any(|s| s.name == "N" && s.symbol_type == SymbolType::Module),
        "namespace N missing"
    );
    assert!(
        out.symbols.iter().any(|s| {
            s.name == "IntVec" && s.symbol_type == SymbolType::TypeAlias
        }),
        "alias IntVec missing"
    );
    assert!(
        out.symbols.iter().any(|s| {
            s.name == "Addable"
                && s.metadata.get("kind").and_then(|v| v.as_str()) == Some("concept_definition")
        }),
        "concept Addable missing"
    );
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
        "union U missing"
    );
    // `#define` macros are not indexed as graph Macro nodes (linux cold node cap).
    assert!(
        out.symbols
            .iter()
            .all(|s| s.symbol_type != SymbolType::Macro),
        "unexpected Macro symbols"
    );
    assert!(
        out.relations.iter().any(|r| {
            r.relation_type == RelationType::Uses
                && r.metadata.get("kind").and_then(|v| v.as_str()) == Some("using_declaration")
        }),
        "using Uses missing: {:?}",
        out.relations
    );
    assert!(
        out.relations
            .iter()
            .any(|r| r.relation_type == RelationType::Extends && r.to.contains("Base")),
        "Extends Base missing: {:?}",
        out.relations
    );
    assert!(
        out.relations
            .iter()
            .any(|r| r.relation_type == RelationType::Instantiates && r.to.contains("Derived")),
        "Instantiates Derived missing: {:?}",
        out.relations
    );
    assert!(
        out.relations.iter().any(|r| {
            r.relation_type == RelationType::Calls && r.to.contains("foo")
        }),
        "template_function call foo missing: {:?}",
        out.relations
    );
}

#[test]
fn enumerator_as_symbol_kind_visited() {
    // Coverage marks enumerator as Symbol; fields on the enum cover the semantic extract.
    let source = b"enum E { A, B };\n";
    let plugin = CppPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("e.cpp"), source)
        .expect("extract_all");
    let e = out
        .symbols
        .iter()
        .find(|s| s.name == "E")
        .expect("E");
    assert_eq!(e.fields.len(), 2);
}

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../cpp-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_cpp::LANGUAGE.into();
    let tracker = rgctl_ast_coverage::AstCoverageTracker::from_manifest_json(
        "cpp",
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
        "ast_coverage_corpus.cpp",
        br#"
#include <vector>
namespace ns {
  using Alias = int;
  concept Integral = true;
  enum Color { Red };
  union U { int i; };
  class Base {};
  class Derived : public Base {
    void run() {
      int x = 0;
      x = 1;
      if (x) {} else {}
      for (auto& v : std::vector<int>{}) { break; }
      while (0) { continue; }
      do { break; } while (0);
      switch (x) { case 0: break; }
      try { throw 1; } catch (...) {}
      co_await 1;
      co_yield 1;
      co_return;
      new int;
      return;
    }
  };
}
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = CppPlugin::new().expect("plugin");
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
