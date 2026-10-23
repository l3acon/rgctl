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
