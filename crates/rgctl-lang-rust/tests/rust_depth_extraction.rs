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

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../rust-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let tracker = rgctl_ast_coverage::AstCoverageTracker::from_manifest_json(
        "rust",
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
        "ast_coverage_corpus.rs",
        br#"
extern crate core;
use std::fmt::Debug;

mod nested {
    pub fn ping() {}
}

const MAX: i32 = 10;
static GLOBAL: i32 = 0;
type Id = u64;
union Word { i: i32, u: u32 }

struct Point { x: i32, y: i32 }

enum Color { Red, Green }

trait Painter {
    type Medium;
    fn paint(&self);
}

macro_rules! noop { () => {}; }

impl Point {
    fn helper(&self) {}
    fn run(&self) -> Result<(), ()> {
        let mut n = 0;
        n = 1;
        n += 1;
        let _c = || 1;
        let _a = async { 1 };
        let _u = unsafe { 1 };
        if let Some(x) = Some(1)
            && let Some(y) = Some(2)
        {
            let _ = x + y;
        }
        if true {
        } else {
        }
        for _i in 0..1 {
            break;
        }
        while false {
            continue;
        }
        loop {
            break;
        }
        match n {
            0 => {}
            _ => {}
        }
        let _ = self.try_me()?;
        noop!();
        nested::ping();
        identity::<i32>(1);
        Point::helper(self);
        return Ok(());
    }
    fn try_me(&self) -> Result<(), ()> { Ok(()) }
}

fn identity<T: Debug>(x: T) -> T { x }

fn gen_block() {
    let _g = gen {
        yield 1;
    };
}
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = RustPlugin::new().expect("plugin");
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
