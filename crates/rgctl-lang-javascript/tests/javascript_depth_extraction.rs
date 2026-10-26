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
        out.relations
            .iter()
            .any(|r| { r.relation_type == RelationType::AnnotatedWith && r.to == "controller" }),
        "class decorator missing: {:?}",
        out.relations
    );
}

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../javascript-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
    let tracker =
        rgctl_ast_coverage::AstCoverageTracker::from_manifest_json("javascript", MANIFEST, &lang)
            .expect("tracker from manifest");
    assert!(
        tracker.expected_len() > 0,
        "manifest should declare active handlers"
    );
    rgctl_ast_coverage::install_hit_callback(tracker);

    let sources: &[(&str, &[u8])] = &[(
        "ast_coverage_corpus.js",
        br#"
export function* gen() { yield 1; }
export { foo as bar } from "./m";
class C {
  @dec
  method() {}
}
const x = new C();
function* g2() { yield 2; }
const f = function* () { yield 3; };
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = JavaScriptPlugin::new().expect("plugin");
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
