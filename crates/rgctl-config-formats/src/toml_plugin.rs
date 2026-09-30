//! TOML configuration format plugin (span-preserving via `toml_edit`).

use crate::span_util::{line_col_at, loc};
use rgctl_plugin_api::Result;
use rgctl_plugin_api::*;
use std::path::Path;
use toml_edit::{Item, DocumentMut};

/// TOML config format plugin
pub struct TomlPlugin;

impl TomlPlugin {
    /// Create a new TOML plugin
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    fn flatten_item(
        &self,
        item: &Item,
        prefix: &str,
        file: &str,
        source: &str,
        results: &mut Vec<ConfigKey>,
    ) {
        match item {
            Item::Table(table) => {
                for (k, v) in table.iter() {
                    let full_key = if prefix.is_empty() {
                        k.to_string()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    self.flatten_item(v, &full_key, file, source, results);
                }
            }
            Item::ArrayOfTables(arr) => {
                results.push(ConfigKey {
                    key_path: prefix.to_string(),
                    value: format!("[array with {} items]", arr.len()),
                    value_type: ConfigValueType::Array,
                    location: span_from_item(item, file, source),
                });
            }
            Item::Value(val) => match val {
                toml_edit::Value::InlineTable(t) => {
                    for (k, v) in t.iter() {
                        let full_key = if prefix.is_empty() {
                            k.to_string()
                        } else {
                            format!("{prefix}.{k}")
                        };
                        let fake = Item::Value(v.clone());
                        self.flatten_item(&fake, &full_key, file, source, results);
                    }
                }
                toml_edit::Value::Array(a) => {
                    results.push(ConfigKey {
                        key_path: prefix.to_string(),
                        value: format!("[array with {} items]", a.len()),
                        value_type: ConfigValueType::Array,
                        location: span_from_item(item, file, source),
                    });
                }
                other => {
                    let (vt, s) = value_to_typed(other);
                    results.push(ConfigKey {
                        key_path: prefix.to_string(),
                        value: s,
                        value_type: vt,
                        location: span_from_item(item, file, source),
                    });
                }
            },
            Item::None => {}
        }
    }
}

fn span_from_item(item: &Item, file: &str, source: &str) -> SourceLocation {
    if let Some(span) = item.span() {
        let (sl, sc) = line_col_at(source, span.start);
        let (el, ec) = line_col_at(source, span.end);
        return loc(file, sl, el, sc, ec);
    }
    loc(file, 1, 1, 1, 1)
}

fn value_to_typed(v: &toml_edit::Value) -> (ConfigValueType, String) {
    match v {
        toml_edit::Value::String(s) => (ConfigValueType::String, s.value().to_string()),
        toml_edit::Value::Integer(i) => (ConfigValueType::Number, i.to_string()),
        toml_edit::Value::Float(f) => (ConfigValueType::Number, f.to_string()),
        toml_edit::Value::Boolean(b) => (ConfigValueType::Boolean, b.to_string()),
        toml_edit::Value::Datetime(d) => (ConfigValueType::String, d.to_string()),
        other => (ConfigValueType::String, other.to_string()),
    }
}

impl Default for TomlPlugin {
    fn default() -> Self {
        Self::new().expect("Failed to create TomlPlugin")
    }
}

impl ConfigFormatPlugin for TomlPlugin {
    fn format_id(&self) -> &str {
        "toml"
    }

    fn file_extensions(&self) -> Vec<&str> {
        vec!["toml"]
    }

    fn extract_config_keys(&self, file_path: &Path, source: &[u8]) -> Result<Vec<ConfigKey>> {
        let file = file_path.to_string_lossy().to_string();
        let text = std::str::from_utf8(source).map_err(|e| Error::ParseError {
            file: file_path.to_path_buf(),
            line: 0,
            message: e.to_string(),
        })?;
        let doc: DocumentMut = text.parse().map_err(|e| Error::ParseError {
            file: file_path.to_path_buf(),
            line: 0,
            message: format!("toml parse: {e}"),
        })?;
        let mut results = Vec::new();
        self.flatten_item(doc.as_item(), "", &file, text, &mut results);
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_spans_nonzero() {
        let src = b"[server]\nport = 8080\n";
        let plugin = TomlPlugin::new().unwrap();
        let keys = plugin
            .extract_config_keys(Path::new("config.toml"), src)
            .unwrap();
        let port = keys.iter().find(|k| k.key_path.contains("port")).unwrap();
        assert!(port.location.start_line >= 1);
    }
}
