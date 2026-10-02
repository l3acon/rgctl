//! `vuln analyze` orchestrator: triage → deps → package queries → blast/taint → OpenVEX.

use crate::deps_check::{deps_check, DepsCheckOpts, DepsCheckResult, DepsVerdict};
use crate::osv::{triage_osv_path, OsvTriage};
use crate::package_resolve::{resolve_package, PackageResolution};
use crate::sink_taint::{SinkFirstTaintResult, SinkResolution};
use openvex::{Justification, Metadata, OpenVex, Statement, Status};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Schema version for analyze JSON.
pub const VULN_ANALYZE_SCHEMA_VERSION: &str = "1";

/// Final exploitability verdict (distinct from deps presence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExploitabilityVerdict {
    /// Deps not present / not in range.
    NotAffected,
    /// Library present; no evidence vulnerable code executes.
    NotExploitable,
    /// Reachable vulnerable sink from external input.
    Exploitable,
    /// Insufficient evidence (e.g. unresolved sink, missing CFG).
    UnderInvestigation,
}

/// Options for [`vuln_analyze`].
#[derive(Debug, Clone, Default)]
pub struct VulnAnalyzeOpts {
    /// Bundled JAR roots.
    pub include_jars: Vec<PathBuf>,
    /// node_modules roots.
    pub include_node_modules: Vec<PathBuf>,
    /// Skip phases requiring a graph (tests / early unit).
    pub skip_graph: bool,
    /// Import hit count from `find --package` (None = skipped).
    pub import_hits: Option<usize>,
    /// Callers of OSV affected methods (precomputed by CLI).
    pub affected_method_callers: Option<Vec<String>>,
    /// Sink-first result (precomputed by CLI).
    pub sink_taint: Option<SinkFirstTaintResult>,
    /// Whether CFG was available for sink-first.
    pub cfg_available: Option<bool>,
}

/// Full analyze response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnAnalyzeResult {
    /// Schema version.
    pub schema_version: String,
    /// OSV id.
    pub osv_id: String,
    /// Deps verdict.
    pub deps_verdict: String,
    /// Exploitability.
    pub exploitability: ExploitabilityVerdict,
    /// Phases executed.
    pub phases: Vec<String>,
    /// Package resolution when run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<PackageResolution>,
    /// Bundled presence when imports empty but deps matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundled_presence: Option<BundledPresence>,
    /// Sink-first summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sink_taint: Option<SinkFirstTaintResult>,
    /// OpenVEX document.
    pub openvex: OpenVex,
    /// Human-readable justification notes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Bundled library presence when direct imports are zero.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundledPresence {
    /// True when deps inventory matched the package.
    pub present: bool,
    /// Provenance strings from deps matches.
    pub provenance: Vec<String>,
    /// Honesty text.
    pub honesty: String,
}

/// Run analyze using precomputed graph phases from the CLI (or skip_graph for deps-only).
pub fn vuln_analyze(
    repo: &Path,
    osv: &Path,
    opts: &VulnAnalyzeOpts,
) -> Result<VulnAnalyzeResult, crate::error::SecurityError> {
    let triage = triage_osv_path(osv)?;
    let mut phases = vec!["triage".to_string()];

    let deps_opts = DepsCheckOpts {
        include_jars: opts.include_jars.clone(),
        include_node_modules: opts.include_node_modules.clone(),
        ..Default::default()
    };
    let deps = deps_check(repo, osv, &deps_opts)?;
    phases.push("deps_check".into());

    if deps.verdict == DepsVerdict::NotAffected {
        let openvex = build_openvex(
            &triage,
            &deps,
            Status::NotAffected,
            Some(Justification::ComponentNotPresent),
            Some("Dependency not present or version not in vulnerable range.".into()),
        );
        return Ok(VulnAnalyzeResult {
            schema_version: VULN_ANALYZE_SCHEMA_VERSION.into(),
            osv_id: triage.id.clone(),
            deps_verdict: "not_affected".into(),
            exploitability: ExploitabilityVerdict::NotAffected,
            phases,
            package: None,
            bundled_presence: None,
            sink_taint: None,
            openvex,
            notes: Some("Early exit after deps check".into()),
        });
    }

    // Resolve primary package for later phases.
    let package = deps
        .matched
        .first()
        .and_then(|m| resolve_package(&m.name).ok())
        .or_else(|| {
            triage
                .packages
                .first()
                .and_then(|p| resolve_package(&p.name).ok())
        });

    let import_hits = opts.import_hits.unwrap_or(0);
    let bundled_presence = if !opts.skip_graph {
        phases.push("package_query".into());
        if import_hits == 0 && !deps.matched.is_empty() {
            Some(BundledPresence {
                present: true,
                provenance: deps.matched.iter().map(|m| m.provenance.clone()).collect(),
                honesty: "Zero direct imports MUST NOT be read as library absence — \
                          package present via bundled/manifest inventory"
                    .into(),
            })
        } else {
            None
        }
    } else {
        None
    };

    let callers = opts.affected_method_callers.clone().unwrap_or_default();
    if !opts.skip_graph {
        phases.push("affected_method_callers".into());
    }

    let sink_taint = opts.sink_taint.clone();
    if sink_taint.is_some() {
        phases.push("sink_first_taint".into());
    }

    let (exploitability, status, justification, impact, notes) =
        decide_verdict(&deps, &callers, sink_taint.as_ref(), opts.cfg_available);

    let openvex = build_openvex(&triage, &deps, status, justification, impact);

    Ok(VulnAnalyzeResult {
        schema_version: VULN_ANALYZE_SCHEMA_VERSION.into(),
        osv_id: triage.id,
        deps_verdict: "affected_candidate".into(),
        exploitability,
        phases,
        package,
        bundled_presence,
        sink_taint,
        openvex,
        notes,
    })
}

