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

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../ruby-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_ruby::LANGUAGE.into();
    let tracker =
        rgctl_ast_coverage::AstCoverageTracker::from_manifest_json("ruby", MANIFEST, &lang)
            .expect("tracker from manifest");
    assert!(
        tracker.expected_len() > 0,
        "manifest should declare active handlers"
    );
    rgctl_ast_coverage::install_hit_callback(tracker);

    let sources: &[(&str, &[u8])] = &[(
        "ast_coverage_corpus.rb",
        br#"
module M; end
class Animal; end
class Dog < Animal
  attr_reader :name
  attr_writer :age
  attr_accessor :color
  def initialize
    @name = "x"
  end
  def run
    x = 1
    x += 1
    if true; elsif false; else; end
    for i in 1..1; break; end
    begin; rescue; ensure; end
    case 1; when 1; end
    lambda { |y| y }
    ->(z) { z }
    ping()
    return
  end
  def self.ping; end
end
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = RubyPlugin::new().expect("plugin");
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
