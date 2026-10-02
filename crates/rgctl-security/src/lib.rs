//! Security analysis for rgctl.
//!
//! - Existing: CWE pattern analyzer (to be unified with declarative taint rules — issue #16).
//! - OSV supply-chain: triage via the [`osv`](https://crates.io/crates/osv) schema crate.
//!   OpenVEX emission (later) should use [`openvex`](https://crates.io/crates/openvex).
//!
//! Policy: [AGENTS.md](../../../AGENTS.md) ·
//! [openspec/changes/_shared/starting-context.md](../../../openspec/changes/_shared/starting-context.md).
//! OSV/deps/bundled JAR scans MUST NOT run on default discover.

pub mod adapters;
pub mod analyzer;
pub mod cve_patterns;
pub mod deps_check;
pub mod error;
pub mod osv;
pub mod package_resolve;
pub mod sink_taint;
pub mod version;
pub mod vuln_analyze;

pub use adapters::{scan_jars, scan_manifests, scan_node_modules, BundledScanOpts, DepHit};
pub use analyzer::{SecurityAnalyzer, SecurityVulnerability};
pub use cve_patterns::{default_cwe_patterns, CwePattern};
pub use deps_check::{deps_check, DepsCheckOpts, DepsCheckResult, DepsMatch, DepsVerdict};
pub use error::SecurityError;
pub use osv::{
    triage_from_vulnerability, triage_osv_bytes, triage_osv_path, OsvPackageTriage, OsvTriage,
};
pub use package_resolve::{resolve_package, PackageResolution, PackageResolveError};
pub use sink_taint::{
    build_sink_first_result, missing_cfg_error_message, rules_with_osv_overlays,
    SinkFirstTaintResult, SinkPathSummary, SinkResolution,
};
pub use version::{engine_for_ecosystem, VersionEngine};
pub use vuln_analyze::{
    affected_methods_from_osv_json, vuln_analyze, BundledPresence, ExploitabilityVerdict,
    VulnAnalyzeOpts, VulnAnalyzeResult, VULN_ANALYZE_SCHEMA_VERSION,
};
