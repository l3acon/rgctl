//! YAML configuration format plugin (span-preserving via `marked-yaml`).

use crate::span_util::loc;
use marked_yaml::{parse_yaml, Node as YamlNode};
use rgctl_plugin_api::Result;
use rgctl_plugin_api::*;
use std::path::Path;

/// YAML config format plugin
pub struct YamlPlugin;

impl YamlPlugin {
    /// Create a new YAML plugin
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    fn flatten_node(
        &self,
        node: &YamlNode,
        prefix: &str,
        file: &str,
        results: &mut Vec<ConfigKey>,
    ) {
        match node {
            YamlNode::Mapping(map) => {
                for (k, v) in map.iter() {
                    let key = k.as_str();
                    let full_key = if prefix.is_empty() {
                        key.to_string()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    self.flatten_node(v, &full_key, file, results);
                }
            }
            YamlNode::Sequence(seq) => {
                let (sl, el, sc, ec) = span_of(node);
                results.push(ConfigKey {
                    key_path: prefix.to_string(),
                    value: format!("[array with {} items]", seq.len()),
                    value_type: ConfigValueType::Array,
                    location: loc(file, sl, el, sc, ec),
                });
            }
            YamlNode::Scalar(s) => {
                let (sl, el, sc, ec) = span_of(node);
                let text = s.as_str();
                let (value_type, value) = classify_scalar(text);
                results.push(ConfigKey {
                    key_path: prefix.to_string(),
                    value,
                    value_type,
                    location: loc(file, sl, el, sc, ec),
                });
            }
        }
    }
}

fn span_of(node: &YamlNode) -> (usize, usize, usize, usize) {
    let span = node.span();
    let (sl, sc) = span
        .start()
        .map(|m| (m.line(), m.column()))
        .unwrap_or((1, 1));
    let (el, ec) = span
        .end()
        .map(|m| (m.line(), m.column()))
        .unwrap_or((sl, sc));
    (sl.max(1), el.max(1), sc.max(1), ec.max(1))
}

fn classify_scalar(text: &str) -> (ConfigValueType, String) {
    if text == "null" || text == "~" || text.is_empty() {
        return (ConfigValueType::Null, text.to_string());
    }
    if text == "true" || text == "false" {
        return (ConfigValueType::Boolean, text.to_string());
    }
    if text.parse::<f64>().is_ok() {
        return (ConfigValueType::Number, text.to_string());
    }
    (ConfigValueType::String, text.to_string())
}

impl Default for YamlPlugin {
    fn default() -> Self {
        Self::new().expect("Failed to create YamlPlugin")
    }
}

impl ConfigFormatPlugin for YamlPlugin {
    fn format_id(&self) -> &str {
        "yaml"
    }

    fn file_extensions(&self) -> Vec<&str> {
        vec!["yaml", "yml"]
    }

    fn extract_config_keys(&self, file_path: &Path, source: &[u8]) -> Result<Vec<ConfigKey>> {
        let file = file_path.to_string_lossy().to_string();
        let text = std::str::from_utf8(source).map_err(|e| Error::ParseError {
            file: file_path.to_path_buf(),
            line: 0,
            message: e.to_string(),
        })?;
        let mut results = Vec::new();
        match parse_yaml(0, text) {
            Ok(node) => self.flatten_node(&node, "", &file, &mut results),
            Err(err) => {
                return Err(Error::ParseError {
                    file: file_path.to_path_buf(),
                    line: 0,
                    message: format!("yaml parse: {err}"),
                });
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_spans_are_nonzero() {
        let src = b"server:\n  port: 8080\n";
        let plugin = YamlPlugin::new().unwrap();
        let keys = plugin
            .extract_config_keys(Path::new("application.yml"), src)
            .unwrap();
        let port = keys.iter().find(|k| k.key_path == "server.port").unwrap();
        assert!(port.location.start_line >= 1, "{port:?}");
        assert_ne!(port.location.start_line, 0);
    }
}
