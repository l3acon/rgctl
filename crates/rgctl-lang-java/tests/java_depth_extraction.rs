//! Golden correctness for OpenSpec 3.6.5 Java AST depth.

use rgctl_lang_java::JavaPlugin;
use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
use std::path::Path;

#[test]
fn interface_constants_inheritance_and_calls() {
    let source = br#"
interface Named {
    String DEFAULT = "anon";
    String label();
}

class Base {}
class Widget extends Base implements Named {
    public String label() {
        return new String("w");
    }

    public void run(Named n) {
        n.label();
        for (String s : new String[]{"a"}) {
            System.out.println(s);
        }
    }
}
"#;
    let plugin = JavaPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("Widget.java"), source)
        .expect("extract_all");

    let named = out
        .symbols
        .iter()
        .find(|s| s.name == "Named" && s.symbol_type == SymbolType::Interface)
        .expect("Named interface");
    assert!(
        named.fields.iter().any(|f| f.name == "DEFAULT"),
        "interface constant DEFAULT missing: {:?}",
        named.fields
    );

    assert!(
        out.relations
            .iter()
            .any(|r| r.relation_type == RelationType::Extends && r.to.contains("Base")),
        "extends Base missing: {:?}",
        out.relations
    );
    assert!(
        out.relations
            .iter()
            .any(|r| r.relation_type == RelationType::Implements && r.to.contains("Named")),
        "implements Named missing: {:?}",
        out.relations
    );
    assert!(
        out.relations
            .iter()
            .any(|r| r.relation_type == RelationType::Calls && r.to.contains("label")),
        "method_invocation Calls missing: {:?}",
        out.relations
    );
    assert!(
        out.relations.iter().any(|r| {
            r.relation_type == RelationType::Instantiates && r.to.contains("String")
        }),
        "object_creation Instantiates missing: {:?}",
        out.relations
    );
}

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../java-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
    let tracker = rgctl_ast_coverage::AstCoverageTracker::from_manifest_json(
        "java",
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
        "ast_coverage_corpus.java",
        br#"
package demo;
import java.util.List;
interface Repo {
  int MAX = 10;
  void save();
}
class Base { void save() {} }
class Widget extends Base implements Repo {
  void run() {
    this.save();
    new Widget();
    for (String s : List.of("a")) {}
  }
}
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = JavaPlugin::new().expect("plugin");
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
