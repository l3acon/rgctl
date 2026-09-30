//! Config usage detector
//!
//! Detect when code references configuration keys / env vars.

use crate::graph_builder::ConfigUsageKind;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

static RUST_ENV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"env::var(?:_os)?\("([^"]+)"\)"#).unwrap());
static RUST_CONFIG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\.get\("([^"]+)"\)"#).unwrap());
static PYTHON_ENV_BRACKET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"os\.environ\[['"]([^'"]+)['"]\]"#).unwrap());
static PYTHON_GETENV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"os\.getenv\(['"]([^'"]+)['"]\)"#).unwrap());
static JS_DOT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"process\.env\.([A-Z0-9_]+)"#).unwrap());
static JS_BRACKET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"process\.env\[['"]([^'"]+)['"]\]"#).unwrap());
static GO_GETENV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"os\.Getenv\("([^"]+)"\)"#).unwrap());

static JAVA_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"@Value\s*\(\s*(?:value\s*=\s*)?["']\$\{([^}:'\"]+)(?::[^"']*)?\}["']"#).unwrap()
});
static JAVA_CONFIG_PROPERTY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"@ConfigProperty\s*\([^)]*name\s*=\s*["']([^"']+)["']"#).unwrap()
});
static JAVA_GETENV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"System\.getenv\s*\(\s*["']([^"']+)["']\s*\)"#).unwrap());
static JAVA_GETPROP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"System\.getProperty\s*\(\s*["']([^"']+)["']"#).unwrap());

static CSHARP_INDEXER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\[["']([^"']+)["']\]"#).unwrap());
static CSHARP_GETSECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"GetSection\s*\(\s*["']([^"']+)["']\s*\)"#).unwrap());
static CSHARP_GETVALUE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"GetValue\s*(?:<[^>]+>)?\s*\(\s*["']([^"']+)["']"#).unwrap());

/// Confidence level for a detected config usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigConfidence {
    /// Directly extracted from source (e.g. string literal)
    Extracted,
    /// Inferred from context
    Inferred,
    /// Ambiguous match
    Ambiguous,
}

/// A detected configuration or environment variable usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigUsage {
    /// Config key or environment variable name
    pub key: String,
    /// Source file path
    pub file: String,
    /// Line number (1-indexed)
    pub line: usize,
    /// Usage kind
    pub usage_type: ConfigUsageKind,
    /// Detection confidence
    pub confidence: ConfigConfidence,
}

/// Detects configuration and environment variable references in source code.
pub struct ConfigUsageDetector;

impl ConfigUsageDetector {
    /// Detect config usages for a supported language.
    pub fn detect(language_id: &str, source: &[u8], file_path: &Path) -> Vec<ConfigUsage> {
        match language_id {
            "rust" | "python" | "typescript" | "javascript" | "go" | "java" | "csharp" => {}
            _ => return Vec::new(),
        }

        let source = String::from_utf8_lossy(source);
        let file = file_path.to_string_lossy().to_string();

        match language_id {
            "rust" => Self::detect_rust(&source, &file),
            "python" => Self::detect_python(&source, &file),
            "typescript" | "javascript" => Self::detect_javascript(&source, &file),
            "go" => Self::detect_go(&source, &file),
            "java" => Self::detect_java(&source, &file),
            "csharp" => Self::detect_csharp(&source, &file),
            _ => Vec::new(),
        }
    }

    /// Normalize a config key for matching (strip defaults already done; Spring relaxed form).
    pub fn normalize_key(key: &str) -> String {
        key.trim()
            .replace(['-', '_'], ".")
            .to_ascii_lowercase()
    }

    fn detect_rust(source: &str, file: &str) -> Vec<ConfigUsage> {
        let mut usages = Vec::new();

        for (idx, line) in source.lines().enumerate() {
            for cap in RUST_ENV_RE.captures_iter(line) {
                usages.push(ConfigUsage {
                    key: cap[1].to_string(),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::EnvVar,
                    confidence: ConfigConfidence::Extracted,
                });
            }
            for cap in RUST_CONFIG_RE.captures_iter(line) {
                usages.push(ConfigUsage {
                    key: cap[1].to_string(),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::ConfigKey,
                    confidence: ConfigConfidence::Inferred,
                });
            }
        }
        usages
    }

    fn detect_python(source: &str, file: &str) -> Vec<ConfigUsage> {
        let mut usages = Vec::new();
        for (idx, line) in source.lines().enumerate() {
            for cap in PYTHON_ENV_BRACKET_RE
                .captures_iter(line)
                .chain(PYTHON_GETENV_RE.captures_iter(line))
            {
                usages.push(ConfigUsage {
                    key: cap[1].to_string(),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::EnvVar,
                    confidence: ConfigConfidence::Extracted,
                });
            }
        }
        usages
    }

    fn detect_javascript(source: &str, file: &str) -> Vec<ConfigUsage> {
        let mut usages = Vec::new();
        for (idx, line) in source.lines().enumerate() {
            for cap in JS_DOT_RE
                .captures_iter(line)
                .chain(JS_BRACKET_RE.captures_iter(line))
            {
                usages.push(ConfigUsage {
                    key: cap[1].to_string(),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::EnvVar,
                    confidence: ConfigConfidence::Extracted,
                });
            }
        }
        usages
    }

