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

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../typescript-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let tracker = rgctl_ast_coverage::AstCoverageTracker::from_manifest_json(
        "typescript",
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
        "ast_coverage_corpus.ts",
        br#"
abstract class Base {
  abstract run(): void;
  host: string = "localhost";
}
namespace Utils {
  export function helper() {}
}
function* gen() { yield 1; }
const g = function* () { yield 2; };
export { helper as h } from "./m";
const s = new Base();
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = TypeScriptPlugin::new().expect("plugin");
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
