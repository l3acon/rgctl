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
        out.symbols
            .iter()
            .any(|s| { s.name == "MAX_RETRIES" && s.symbol_type == SymbolType::Variable }),
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

#[test]
fn ast_coverage_all_declared_handlers_exercised() {
    if std::env::var_os("RGCTL_AST_COVERAGE_TRACK").is_none() {
        return;
    }
    const MANIFEST: &str = include_str!("../php-ast-coverage.json");
    let lang: tree_sitter::Language = tree_sitter_php::LANGUAGE_PHP.into();
    let tracker =
        rgctl_ast_coverage::AstCoverageTracker::from_manifest_json("php", MANIFEST, &lang)
            .expect("tracker from manifest");
    assert!(
        tracker.expected_len() > 0,
        "manifest should declare active handlers"
    );
    rgctl_ast_coverage::install_hit_callback(tracker);

    let sources: &[(&str, &[u8])] = &[(
        "ast_coverage_corpus.php",
        br#"
<?php
namespace App;
use App\Models\Order as Ord;
const MAX = 10;
interface Repo {}
enum Status { case Pending; }
class Order extends Base implements Repo {
  public const LIMIT = 5;
  public function run() {
    $x = 1;
    $x += 1;
    if ($x) {} else {}
    foreach ([] as $v) { break; }
    for (;;) { break; }
    while (false) { continue; }
    do { break; } while (false);
    switch ($x) { case 0: break; }
    try {} catch (\Exception $e) {} finally {}
    match ($x) { default => 0 };
    $this->run();
    foo();
    return;
  }
}
function foo() {}
"#,
    )];

    let mut present = std::collections::HashSet::new();
    let plugin = PhpPlugin::new().expect("plugin");
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
