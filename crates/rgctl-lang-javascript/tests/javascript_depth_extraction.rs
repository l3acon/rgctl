//! Golden correctness for OpenSpec 3.3 JavaScript AST depth.

use rgctl_lang_javascript::JavaScriptPlugin;
use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
use std::path::Path;

#[test]
fn generators_exports_new_and_decorators() {
    let source = br#"
function* gen() {
  yield 1;
}

const g = function* () {
  yield 2;
};

export { foo as bar } from './mod';

@controller
class Service {
  @get
  list() {
    return new Order();
  }
}

class Order {}
"#;
    let plugin = JavaScriptPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("depth.js"), source)
        .expect("extract_all");

    assert!(
        out.symbols.iter().any(|s| s.name == "gen"),
        "generator gen missing"
    );
    assert!(
        out.symbols.iter().any(|s| s.name == "g"),
        "bound generator g missing: {:?}",
        out.symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Function)
            .map(|s| &s.name)
            .collect::<Vec<_>>()
    );
    assert!(
        out.relations.iter().any(|r| {
            r.relation_type == RelationType::Uses
                && r.to == "bar"
                && r.metadata.get("kind").and_then(|v| v.as_str()) == Some("export_specifier")
        }),
        "export specifier missing: {:?}",
        out.relations
    );
    assert!(
        out.relations
            .iter()
            .any(|r| r.relation_type == RelationType::Instantiates && r.to == "Order"),
        "Instantiates Order missing: {:?}",
        out.relations
    );
    assert!(
        out.relations.iter().any(|r| {
            r.relation_type == RelationType::AnnotatedWith && r.to == "controller"
        }),
        "class decorator missing: {:?}",
        out.relations
    );
}
