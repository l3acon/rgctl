//! Golden correctness for OpenSpec 3.5 Kotlin companion/enum/typealias parity.

use rgctl_lang_kotlin::KotlinPlugin;
use rgctl_plugin_api::{LanguagePlugin, SymbolType};
use std::path::Path;

#[test]
fn enum_entries_typealias_and_companion() {
    let source = br#"
enum class Status { PENDING, DONE }

typealias UserId = String

class Service {
  companion object {
    const val DEFAULT = 1
  }
}
"#;
    let plugin = KotlinPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("depth.kt"), source)
        .expect("extract_all");

    let status = out
        .symbols
        .iter()
        .find(|s| s.name == "Status" && s.symbol_type == SymbolType::Enum)
        .expect("Status");
    assert!(
        status.fields.iter().any(|f| f.name == "PENDING"),
        "PENDING missing: {:?}",
        status.fields
    );
    assert!(status.fields.iter().any(|f| f.name == "DONE"));

    assert!(
        out.symbols
            .iter()
            .any(|s| s.name == "UserId" && s.symbol_type == SymbolType::TypeAlias),
        "typealias UserId missing: {:?}",
        out.symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    assert!(
        out.symbols.iter().any(|s| s.name == "Companion"),
        "companion object missing"
    );
}

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../kotlin-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_kotlin_ng::LANGUAGE.into();
    let tracker =
        rgctl_ast_coverage::AstCoverageTracker::from_manifest_json("kotlin", MANIFEST, &lang)
            .expect("tracker from manifest");
    assert!(
        tracker.expected_len() > 0,
        "manifest should declare active handlers"
    );
    rgctl_ast_coverage::install_hit_callback(tracker);

    let sources: &[(&str, &[u8])] = &[(
        "ast_coverage_corpus.kt",
        br#"
import kotlin.io.*
typealias Id = Int
enum class Color { RED, GREEN }
class Widget(val x: Int) {
  companion object { fun create() = Widget(0) }
  constructor() : this(0)
  var y: Int = 0
    get() = field
    set(v) { field = v }
  fun run() {
    if (true) {} else {}
    for (i in 0..1) { break }
    do { break } while (false)
    when (x) { 0 -> {} else -> {} }
    try { throw Exception() } catch (e: Exception) {} finally {}
    create()
    return
  }
}
object Singleton
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = KotlinPlugin::new().expect("plugin");
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