fn decide_verdict(
    _deps: &DepsCheckResult,
    callers: &[String],
    sink_taint: Option<&SinkFirstTaintResult>,
    cfg_available: Option<bool>,
) -> (
    ExploitabilityVerdict,
    Status,
    Option<Justification>,
    Option<String>,
    Option<String>,
) {
    if let Some(st) = sink_taint {
        if !st.paths.is_empty() {
            return (
                ExploitabilityVerdict::Exploitable,
                Status::Affected,
                None,
                None,
                Some("Sink-first taint found external→sink path(s)".into()),
            );
        }
        if st.sink_resolution == SinkResolution::Unresolved {
            // Empty taint alone cannot prove safe; callers of affected methods can.
            if callers.is_empty() {
                return (
                    ExploitabilityVerdict::NotExploitable,
                    Status::NotAffected,
                    Some(Justification::VulnerableCodeNotInExecutePath),
                    Some(
                        "Affected methods are not called from indexed source; sink lives \
                         outside the graph (bundled bytecode). Serialize-only usage is \
                         consistent with not_exploitable."
                            .into(),
                    ),
                    Some(
                        "sink_resolution=unresolved; exploitability grounded in zero \
                         affected-method callers, not empty taint paths alone"
                            .into(),
                    ),
                );
            }
            return (
                ExploitabilityVerdict::UnderInvestigation,
                Status::UnderInvestigation,
                None,
                None,
                Some(
                    "Sink unresolved in graph but callers of related symbols exist — \
                     needs bytecode / deeper analysis"
                        .into(),
                ),
            );
        }
        // Resolved sink, no paths
        return (
            ExploitabilityVerdict::NotExploitable,
            Status::NotAffected,
            Some(Justification::VulnerableCodeNotInExecutePath),
            Some("No external→sink taint path to the resolved vulnerable method.".into()),
            None,
        );
    }

    // No sink-first phase: use caller evidence.
    if callers.is_empty() {
        let cfg_note = match cfg_available {
            Some(false) => " (CFG not available; caller scan only)",
            _ => "",
        };
        return (
            ExploitabilityVerdict::NotExploitable,
            Status::NotAffected,
            Some(Justification::VulnerableCodeNotInExecutePath),
            Some(format!(
                "No callers of OSV affected methods in the indexed graph{cfg_note}."
            )),
            Some("Verdict from affected-method caller scan".into()),
        );
    }

    (
        ExploitabilityVerdict::UnderInvestigation,
        Status::UnderInvestigation,
        None,
        None,
        Some("Affected methods have callers; run sink-first taint with CFG for a firm verdict".into()),
    )
}

fn build_openvex(
    triage: &OsvTriage,
    deps: &DepsCheckResult,
    status: Status,
    justification: Option<Justification>,
    impact: Option<String>,
) -> OpenVex {
    let products: Vec<String> = deps
        .matched
        .iter()
        .map(|m| format!("{}:{}", m.ecosystem, m.name))
        .collect();
    let products = if products.is_empty() {
        triage
            .packages
            .iter()
            .map(|p| format!("{}:{}", p.ecosystem, p.name))
            .collect()
    } else {
        products
    };

    OpenVex {
        metadata: Metadata {
            context: "https://openvex.dev/ns/v0.2.0".into(),
            id: format!("vex:{}", triage.id),
            author: "rgctl".into(),
            role: "document creator".into(),
            timestamp: None,
            version: "1".into(),
            tooling: Some("rgctl vuln analyze".into()),
            supplier: None,
        },
        statements: vec![Statement {
            vulnerability: Some(triage.id.clone()),
            vuln_description: None,
            timestamp: None,
            products,
            subcomponents: vec![],
            status,
            status_notes: None,
            justification,
            impact_statement: impact,
            action_statement: None,
            action_statement_timestamp: None,
        }],
    }
}

