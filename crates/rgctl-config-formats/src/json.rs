//! JSON configuration format plugin (spans via quoted-key lookup).

use crate::span_util::{find_quoted_key_span, loc};
use rgctl_plugin_api::Result;
use rgctl_plugin_api::*;
use std::path::Path;

/// JSON config format plugin
pub struct JsonPlugin;

impl JsonPlugin {
    /// Create a new JSON plugin
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    fn flatten_json_value(
        &self,
        value: &serde_json::Value,
        prefix: &str,
        file: &str,
        source: &str,
        used: &mut Vec<usize>,
        results: &mut Vec<ConfigKey>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let full_key = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    self.flatten_json_value(v, &full_key, file, source, used, results);
                }
            }
            serde_json::Value::Array(arr) => {
                let leaf = prefix.rsplit('.').next().unwrap_or(prefix);
                let location = find_quoted_key_span(source, leaf, used)
                    .map(|(sl, el, sc, ec)| loc(file, sl, el, sc, ec))
                    .unwrap_or_else(|| loc(file, 1, 1, 1, 1));
                results.push(ConfigKey {
                    key_path: prefix.to_string(),
                    value: format!("[array with {} items]", arr.len()),
                    value_type: ConfigValueType::Array,
                    location,
                });
            }
            other => {
                let leaf = prefix.rsplit('.').next().unwrap_or(prefix);
                let location = find_quoted_key_span(source, leaf, used)
                    .map(|(sl, el, sc, ec)| loc(file, sl, el, sc, ec))
                    .unwrap_or_else(|| loc(file, 1, 1, 1, 1));
                let (value_type, value) = match other {
                    serde_json::Value::String(s) => (ConfigValueType::String, s.clone()),
                    serde_json::Value::Number(n) => (ConfigValueType::Number, n.to_string()),
                    serde_json::Value::Bool(b) => (ConfigValueType::Boolean, b.to_string()),
                    serde_json::Value::Null => (ConfigValueType::Null, "null".to_string()),
                    _ => (ConfigValueType::String, other.to_string()),
                };
                results.push(ConfigKey {
                    key_path: prefix.to_string(),
                    value,
                    value_type,
                    location,
                });
            }
        }
    }
}

impl Default for JsonPlugin {
    fn default() -> Self {
        Self::new().expect("Failed to create JsonPlugin")
    }
}

impl ConfigFormatPlugin for JsonPlugin {
    fn format_id(&self) -> &str {
        "json"
    }

    fn file_extensions(&self) -> Vec<&str> {
        vec!["json"]
    }

    fn extract_config_keys(&self, file_path: &Path, source: &[u8]) -> Result<Vec<ConfigKey>> {
        let file = file_path.to_string_lossy().to_string();
        let text = std::str::from_utf8(source).map_err(|e| Error::ParseError {
            file: file_path.to_path_buf(),
            line: 0,
            message: e.to_string(),
        })?;
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| Error::ParseError {
                file: file_path.to_path_buf(),
                line: 0,
                message: e.to_string(),
            })?;
        let mut results = Vec::new();
        let mut used = Vec::new();
        self.flatten_json_value(&value, "", &file, text, &mut used, &mut results);
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_spans_nonzero() {
        let src = b"{\n  \"server\": {\n    \"port\": 8080\n  }\n}\n";
        let plugin = JsonPlugin::new().unwrap();
        let keys = plugin
            .extract_config_keys(Path::new("config.json"), src)
            .unwrap();
        let port = keys.iter().find(|k| k.key_path == "server.port").unwrap();
        assert!(port.location.start_line >= 1);
        assert_ne!(port.location.start_line, 0);
    }
}
