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
    assert!(
        out.symbols
            .iter()
            .any(|s| s.name == "MAX" && s.symbol_type == SymbolType::Macro)
    );
    assert!(
        out.symbols
            .iter()
            .any(|s| s.name == "SQR" && s.symbol_type == SymbolType::Macro)
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
