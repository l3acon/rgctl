//! Golden correctness for OpenSpec 3.1 Rust call/symbol depth.

use rgctl_lang_rust::RustPlugin;
use rgctl_plugin_api::{LanguagePlugin, RelationType, SymbolType};
use std::path::Path;

#[test]
fn scoped_associated_and_turbofish_call_hints() {
    let source = br#"
pub struct Widget;

impl Widget {
    pub fn new() -> Self { Widget }
    fn helper(&self) {}
    pub fn run(&self) {
        let _a = Widget::new();
        Self::helper(self);
        identity::<Widget>(self);
    }
}

fn identity<T>(x: T) -> T { x }
"#;
    let plugin = RustPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("src/widget.rs"), source)
        .expect("extract_all");

    let calls: Vec<_> = out
        .relations
        .iter()
        .filter(|r| r.relation_type == RelationType::Calls)
        .collect();

    assert!(
        calls.iter().any(|r| {
            r.to_type_hint.as_deref() == Some("Widget")
                && (r.to.contains("new")
                    || r.to.contains("<init>")
                    || r.to_qualified_hint
                        .as_deref()
                        .is_some_and(|h| h.contains("Widget") && h.contains("new")))
        }),
        "Widget::new hint missing: {calls:?}"
    );
    assert!(
        calls.iter().any(|r| {
            r.from.contains("run")
                && r.to.contains("helper")
                && r.to_type_hint.as_deref() == Some("Widget")
        }),
        "Self::helper hint missing: {calls:?}"
    );
    assert!(
        calls.iter().any(|r| r.to.contains("identity")),
        "turbofish identity::<Widget> missing: {calls:?}"
    );
}

#[test]
fn enum_variants_type_item_macro_and_trait_signatures() {
    let source = br#"
enum Color { Red, Green }

type Id = u64;

macro_rules! noop { () => {}; }

trait Painter {
    type Medium;
    fn paint(&self);
}
"#;
    let plugin = RustPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("depth.rs"), source)
        .expect("extract_all");

    let color = out
        .symbols
        .iter()
        .find(|s| s.name == "Color" && s.symbol_type == SymbolType::Enum)
        .expect("Color enum");
    assert_eq!(color.fields.len(), 2);
    assert!(out.symbols.iter().any(|s| {
        s.name == "Red"
            && s.metadata
                .get("is_enum_variant")
                .and_then(|v| v.as_bool())
                == Some(true)
    }));
    assert!(out
        .symbols
        .iter()
        .any(|s| s.name == "Id" && s.symbol_type == SymbolType::TypeAlias));
    assert!(out
        .symbols
        .iter()
        .any(|s| s.name == "noop" && s.symbol_type == SymbolType::Macro));
    assert!(out.symbols.iter().any(|s| {
        s.name == "Medium"
            && s.metadata
                .get("kind")
                .and_then(|v| v.as_str())
                == Some("associated_type")
    }));
    assert!(out.symbols.iter().any(|s| {
        s.name == "paint"
            && s.metadata
                .get("trait_method")
                .and_then(|v| v.as_bool())
                == Some(true)
    }));
}