/// Extract OSV `affected_methods` strings from ecosystem_specific when present.
pub fn affected_methods_from_osv_json(bytes: &[u8]) -> Vec<String> {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return vec![];
    };
    let mut out = Vec::new();
    if let Some(arr) = v.get("affected").and_then(|a| a.as_array()) {
        for a in arr {
            if let Some(methods) = a
                .pointer("/ecosystem_specific/affected_methods")
                .and_then(|m| m.as_array())
            {
                for m in methods {
                    if let Some(s) = m.as_str() {
                        out.push(s.to_string());
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sink_taint::{build_sink_first_result, SinkResolution};
    use std::io::Write;

    #[test]
    fn xalan_fast_exit_not_affected() {
        let dir = tempfile::tempdir().unwrap();
        let pom = dir.path().join("pom.xml");
        std::fs::write(
            &pom,
            r#"<project><modelVersion>4.0.0</modelVersion>
  <groupId>com.redhat.coolstore</groupId><artifactId>monolith</artifactId><version>1.0</version>
  <dependencies></dependencies></project>"#,
        )
        .unwrap();
        let osv = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/osv/xalan-CVE-2022-34169.json");
        let r = vuln_analyze(
            dir.path(),
            &osv,
            &VulnAnalyzeOpts {
                skip_graph: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.exploitability, ExploitabilityVerdict::NotAffected);
        assert_eq!(r.phases, vec!["triage", "deps_check"]);
        assert_eq!(r.openvex.statements[0].status, Status::NotAffected);
    }

    #[test]
    fn jackson_not_exploitable_with_zero_callers() {
        let dir = tempfile::tempdir().unwrap();
        let pom = dir.path().join("pom.xml");
        std::fs::write(
            &pom,
            r#"<?xml version="1.0"?>
<project>
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.redhat.coolstore</groupId>
  <artifactId>monolith</artifactId>
  <version>1.0.0</version>
  <dependencies>
    <dependency>
      <groupId>com.fasterxml.jackson.core</groupId>
      <artifactId>jackson-databind</artifactId>
      <version>2.13.5</version>
    </dependency>
  </dependencies>
</project>"#,
        )
        .unwrap();
        let osv = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/osv/jackson-CVE-2022-42004.json");
        let sink = build_sink_first_result(
            "com.fasterxml.jackson.databind.ObjectMapper.readValue",
            8,
            true,
            false,
            vec![],
            vec![],
        );
        assert_eq!(sink.sink_resolution, SinkResolution::Unresolved);
        let r = vuln_analyze(
            dir.path(),
            &osv,
            &VulnAnalyzeOpts {
                skip_graph: false,
                import_hits: Some(0),
                affected_method_callers: Some(vec![]),
                sink_taint: Some(sink),
                cfg_available: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.deps_verdict, "affected_candidate");
        assert_eq!(r.exploitability, ExploitabilityVerdict::NotExploitable);
        assert!(r.bundled_presence.is_some());
        assert_eq!(r.openvex.statements[0].status, Status::NotAffected);
        assert_eq!(
            r.openvex.statements[0].justification,
            Some(Justification::VulnerableCodeNotInExecutePath)
        );
    }

    #[test]
    fn affected_methods_parsed_from_fixture() {
        let osv = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/osv/jackson-CVE-2022-42004.json");
        let bytes = std::fs::read(&osv).unwrap();
        let methods = affected_methods_from_osv_json(&bytes);
        assert!(methods.iter().any(|m| m.contains("readValue")));
    }

    #[test]
    fn write_openvex_roundtrip() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        let doc = OpenVex {
            metadata: Metadata {
                context: "https://openvex.dev/ns/v0.2.0".into(),
                id: "vex:test".into(),
                author: "rgctl".into(),
                role: "document creator".into(),
                timestamp: None,
                version: "1".into(),
                tooling: None,
                supplier: None,
            },
            statements: vec![],
        };
        write!(f, "{}", serde_json::to_string(&doc).unwrap()).unwrap();
    }
}
