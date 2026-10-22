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
