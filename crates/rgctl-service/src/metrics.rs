//! PageRank / betweenness / community metrics.

use crate::command::MetricsArgs;
use crate::error::{Result, ServiceError};
use crate::metrics_json::{
    MetricsCommunitiesSection, MetricsPagerankSection, build_metrics_response, metrics_response_to_json,
};
use rgctl_analysis::{
    BetweennessCentrality, CommunityDetector, FastPageRank, NodeRole, PetGraphView,
    classify_node_role, community_edge_types_for_backend, default_behavioral_edges,
    filter_scores_by_role,
};
use rgctl_graph::CodeGraph;
use rgctl_graph::backend::GraphBackend;
use serde_json::{Value, json};
use uuid::Uuid;

fn enrich_top_entry(graph: &CodeGraph, id: Uuid, score_key: &str, score: f64) -> Value {
    let mut obj = json!({
        "node": id.to_string(),
        score_key: score,
    });
    if let Ok(Some(n)) = graph.backend().get_node(id) {
        obj["name"] = json!(n.name.to_string());
        obj["kind"] = json!(format!("{:?}", n.node_type).to_lowercase());
        if let Some(qn) = n.qualified_name.as_ref() {
            obj["qualified_name"] = json!(qn.to_string());
        }
        if let Some(fp) = n.file_path.as_ref() {
            obj["file_path"] = json!(fp.to_string());
        }
    }
    obj
}

/// Compute requested metric sections.
pub fn run_metrics(graph: &CodeGraph, args: &MetricsArgs) -> Result<Value> {
    if !args.pagerank && !args.betweenness && !args.communities {
        return Err(ServiceError::InvalidParams(
            "at least one of pagerank, betweenness, communities is required".into(),
        ));
    }
    let view = PetGraphView::from_backend(graph.backend()).map_err(ServiceError::from)?;
    let iterations = 20;
    let allowed = default_behavioral_edges();
    let mut pagerank = None;
    let mut betweenness = None;
    let mut communities = None;

    if args.pagerank {
        let engine = FastPageRank::new(iterations, 0.85);
        let (scores, stats) = engine.compute(&view, allowed);
        let backend = graph.backend();
        let mut top = filter_scores_by_role(&scores, args.include_tests, |id| {
            backend
                .get_node(id)
                .ok()
                .flatten()
                .map(|n| {
                    let stub = n.get_property("is_external_stub") == Some("true");
                    let name = n
                        .qualified_name
                        .as_deref()
                        .unwrap_or(n.name.as_str());
                    classify_node_role(name, n.file_path.as_deref(), stub)
                })
                .unwrap_or(NodeRole::Production)
        });
        if top.is_empty() {
            top = scores.iter().map(|(id, score)| (*id, *score)).collect();
            top.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        }
        top.truncate(20);
        let top = top
            .iter()
            .map(|(id, score)| enrich_top_entry(graph, *id, "pagerank", *score))
            .collect();
        pagerank = Some(MetricsPagerankSection {
            top,
            converged: stats.converged,
            iterations: stats.iterations_run,
            max_delta: stats.max_delta,
        });
    }

    if args.betweenness {
        let bc = BetweennessCentrality::compute_unbounded(&view, allowed);
        let mut top: Vec<_> = bc.iter().map(|(id, score)| (id, *score)).collect();
        top.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        top.truncate(20);
        betweenness = Some(
            top.iter()
                .map(|(id, s)| enrich_top_entry(graph, **id, "score", *s))
                .collect(),
        );
    }

    if args.communities {
        let detector = CommunityDetector::new();
        let allowed_comm = community_edge_types_for_backend(graph.backend());
        let result = detector
            .detect_with_view_filtered(&view, &allowed_comm)
            .map_err(ServiceError::from)?;
        communities = Some(MetricsCommunitiesSection {
            count: result.communities.len(),
            modularity: result.modularity,
            assignments: result.assignments.len(),
        });
    }

    Ok(metrics_response_to_json(&build_metrics_response(
        pagerank,
        betweenness,
        communities,
    )))
}
