//! Golden correctness for OpenSpec 3.6.5 Ruby AST depth.

use rgctl_lang_ruby::RubyPlugin;
use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
use std::path::Path;

#[test]
fn attrs_ivars_lambda_and_superclass() {
    let source = br#"
class Animal
end

class Dog < Animal
  attr_reader :name
  attr_accessor :age

  def initialize(name)
    @name = name
    @breed = "lab"
  end

  def transform
    ->(x) { x }
  end
end
"#;
    let plugin = RubyPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("dog.rb"), source)
        .expect("extract_all");

    let dog = out
        .symbols
        .iter()
        .find(|s| s.name == "Dog" && s.symbol_type == SymbolType::Class)
        .expect("Dog");
    assert!(
        dog.fields.iter().any(|f| f.name == "name"),
        "attr_reader name missing: {:?}",
        dog.fields
    );
    assert!(
        dog.fields.iter().any(|f| f.name == "age"),
        "attr_accessor age missing: {:?}",
        dog.fields
    );
    assert!(
        dog.fields.iter().any(|f| f.name == "breed"),
        "@breed ivar missing: {:?}",
        dog.fields
    );

    assert!(
        out.relations.iter().any(|r| {
            r.relation_type == RelationType::Extends
                && r.to.contains("Animal")
                && r.from.contains("Dog")
        }),
        "superclass Extends missing: {:?}",
        out.relations
    );

    assert!(
        out.symbols.iter().any(|s| {
            s.symbol_type == SymbolType::Function
                && s.metadata.get("kind").and_then(|v| v.as_str()) == Some("lambda")
        }),
        "lambda symbol missing: {:?}",
        out.symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Function)
            .map(|s| &s.name)
            .collect::<Vec<_>>()
    );
}
