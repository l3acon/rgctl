//! Security analysis: CWE patterns and vulnerability reporting.

pub mod analyzer;
pub mod cve_patterns;
pub mod error;
pub mod osv;

pub use analyzer::{SecurityAnalyzer, SecurityVulnerability};
pub use cve_patterns::{default_cwe_patterns, CwePattern};
pub use error::SecurityError;
pub use osv::{
    triage_from_vulnerability, triage_osv_bytes, triage_osv_path, OsvPackageTriage, OsvTriage,
};
