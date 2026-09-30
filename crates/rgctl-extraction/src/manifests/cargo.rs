//! Cargo.toml → DependencyDeclaration.

use super::{loc, DependencyDeclaration};
use std::path::Path;
use toml_edit::{DocumentMut, Item, Value};

pub fn extract(path: &Path, source: &[u8]) -> Vec<DependencyDeclaration> {
    let Ok(text) = std::str::from_utf8(source) else {
        return Vec::new();
    };
    let Ok(doc) = text.parse::<DocumentMut>() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (section, scope) in [
        ("dependencies", "normal"),
        ("dev-dependencies", "dev"),
        ("build-dependencies", "build"),
    ] {
        if let Some(Item::Table(table)) = doc.get(section) {
            for (name, item) in table.iter() {
                let (version, unresolved) = match item {
                    Item::Value(Value::String(s)) => (Some(s.value().to_string()), false),
                    Item::Value(Value::InlineTable(t)) => {
                        if t.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
                            (None, true)
                        } else {
                            let ver = t
                                .get("version")
                                .and_then(|v| v.as_str())
                                .map(str::to_string);
                            (ver, false)
                        }
                    }
                    Item::Table(t) => {
                        if t.get("workspace")
                            .and_then(|i| i.as_bool())
                            .unwrap_or(false)
                        {
                            (None, true)
                        } else {
                            let ver = t
                                .get("version")
                                .and_then(|i| i.as_str())
                                .map(str::to_string);
                            (ver, false)
                        }
                    }
                    _ => (None, false),
                };
                let line = item.span().map(|s| {
                    // Approximate line from byte offset.
                    text[..s.start.min(text.len())].bytes().filter(|b| *b == b'\n').count() + 1
                }).unwrap_or(1);
                out.push(DependencyDeclaration {
                    name: name.to_string(),
                    version_requirement: version,
                    scope: Some(scope.to_string()),
                    ecosystem: "cargo".to_string(),
                    location: loc(path, line, line),
                    optional: false,
                    unresolved,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_deps() {
        let src = b"[dependencies]\nserde = \"1.0\"\ntokio = { version = \"1\", features = [\"full\"] }\n";
        let decls = extract(Path::new("Cargo.toml"), src);
        assert!(decls.iter().any(|d| d.name == "serde"));
        assert!(decls.iter().any(|d| d.name == "tokio"));
    }
}