    fn detect_go(source: &str, file: &str) -> Vec<ConfigUsage> {
        let mut usages = Vec::new();
        for (idx, line) in source.lines().enumerate() {
            for cap in GO_GETENV_RE.captures_iter(line) {
                usages.push(ConfigUsage {
                    key: cap[1].to_string(),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::EnvVar,
                    confidence: ConfigConfidence::Extracted,
                });
            }
        }
        usages
    }

    fn detect_java(source: &str, file: &str) -> Vec<ConfigUsage> {
        let mut usages = Vec::new();
        for (idx, line) in source.lines().enumerate() {
            for cap in JAVA_VALUE_RE.captures_iter(line) {
                usages.push(ConfigUsage {
                    key: cap[1].to_string(),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::ConfigKey,
                    confidence: ConfigConfidence::Extracted,
                });
            }
            for cap in JAVA_CONFIG_PROPERTY_RE.captures_iter(line) {
                usages.push(ConfigUsage {
                    key: cap[1].to_string(),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::ConfigKey,
                    confidence: ConfigConfidence::Extracted,
                });
            }
            for cap in JAVA_GETENV_RE.captures_iter(line) {
                usages.push(ConfigUsage {
                    key: cap[1].to_string(),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::EnvVar,
                    confidence: ConfigConfidence::Extracted,
                });
            }
            for cap in JAVA_GETPROP_RE.captures_iter(line) {
                usages.push(ConfigUsage {
                    key: cap[1].to_string(),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::ConfigKey,
                    confidence: ConfigConfidence::Extracted,
                });
            }
        }
        usages
    }

    fn detect_csharp(source: &str, file: &str) -> Vec<ConfigUsage> {
        let mut usages = Vec::new();
        for (idx, line) in source.lines().enumerate() {
            let looks_config = line.contains("Configuration")
                || line.contains("IConfiguration")
                || line.contains("GetSection")
                || line.contains("GetValue")
                || line.contains("_config")
                || line.contains("configuration");
            if !looks_config {
                continue;
            }
            for cap in CSHARP_GETSECTION_RE
                .captures_iter(line)
                .chain(CSHARP_GETVALUE_RE.captures_iter(line))
            {
                usages.push(ConfigUsage {
                    key: cap[1].replace(':', "."),
                    file: file.to_string(),
                    line: idx + 1,
                    usage_type: ConfigUsageKind::ConfigKey,
                    confidence: ConfigConfidence::Extracted,
                });
            }
            for cap in CSHARP_INDEXER_RE.captures_iter(line) {
                let key = cap[1].replace(':', ".");
                if key.contains('.') || key.contains("Connection") {
                    usages.push(ConfigUsage {
                        key,
                        file: file.to_string(),
                        line: idx + 1,
                        usage_type: ConfigUsageKind::ConfigKey,
                        confidence: ConfigConfidence::Inferred,
                    });
                }
            }
        }
        usages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_rust_config_detection() {
        let source = br#"
        fn main() {
            let host = env::var("DB_HOST").unwrap();
            let pool_size = config.get("database.pool_size").unwrap();
        }
        "#;

        let usages = ConfigUsageDetector::detect("rust", source, Path::new("main.rs"));
        assert!(
            usages
                .iter()
                .any(|u| u.key == "DB_HOST" && u.usage_type == ConfigUsageKind::EnvVar)
        );
        assert!(usages.iter().any(|u| u.key == "database.pool_size"));
    }

    #[test]
    fn test_python_config_detection() {
        let source = br#"
import os
host = os.environ['DB_HOST']
port = os.getenv('DB_PORT')
"#;

        let usages = ConfigUsageDetector::detect("python", source, Path::new("app.py"));
        assert!(usages.iter().any(|u| u.key == "DB_HOST"));
        assert!(usages.iter().any(|u| u.key == "DB_PORT"));
    }

    #[test]
    fn test_javascript_config_detection() {
        let source = br#"const x = process.env.API_KEY; const y = process.env['DB_HOST'];"#;
        let usages = ConfigUsageDetector::detect("javascript", source, Path::new("app.js"));
        assert!(usages.iter().any(|u| u.key == "API_KEY"));
        assert!(usages.iter().any(|u| u.key == "DB_HOST"));
    }

    #[test]
    fn test_c_returns_empty() {
        let src = b"getenv(\"HOME\");";
        let usages = ConfigUsageDetector::detect("c", src, Path::new("main.c"));
        assert!(usages.is_empty());
    }

    #[test]
    fn java_value_and_config_property() {
        let src = br#"
@Value("${app.jwt.secret}")
String secret;
@ConfigProperty(name = "quarkus.datasource.jdbc.url")
String url;
System.getenv("PATH");
"#;
        let usages = ConfigUsageDetector::detect("java", src, Path::new("App.java"));
        assert!(usages.iter().any(|u| u.key == "app.jwt.secret"));
        assert!(usages.iter().any(|u| u.key == "quarkus.datasource.jdbc.url"));
        assert!(usages.iter().any(|u| u.key == "PATH"));
    }

    #[test]
    fn csharp_get_section() {
        let src = br#"var x = configuration.GetSection("ConnectionStrings:Default");"#;
        let usages = ConfigUsageDetector::detect("csharp", src, Path::new("Startup.cs"));
        assert!(usages.iter().any(|u| u.key.contains("ConnectionStrings")));
    }
}
