//! Structured metrics CLI JSON response.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Current metrics JSON schema version.
pub const METRICS_SCHEMA_VERSION: u32 = 1;

/// PageRank section of the metrics payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricsPagerankSection {
    pub top: Vec<Value>,
    pub converged: bool,
    pub iterations: usize,
    pub max_delta: f64,
}

/// Community detection section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricsCommunitiesSection {
    pub count: usize,
    pub modularity: f64,
    pub assignments: usize,
}

/// Top-level metrics JSON payload (sections present based on flags).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricsJsonResponse {
    pub schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagerank: Option<MetricsPagerankSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub betweenness: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub communities: Option<MetricsCommunitiesSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub churn_hotspots: Option<Vec<Value>>,
}

/// Build a metrics response; unset sections are omitted from JSON via `Option`.
pub fn build_metrics_response(
    pagerank: Option<MetricsPagerankSection>,
    betweenness: Option<Vec<Value>>,
    communities: Option<MetricsCommunitiesSection>,
) -> MetricsJsonResponse {
    build_metrics_response_with_churn(pagerank, betweenness, communities, None)
}

/// Build a metrics response including optional git churn hotspots.
pub fn build_metrics_response_with_churn(
    pagerank: Option<MetricsPagerankSection>,
    betweenness: Option<Vec<Value>>,
    communities: Option<MetricsCommunitiesSection>,
    churn_hotspots: Option<Vec<Value>>,
) -> MetricsJsonResponse {
    MetricsJsonResponse {
        schema_version: METRICS_SCHEMA_VERSION,
        pagerank,
        betweenness,
        communities,
        churn_hotspots,
    }
}

/// Serialize a typed metrics response to JSON.
pub fn metrics_response_to_json(response: &MetricsJsonResponse) -> Value {
    serde_json::to_value(response).expect("MetricsJsonResponse serializes")
}

/// Wrap a legacy metrics object with schema version and normalize keys.
pub fn wrap_metrics_payload(payload: &mut Value) {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("schema_version".into(), json!(METRICS_SCHEMA_VERSION));
    }
}

/// Fixture with all sections populated (empty arrays / zeros).
pub fn fixture_metrics_response() -> MetricsJsonResponse {
    MetricsJsonResponse {
        schema_version: METRICS_SCHEMA_VERSION,
        pagerank: Some(MetricsPagerankSection {
            top: vec![json!({ "node": "00000000-0000-0000-0000-000000000001", "pagerank": 0.1 })],
            converged: true,
            iterations: 20,
            max_delta: 1e-9,
        }),
        betweenness: Some(vec![
            json!({ "node": "00000000-0000-0000-0000-000000000001", "score": 0.5 }),
        ]),
        communities: Some(MetricsCommunitiesSection {
            count: 2,
            modularity: 0.42,
            assignments: 10,
        }),
        churn_hotspots: None,
    }
}

pub fn fixture_metrics_json() -> Value {
    serde_json::to_value(fixture_metrics_response()).expect("metrics fixture serializes")
}
