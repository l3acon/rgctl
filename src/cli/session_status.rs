//! Session graph status: cheap freshness / presence check (no rediscover).

use super::context::CliContext;
use super::OutputFormat;
use anyhow::Result;
use serde::Serialize;
use std::path::Path;

const STATUS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
struct SessionStatus {
    schema_version: u32,
    command: &'static str,
    /// `ok` | `missing`
    status: &'static str,
    repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nodes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edges: Option<usize>,
    kantra_findings: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    kantra_findings_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

fn kantra_findings_path(repo: &Path) -> std::path::PathBuf {
    rgctl_graph::paths::artifact_path(repo, "kantra_findings.json")
}

/// `rgctl status` — report whether `.rgctl/` has a usable snapshot.
pub fn run_status(ctx: &CliContext) -> Result<()> {
    let findings = kantra_findings_path(&ctx.repo);
    let kantra_present = findings.is_file();
    let kantra_path = kantra_present.then(|| findings.display().to_string());

    let session = ctx.snapshot_session()?;
    let payload = match session {
        Some(s) => SessionStatus {
            schema_version: STATUS_SCHEMA_VERSION,
            command: "status",
            status: "ok",
            repo: ctx.repo.display().to_string(),
            snapshot: Some(
                rgctl_graph::paths::artifact_path(
                    &ctx.repo,
                    rgctl_graph::snapshot::SNAPSHOT_FILE,
                )
                .display()
                .to_string(),
            ),
            digest: Some(s.digest.to_string()),
            nodes: Some(s.store.node_count()),
            edges: Some(s.store.edge_count()),
            kantra_findings: kantra_present,
            kantra_findings_path: kantra_path,
            message: None,
        },
        None => SessionStatus {
            schema_version: STATUS_SCHEMA_VERSION,
            command: "status",
            status: "missing",
            repo: ctx.repo.display().to_string(),
            snapshot: None,
            digest: None,
            nodes: None,
            edges: None,
            kantra_findings: kantra_present,
            kantra_findings_path: kantra_path,
            message: Some("Graph snapshot not found; run `rgctl discover` first".into()),
        },
    };

    if ctx.format == OutputFormat::Json {
        let v = serde_json::to_value(&payload)?;
        ctx.emit_json_value(&v)?;
    } else if payload.status == "ok" {
        ctx.stdout_line(&format!(
            "status=ok nodes={} edges={} digest={}{}",
            payload.nodes.unwrap_or(0),
            payload.edges.unwrap_or(0),
            payload.digest.as_deref().unwrap_or("?"),
            if payload.kantra_findings {
                " kantra_findings=yes"
            } else {
                ""
            }
        ))?;
    } else {
        ctx.stdout_line(
            payload
                .message
                .as_deref()
                .unwrap_or("Graph snapshot missing"),
        )?;
    }

    if payload.status == "missing" {
        anyhow::bail!("graph snapshot missing");
    }
    Ok(())
}
