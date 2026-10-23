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
            s.name == "Order"
                && s.metadata.get("is_record").and_then(|v| v.as_bool()) == Some(true)
        }),
        "record Order missing"
    );
}
