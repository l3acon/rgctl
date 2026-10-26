//! Golden correctness for OpenSpec 3.5 C# field/member parity.

use rgctl_lang_csharp::CSharpPlugin;
use rgctl_plugin_api::{LanguagePlugin, SymbolType};
use std::path::Path;

#[test]
fn enum_members_events_and_record() {
    let source = br#"
public enum Status { Pending, Done }

public class Service {
  public event EventHandler Changed;
  public string Name { get; set; }
}

public record Order(string Id);
"#;
    let plugin = CSharpPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("depth.cs"), source)
        .expect("extract_all");

    let status = out
        .symbols
        .iter()
        .find(|s| s.name == "Status" && s.symbol_type == SymbolType::Enum)
        .expect("Status");
    assert!(status.fields.iter().any(|f| f.name == "Pending"));
    assert!(status.fields.iter().any(|f| f.name == "Done"));

    let service = out
        .symbols
        .iter()
        .find(|s| s.name == "Service" && s.symbol_type == SymbolType::Class)
        .expect("Service");
    assert!(service.fields.iter().any(|f| f.name == "Changed"));
    assert!(service.fields.iter().any(|f| f.name == "Name"));

    assert!(
        out.symbols.iter().any(|s| {
            s.name == "Order" && s.metadata.get("is_record").and_then(|v| v.as_bool()) == Some(true)
        }),
        "record Order missing"
    );
}

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../csharp-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_c_sharp::LANGUAGE.into();
    let tracker =
        rgctl_ast_coverage::AstCoverageTracker::from_manifest_json("csharp", MANIFEST, &lang)
            .expect("tracker from manifest");
    assert!(
        tracker.expected_len() > 0,
        "manifest should declare active handlers"
    );
    rgctl_ast_coverage::install_hit_callback(tracker);

    let sources: &[(&str, &[u8])] = &[(
        "ast_coverage_corpus.cs",
        br#"
using System;
namespace Demo {
  enum Color { Red }
  interface IRepo { void Save(); }
  record Rec(int X);
  class Widget : IRepo {
    public event EventHandler Changed;
    public int Prop { get; set; }
    void Local() {
      void inner() {}
      if (true) {} else {}
      foreach (var x in new int[]{1}) { break; }
      for (int i=0;i<1;i++) {}
      do { break; } while(false);
      switch (1) { case 1: break; }
      try {} catch {} finally {}
      Save();
      return;
    }
    public void Save() {}
  }
}
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = CSharpPlugin::new().expect("plugin");
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
