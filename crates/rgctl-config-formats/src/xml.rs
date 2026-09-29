//! XML configuration format plugin (`roxmltree`) for allowlisted non-POM XML.

use crate::span_util::{line_col_at, loc};
use rgctl_plugin_api::Result;
use rgctl_plugin_api::*;
use std::path::Path;

/// XML config format plugin (config-route only — never POM manifests).
pub struct XmlPlugin;

impl XmlPlugin {
    /// Create a new XML plugin
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    fn walk(
        &self,
        node: roxmltree::Node<'_, '_>,
        prefix: &str,
        file: &str,
        source: &str,
        results: &mut Vec<ConfigKey>,
    ) {
        if !node.is_element() {
            return;
        }
        let tag = node.tag_name().name();
        let full = if prefix.is_empty() {
            tag.to_string()
        } else {
            format!("{prefix}.{tag}")
        };

        let mut has_element_child = false;
        for child in node.children() {
            if child.is_element() {
                has_element_child = true;
                self.walk(child, &full, file, source, results);
            }
        }

        if !has_element_child {
            let text = node
                .text()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or("");
            let range = node.range();
            let (sl, sc) = line_col_at(source, range.start);
            let (el, ec) = line_col_at(source, range.end);
            results.push(ConfigKey {
                key_path: full.clone(),
                value: text.to_string(),
                value_type: ConfigValueType::String,
                location: loc(file, sl, el, sc, ec),
            });
        }

        for attr in node.attributes() {
            let key = format!("{full}.@{}", attr.name());
            let range = attr.range();
            let (sl, sc) = line_col_at(source, range.start);
            let (el, ec) = line_col_at(source, range.end);
            results.push(ConfigKey {
                key_path: key,
                value: attr.value().to_string(),
                value_type: ConfigValueType::String,
                location: loc(file, sl, el, sc, ec),
            });
        }
    }
}

impl Default for XmlPlugin {
    fn default() -> Self {
        Self::new().expect("Failed to create XmlPlugin")
    }
}

impl ConfigFormatPlugin for XmlPlugin {
    fn format_id(&self) -> &str {
        "xml"
    }

    fn file_extensions(&self) -> Vec<&str> {
        vec!["xml"]
    }

    fn extract_config_keys(&self, file_path: &Path, source: &[u8]) -> Result<Vec<ConfigKey>> {
        let file = file_path.to_string_lossy().to_string();
        let text = std::str::from_utf8(source).map_err(|e| Error::ParseError {
            file: file_path.to_path_buf(),
            line: 0,
            message: e.to_string(),
        })?;
        let doc = roxmltree::Document::parse(text).map_err(|e| Error::ParseError {
            file: file_path.to_path_buf(),
            line: 0,
            message: format!("xml parse: {e}"),
        })?;
        let mut results = Vec::new();
        if let Some(root) = doc.root().first_element_child() {
            self.walk(root, "", &file, text, &mut results);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_spans_nonzero() {
        let src = b"<config>\n  <server>\n    <port>8080</port>\n  </server>\n</config>\n";
        let plugin = XmlPlugin::new().unwrap();
        let keys = plugin
            .extract_config_keys(Path::new("config.xml"), src)
            .unwrap();
        assert!(keys.iter().any(|k| k.key_path.contains("port")));
        assert!(keys.iter().all(|k| k.location.start_line >= 1));
    }
}
