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
