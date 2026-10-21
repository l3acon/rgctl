//! Golden correctness for OpenSpec 3.3 TypeScript AST depth.

use rgctl_lang_typescript::TypeScriptPlugin;
use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
use std::path::Path;

#[test]
fn abstract_class_fields_methods_and_namespace() {
    let source = br#"
abstract class Base {
  abstract run(): void;
  host: string = "localhost";
}

namespace Utils {
  export function helper() {}
}

function* gen() {
  yield 1;
}

const g = function* () {
  yield 2;
};
"#;
    let plugin = TypeScriptPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("depth.ts"), source)
        .expect("extract_all");

    let base = out
        .symbols
        .iter()
        .find(|s| s.name == "Base" && s.symbol_type == SymbolType::Class)
        .expect("Base");
    assert!(base.fields.iter().any(|f| f.name == "host"));

    assert!(
        out.symbols.iter().any(|s| {
            s.name == "run"
                && s.qualified_name.as_deref() == Some("Base.run")
                && s.metadata.get("is_abstract").and_then(|v| v.as_bool()) == Some(true)
        }),
        "abstract run missing: {:?}",
        out.symbols
            .iter()
            .map(|s| (&s.name, &s.qualified_name))
            .collect::<Vec<_>>()
    );
    assert!(
        out.symbols
            .iter()
            .any(|s| s.name == "Utils" && s.symbol_type == SymbolType::Module),
        "namespace Utils missing"
    );
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
}

#[test]
fn export_specifier_and_instantiates() {
    let source = br#"
export { foo as bar } from './mod';

class OrderDto {}
class Service {
  make(): OrderDto {
    return new OrderDto();
  }
}
"#;
    let plugin = TypeScriptPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("svc.ts"), source)
        .expect("extract_all");

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
            .any(|r| r.relation_type == RelationType::Instantiates && r.to == "OrderDto"),
        "Instantiates OrderDto missing: {:?}",
        out.relations
    );
}
