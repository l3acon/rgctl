//! CWE / OWASP vulnerability patterns (Phase 13.5).
//!
//! Defaults are loaded from the declarative catalog in
//! `rgctl-analysis/rules/taint/cwe-catalog.yaml` (see #16).

use rgctl_analysis::taint_rules::bundled_cwe_catalog;
use serde::{Deserialize, Serialize};

/// Common Weakness Enumeration pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CwePattern {
    /// CWE id (e.g. CWE-89).
    pub cwe_id: String,
    /// Short name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Severity 1–10.
    pub severity: u8,
    /// Source regex patterns.
    pub source_patterns: Vec<String>,
    /// Sink regex patterns.
    pub sink_patterns: Vec<String>,
    /// Sanitizer regex patterns.
    pub sanitizer_patterns: Vec<String>,
}

/// Built-in OWASP Top 10 oriented patterns (from declarative CWE catalog).
pub fn default_cwe_patterns() -> Vec<CwePattern> {
    bundled_cwe_catalog()
        .unwrap_or_default()
        .into_iter()
        .map(|e| CwePattern {
            cwe_id: e.cwe_id,
            name: e.name,
            description: e.description,
            severity: e.severity,
            source_patterns: e.source_patterns,
            sink_patterns: e.sink_patterns,
            sanitizer_patterns: e.sanitizer_patterns,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_patterns_cover_owasp() {
        let patterns = default_cwe_patterns();
        assert!(patterns.len() >= 10);
        assert!(patterns.iter().any(|p| p.cwe_id == "CWE-89"));
        assert!(patterns.iter().any(|p| p.cwe_id == "CWE-79"));
    }
}
