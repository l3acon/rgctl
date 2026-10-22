//! Golden correctness for OpenSpec 3.6.5 PHP AST depth.

use rgctl_lang_php::PhpPlugin;
use rgctl_plugin_api::{LanguagePlugin, SymbolType};
use std::path::Path;

#[test]
fn top_level_and_class_constants_plus_use() {
    let source = br#"<?php
namespace App;

use Vendor\Lib\Helper;

const MAX_RETRIES = 3;

class Order {
    public const STATUS_OPEN = 'open';
    private const FLAG = 1;

    public function run(): void {}
}
"#;
    let plugin = PhpPlugin::new().expect("plugin");
    let out = plugin
        .extract_all(Path::new("Order.php"), source)
        .expect("extract_all");

    assert!(
        out.symbols.iter().any(|s| {
            s.name == "MAX_RETRIES" && s.symbol_type == SymbolType::Variable
        }),
        "top-level const missing: {:?}",
        out.symbols
            .iter()
            .map(|s| (&s.name, &s.symbol_type))
            .collect::<Vec<_>>()
    );

    let order = out
        .symbols
        .iter()
        .find(|s| s.name == "Order" && s.symbol_type == SymbolType::Class)
        .expect("Order class");
    assert!(
        order.fields.iter().any(|f| f.name == "STATUS_OPEN"),
        "class const STATUS_OPEN missing: {:?}",
        order.fields
    );
    assert!(
        order.fields.iter().any(|f| f.name == "FLAG"),
        "class const FLAG missing: {:?}",
        order.fields
    );

    assert!(
        out.symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Import && s.name.contains("Helper")),
        "namespace use Helper missing: {:?}",
        out.symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Import)
            .map(|s| &s.name)
            .collect::<Vec<_>>()
    );
}
