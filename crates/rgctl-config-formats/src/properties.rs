//! Java properties / INI-style configuration plugin (span-accurate).

use rgctl_plugin_api::{ConfigKey, ConfigValueType, Error, Result, SourceLocation};
use rgctl_plugin_api::ConfigFormatPlugin;
use std::path::Path;

/// Properties file config format plugin
pub struct PropertiesPlugin;

impl PropertiesPlugin {
    /// Create a new properties plugin
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl ConfigFormatPlugin for PropertiesPlugin {
    fn format_id(&self) -> &str {
        "properties"
    }

    fn file_extensions(&self) -> Vec<&str> {
        vec!["properties", "ini"]
    }

    fn extract_config_keys(&self, file_path: &Path, source: &[u8]) -> Result<Vec<ConfigKey>> {
        let file = file_path.to_string_lossy().to_string();
        let text = std::str::from_utf8(source).map_err(|e| Error::ParseError {
            file: file_path.to_path_buf(),
            line: 0,
            message: e.to_string(),
        })?;

        let mut keys = Vec::new();
        let mut logical = String::new();
        let mut logical_start_line = 1usize;
        let mut logical_start_col = 1usize;
        let mut pending_continuation = false;

        for (line_idx, raw_line) in text.lines().enumerate() {
            let line_no = line_idx + 1;
            // Preserve leading spaces for column math on the physical line.
            let line_for_col = raw_line;
            let trimmed_start = raw_line.trim_start();
            let leading = raw_line.len() - trimmed_start.len();

            if !pending_continuation {
                if trimmed_start.is_empty()
                    || trimmed_start.starts_with('#')
                    || trimmed_start.starts_with('!')
                    || trimmed_start.starts_with(';')
                {
                    continue;
                }
                // INI section headers — skip for key/value flatten (documented honesty).
                if trimmed_start.starts_with('[') && trimmed_start.contains(']') {
                    continue;
                }
                logical.clear();
                logical_start_line = line_no;
                logical_start_col = leading + 1;
            }

            let mut content = trimmed_start;

            let cont = content.ends_with('\\')
                && !content.ends_with("\\\\")
                && content.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1;
            if cont {
                content = &content[..content.len() - 1];
                logical.push_str(content);
                pending_continuation = true;
                continue;
            }
            logical.push_str(content);
            pending_continuation = false;

            let Some((key, value, key_end_col)) = split_property(&logical) else {
                continue;
            };
            let end_col = logical_start_col + key_end_col.saturating_sub(1);
            keys.push(ConfigKey {
                key_path: key,
                value,
                value_type: ConfigValueType::String,
                location: SourceLocation {
                    file: file.clone(),
                    start_line: logical_start_line,
                    end_line: line_no,
                    start_column: logical_start_col,
                    end_column: end_col.max(logical_start_col),
                },
            });
            let _ = line_for_col; // column base already from leading whitespace
        }

        Ok(keys)
    }
}

/// Split on first unescaped `=` or `:` (Java properties). Returns (key, value, key_end_1based_col_in_logical).
fn split_property(logical: &str) -> Option<(String, String, usize)> {
    let bytes = logical.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'=' | b':' => {
                let key = logical[..i].trim().to_string();
                if key.is_empty() {
                    return None;
                }
                let value = logical[i + 1..].trim().to_string();
                // 1-based column of delimiter within logical string (approx key end).
                let key_end = i + 1;
                return Some((key, value, key_end));
            }
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_properties_parsing() {
        let source = b"# comment\nserver.port=8080\ndb.host=localhost\n";
        let plugin = PropertiesPlugin::new().unwrap();
        let keys = plugin
            .extract_config_keys(Path::new("app.properties"), source)
            .unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.key_path == "server.port"));
        let port = keys.iter().find(|k| k.key_path == "server.port").unwrap();
        assert!(port.location.start_line >= 1);
        assert!(port.location.start_column >= 1);
    }

    #[test]
    fn colon_delimiter_and_continuation() {
        let source = b"server.port: 8080\nlong.value=foo\\\nbar\n";
        let plugin = PropertiesPlugin::new().unwrap();
        let keys = plugin
            .extract_config_keys(Path::new("app.properties"), source)
            .unwrap();
        assert!(keys.iter().any(|k| k.key_path == "server.port" && k.value == "8080"));
        let long = keys.iter().find(|k| k.key_path == "long.value").unwrap();
        assert_eq!(long.value, "foobar");
        assert!(long.location.start_line >= 1);
        assert!(long.location.end_line >= long.location.start_line);
    }
}
