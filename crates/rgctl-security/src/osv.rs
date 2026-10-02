//! OSV triage — parse with the stable [`osv`](https://crates.io/crates/osv) schema crate,
//! then project an agent-facing JSON envelope.
//!
//! Policy: [AGENTS.md](../../../../AGENTS.md) · starting context:
//! [openspec/changes/_shared/starting-context.md](../../../../openspec/changes/_shared/starting-context.md).
//! This module does **not** run on the discover hot path.
//!
//! When emitting OpenVEX later, prefer the [`openvex`](https://crates.io/crates/openvex) types crate
//! rather than hand-rolled VEX JSON.

use osv::schema::{Event, Vulnerability};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

use crate::error::SecurityError;

/// Normalized OSV triage payload (agent JSON contract).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsvTriage {
    pub schema_version: u32,
    pub command: &'static str,
    pub id: String,
    pub aliases: Vec<String>,
    pub summary: Option<String>,
    pub packages: Vec<OsvPackageTriage>,
    /// Honesty: OSV `versions[]` is not authoritative when `ranges` exist (backport series).
    pub versions_array_not_authoritative: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsvPackageTriage {
    pub ecosystem: String,
    pub name: String,
    pub purl: Option<String>,
    pub introduced: Option<String>,
    pub fixed: Option<String>,
    pub listed_versions: Vec<String>,
    pub affected_methods: Vec<String>,
    pub affected_classes: Vec<String>,
}

/// Parse OSV JSON bytes via `osv::schema::Vulnerability`, then normalize for agents.
pub fn triage_osv_bytes(bytes: &[u8]) -> Result<OsvTriage, SecurityError> {
    let doc: Vulnerability =
        serde_json::from_slice(bytes).map_err(|e| SecurityError::OsvParse(e.to_string()))?;
    Ok(triage_from_vulnerability(doc))
}

/// Load and triage an OSV JSON file.
pub fn triage_osv_path(path: &Path) -> Result<OsvTriage, SecurityError> {
    let bytes = fs::read(path).map_err(|e| SecurityError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    triage_osv_bytes(&bytes)
}

/// Project a parsed [`Vulnerability`] into the agent triage envelope.
pub fn triage_from_vulnerability(doc: Vulnerability) -> OsvTriage {
    let mut packages = Vec::new();
    let mut any_versions = false;
    let mut any_ranges = false;

    for aff in doc.affected.unwrap_or_default() {
        let listed = aff.versions.clone().unwrap_or_default();
        if !listed.is_empty() {
            any_versions = true;
        }
        let mut introduced = None;
        let mut fixed = None;
        for range in aff.ranges.unwrap_or_default() {
            any_ranges = true;
            for ev in range.events {
                match ev {
                    Event::Introduced(v) => introduced = Some(v),
                    Event::Fixed(v) => fixed = Some(v),
                    Event::LastAffected(_) | Event::Limit(_) => {}
                    _ => {}
                }
            }
        }
        let (affected_methods, affected_classes) =
            extract_ecosystem_specific(aff.ecosystem_specific.as_ref());
        let (ecosystem, name, purl) = match aff.package {
            Some(pkg) => (
                ecosystem_label(&pkg.ecosystem),
                pkg.name,
                pkg.purl,
            ),
            None => ("unknown".into(), String::new(), None),
        };
        packages.push(OsvPackageTriage {
            ecosystem,
            name,
            purl,
            introduced,
            fixed,
            listed_versions: listed,
            affected_methods,
            affected_classes,
        });
    }

    OsvTriage {
        schema_version: 1,
        command: "vuln triage",
        id: doc.id,
        aliases: doc.aliases.unwrap_or_default(),
        summary: doc.summary,
        packages,
        versions_array_not_authoritative: any_versions && any_ranges,
    }
}

fn extract_ecosystem_specific(v: Option<&Value>) -> (Vec<String>, Vec<String>) {
    let Some(obj) = v.and_then(|x| x.as_object()) else {
        return (Vec::new(), Vec::new());
    };
    let methods = obj
        .get("affected_methods")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let classes = obj
        .get("affected_classes")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    (methods, classes)
}

fn ecosystem_label(eco: &osv::schema::Ecosystem) -> String {
    serde_json::to_value(eco)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{eco:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triage_jackson_fixture() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/osv/jackson-CVE-2022-42004.json");
        let t = triage_osv_path(&p).expect("triage");
        assert!(t.aliases.iter().any(|a| a.contains("CVE-2022-42004")));
        assert_eq!(t.packages.len(), 1);
        let pkg = &t.packages[0];
        assert_eq!(pkg.name, "com.fasterxml.jackson.core:jackson-databind");
        assert_eq!(pkg.introduced.as_deref(), Some("2.0.0"));
        assert_eq!(pkg.fixed.as_deref(), Some("2.14.0"));
        assert!(pkg.affected_methods.iter().any(|m| m.contains("readValue")));
        assert!(!t.versions_array_not_authoritative);
    }

    #[test]
    fn triage_xalan_backport_honesty() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/osv/xalan-CVE-2022-34169.json");
        let t = triage_osv_path(&p).expect("triage");
        assert!(t.versions_array_not_authoritative);
        let pkg = &t.packages[0];
        assert_eq!(pkg.fixed.as_deref(), Some("2.7.2.Final-rhlw-00004"));
        assert!(pkg
            .listed_versions
            .contains(&"2.7.2.Final-rhlw-00004".into()));
    }
}
