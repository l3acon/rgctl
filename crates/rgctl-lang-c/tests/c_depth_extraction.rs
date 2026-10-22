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
