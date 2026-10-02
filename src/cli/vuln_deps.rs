//! `rgctl vuln` and `rgctl deps` — OSV triage and dependency match (not on discover hot path).

use super::context::CliContext;
use super::OutputFormat;
use anyhow::{Context, Result};
use rgctl_security::{deps_check, triage_osv_path, DepsCheckOpts, DepsVerdict};
use std::path::PathBuf;

pub fn run_vuln_triage(ctx: &CliContext, osv: PathBuf) -> Result<()> {
    let triage = triage_osv_path(&osv).map_err(|e| anyhow::anyhow!("{e}"))?;
    if ctx.format == OutputFormat::Json {
        ctx.emit_json_value(&serde_json::to_value(&triage).context("serialize")?)?;
    } else {
        ctx.stdout_line(&format!(
            "id={} packages={} versions_array_not_authoritative={}",
            triage.id,
            triage.packages.len(),
            triage.versions_array_not_authoritative
        ))?;
        for p in &triage.packages {
            ctx.stdout_line(&format!(
                "  {} {} introduced={:?} fixed={:?}",
                p.ecosystem, p.name, p.introduced, p.fixed
            ))?;
        }
    }
    Ok(())
}

pub fn run_deps_check(
    ctx: &CliContext,
    osv: PathBuf,
    include_jars: Vec<PathBuf>,
    include_node_modules: Vec<PathBuf>,
) -> Result<()> {
    let opts = DepsCheckOpts {
        include_jars,
        include_node_modules,
        ..Default::default()
    };
    let result = deps_check(&ctx.repo, &osv, &opts).map_err(|e| anyhow::anyhow!("{e}"))?;
    if ctx.format == OutputFormat::Json {
        ctx.emit_json_value(&serde_json::to_value(&result).context("serialize")?)?;
    } else {
        let verdict = match result.verdict {
            DepsVerdict::NotAffected => "not_affected",
            DepsVerdict::AffectedCandidate => "affected_candidate",
        };
        ctx.stdout_line(&format!(
            "verdict={verdict} osv_id={} checked={:?} matched={}",
            result.osv_id,
            result.checked,
            result.matched.len()
        ))?;
        for m in &result.matched {
            ctx.stdout_line(&format!(
                "  {} {} version={:?} in_range={} src={}",
                m.ecosystem, m.name, m.version, m.in_range, m.provenance
            ))?;
        }
    }
    Ok(())
}
