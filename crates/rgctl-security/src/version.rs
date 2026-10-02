//! Ecosystem version comparison for OSV range membership.
//!
//! Sync-only APIs (safe under `spawn_blocking`). Not on the discover hot path.

use crate::error::SecurityError;

/// Pluggable version ordering / range membership.
pub trait VersionEngine: Send + Sync {
    fn ecosystem_id(&self) -> &'static str;
    /// Compare `a` vs `b`: Less = a < b, etc.
    fn cmp_versions(&self, a: &str, b: &str) -> Result<std::cmp::Ordering, SecurityError>;
    /// True if `version` is in `[introduced, fixed)` (fixed exclusive when present).
    fn in_vulnerable_range(
        &self,
        version: &str,
        introduced: Option<&str>,
        fixed: Option<&str>,
    ) -> Result<bool, SecurityError> {
        if let Some(intro) = introduced {
            if intro != "0" && self.cmp_versions(version, intro)? == std::cmp::Ordering::Less {
                return Ok(false);
            }
        }
        if let Some(fix) = fixed {
            if self.cmp_versions(version, fix)? != std::cmp::Ordering::Less {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Resolve an engine from OSV / PURL ecosystem labels.
pub fn engine_for_ecosystem(label: &str) -> Result<&'static dyn VersionEngine, SecurityError> {
    let lower = label.to_ascii_lowercase();
    if lower.starts_with("maven") {
        return Ok(&MavenEngine);
    }
    if lower == "crates.io" || lower == "cargo" {
        return Ok(&CargoEngine);
    }
    if lower == "npm" || lower == "javascript" {
        return Ok(&NpmEngine);
    }
    if lower == "go" || lower == "golang" {
        return Ok(&GoEngine);
    }
    Err(SecurityError::UnsupportedEcosystem(label.to_string()))
}

struct MavenEngine;
struct CargoEngine;
struct NpmEngine;
struct GoEngine;

impl VersionEngine for MavenEngine {
    fn ecosystem_id(&self) -> &'static str {
        "Maven"
    }
    fn cmp_versions(&self, a: &str, b: &str) -> Result<std::cmp::Ordering, SecurityError> {
        Ok(cmp_maven(a, b))
    }
}

impl VersionEngine for CargoEngine {
    fn ecosystem_id(&self) -> &'static str {
        "crates.io"
    }
    fn cmp_versions(&self, a: &str, b: &str) -> Result<std::cmp::Ordering, SecurityError> {
        Ok(cmp_semver_like(a, b))
    }
}

impl VersionEngine for NpmEngine {
    fn ecosystem_id(&self) -> &'static str {
        "npm"
    }
    fn cmp_versions(&self, a: &str, b: &str) -> Result<std::cmp::Ordering, SecurityError> {
        Ok(cmp_semver_like(a, b))
    }
}

impl VersionEngine for GoEngine {
    fn ecosystem_id(&self) -> &'static str {
        "Go"
    }
    fn cmp_versions(&self, a: &str, b: &str) -> Result<std::cmp::Ordering, SecurityError> {
        let a = a.strip_prefix('v').unwrap_or(a);
        let b = b.strip_prefix('v').unwrap_or(b);
        Ok(cmp_semver_like(a, b))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MavenToken {
    nums: Vec<u64>,
    qualifier: String,
    vendor: String,
}

fn parse_maven(v: &str) -> MavenToken {
    // Split vendor suffix like -rhlw-00004 from the rest.
    let (core, vendor) = if let Some(idx) = v.find("-rhlw-") {
        (&v[..idx], v[idx + 1..].to_string())
    } else if let Some(idx) = v.find("-redhat-") {
        (&v[..idx], v[idx + 1..].to_string())
    } else {
        (v, String::new())
    };
    let mut nums = Vec::new();
    let mut qualifier = String::new();
    for part in core.split(|c| c == '.' || c == '-') {
        if part.is_empty() {
            continue;
        }
        if let Ok(n) = part.parse::<u64>() {
            nums.push(n);
        } else {
            qualifier = part.to_ascii_lowercase();
        }
    }
    MavenToken {
        nums,
        qualifier,
        vendor,
    }
}

fn qualifier_rank(q: &str) -> i32 {
    match q {
        "" | "ga" | "final" | "release" => 0,
        "sp" => 1,
        "rc" => -1,
        "milestone" | "m" => -2,
        "alpha" | "a" | "beta" | "b" | "snapshot" => -3,
        _ => -1,
    }
}

fn cmp_maven(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let aa = parse_maven(a);
    let bb = parse_maven(b);
    let len = aa.nums.len().max(bb.nums.len());
    for i in 0..len {
        let x = aa.nums.get(i).copied().unwrap_or(0);
        let y = bb.nums.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    match qualifier_rank(&aa.qualifier).cmp(&qualifier_rank(&bb.qualifier)) {
        Ordering::Equal => {}
        o => return o,
    }
    // Exact equality including vendor: same build is equal.
    // Vendor suffix present vs absent: treat vendor build as > base of same nums/qualifier
    // only when comparing for "fixed" exclusivity — equal strings already handled.
    aa.vendor.cmp(&bb.vendor)
}

fn cmp_semver_like(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let parse = |s: &str| -> (Vec<u64>, String) {
        let (core, pre) = s.split_once('-').unwrap_or((s, ""));
        let nums: Vec<u64> = core
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        (nums, pre.to_string())
    };
    let (an, ap) = parse(a);
    let (bn, bp) = parse(b);
    let len = an.len().max(bn.len());
    for i in 0..len {
        let x = an.get(i).copied().unwrap_or(0);
        let y = bn.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    match (ap.is_empty(), bp.is_empty()) {
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ => ap.cmp(&bp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_jackson_range() {
        let e = engine_for_ecosystem("Maven").unwrap();
        assert!(e
            .in_vulnerable_range("2.13.5", Some("2.0.0"), Some("2.14.0"))
            .unwrap());
        assert!(!e
            .in_vulnerable_range("2.14.0", Some("2.0.0"), Some("2.14.0"))
            .unwrap());
    }

    #[test]
    fn maven_vendor_fixed_exact() {
        let e = engine_for_ecosystem("Maven").unwrap();
        let fixed = "2.7.2.Final-rhlw-00004";
        assert!(!e
            .in_vulnerable_range(fixed, Some("0"), Some(fixed))
            .unwrap());
        assert!(e
            .in_vulnerable_range("2.7.2.Final", Some("0"), Some(fixed))
            .unwrap());
    }

    #[test]
    fn cargo_and_npm() {
        let c = engine_for_ecosystem("crates.io").unwrap();
        assert!(c
            .in_vulnerable_range("1.2.3", Some("1.0.0"), Some("1.3.0"))
            .unwrap());
        let n = engine_for_ecosystem("npm").unwrap();
        assert!(!n
            .in_vulnerable_range("2.0.0", Some("1.0.0"), Some("2.0.0"))
            .unwrap());
    }

    #[test]
    fn unsupported() {
        assert!(engine_for_ecosystem("MadeUpEco").is_err());
    }
}
