//! Boundary classification catalogs for blast-radius `--classify-boundary`.
//!
//! Data-driven annotation/attribute/name patterns → [`BoundaryKind`].

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Stable boundary kinds (JSON schema).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BoundaryKind {
    /// HTTP / REST endpoint.
    RestEndpoint,
    /// Message-driven / queue consumer.
    MessagingConsumer,
    /// Scheduled / cron job.
    ScheduledJob,
    /// CLI entrypoint.
    CliEntrypoint,
    /// Test code.
    Test,
    /// Internal / unclassified.
    Internal,
}

impl BoundaryKind {
    /// Parse allowlist token (kind name or catalog alias).
    pub fn parse_filter(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "REST_ENDPOINT" | "REST" | "HTTP" | "POST" | "GET" => Some(Self::RestEndpoint),
            "MESSAGING_CONSUMER" | "MESSAGING" | "MDB" | "JMS" => Some(Self::MessagingConsumer),
            "SCHEDULED_JOB" | "SCHEDULED" | "CRON" => Some(Self::ScheduledJob),
            "CLI_ENTRYPOINT" | "CLI" => Some(Self::CliEntrypoint),
            "TEST" => Some(Self::Test),
            "INTERNAL" => Some(Self::Internal),
            _ => None,
        }
    }
}

/// One catalog match rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryRule {
    /// Rule id.
    pub id: String,
    /// Resulting kind.
    pub kind: BoundaryKind,
    /// Annotation / attribute simple or FQN substrings.
    #[serde(default)]
    pub annotations: Vec<String>,
    /// Symbol / qualified-name substrings.
    #[serde(default)]
    pub name_contains: Vec<String>,
    /// File path substrings (e.g. `/test/`).
    #[serde(default)]
    pub path_contains: Vec<String>,
    /// Optional HTTP method filter token (POST, GET, …) for allowlists.
    #[serde(default)]
    pub http_methods: Vec<String>,
}

/// Language boundary catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryCatalog {
    /// Catalog id.
    pub id: String,
    /// Languages this catalog applies to.
    pub languages: Vec<String>,
    /// Ordered rules (first match wins).
    pub rules: Vec<BoundaryRule>,
}

/// Classification result for one impact node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryLabel {
    /// Symbol name.
    pub name: String,
    /// Optional qualified name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    /// File path when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Classified kind.
    pub kind: BoundaryKind,
    /// Matching rule id.
    pub rule_id: String,
}

/// Inputs for classifying a symbol.
#[derive(Debug, Clone, Default)]
pub struct BoundaryNodeRef<'a> {
    /// Display name.
    pub name: &'a str,
    /// Qualified name.
    pub qualified_name: Option<&'a str>,
    /// File path.
    pub file: Option<&'a str>,
    /// Language id.
    pub language: Option<&'a str>,
    /// Annotation names on the symbol / enclosing type.
    pub annotations: &'a [String],
}

/// Classify nodes with loaded catalogs; optional kind/method allowlist.
pub fn classify_boundaries(
    catalogs: &[BoundaryCatalog],
    nodes: &[BoundaryNodeRef<'_>],
    allowlist: Option<&[String]>,
) -> Vec<BoundaryLabel> {
    let filters = allowlist.map(|a| {
        a.iter()
            .filter_map(|s| BoundaryKind::parse_filter(s))
            .collect::<HashSet<_>>()
    });
    let method_tokens: HashSet<String> = allowlist
        .unwrap_or(&[])
        .iter()
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| matches!(s.as_str(), "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD"))
        .collect();

    let mut out = Vec::new();
    for node in nodes {
        let lang = node.language.unwrap_or("");
        let mut labeled = None;
        for cat in catalogs {
            if !cat.languages.is_empty()
                && !lang.is_empty()
                && !cat
                    .languages
                    .iter()
                    .any(|l| l.eq_ignore_ascii_case(lang) || (lang == "typescript" && l == "javascript"))
            {
                continue;
            }
            for rule in &cat.rules {
                if rule_matches(rule, node) {
                    labeled = Some((rule.kind, rule.id.clone(), rule));
                    break;
                }
            }
            if labeled.is_some() {
                break;
            }
        }
        let (kind, rule_id, _rule) = match labeled {
            Some((k, id, r)) => (k, id, Some(r)),
            None => (BoundaryKind::Internal, "default-internal".into(), None),
        };
        if let Some(ref f) = filters {
            if !f.contains(&kind) {
                continue;
            }
        }
        if !method_tokens.is_empty() {
            let mut node_methods = HashSet::new();
            for a in node.annotations {
                let u = a.to_ascii_uppercase();
                match u.as_str() {
                    "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" => {
                        node_methods.insert(u);
                    }
                    _ if u.contains("POSTMAPPING") || u == "POST" => {
                        node_methods.insert("POST".into());
                    }
                    _ if u.contains("GETMAPPING") => {
                        node_methods.insert("GET".into());
                    }
                    _ if u.contains("PUTMAPPING") => {
                        node_methods.insert("PUT".into());
                    }
                    _ if u.contains("DELETEMAPPING") => {
                        node_methods.insert("DELETE".into());
                    }
                    _ => {}
                }
            }
            if node_methods.is_empty() || node_methods.is_disjoint(&method_tokens) {
                continue;
            }
        }
        if kind == BoundaryKind::Internal && filters.is_some() {
            continue;
        }
        out.push(BoundaryLabel {
            name: node.name.to_string(),
            qualified_name: node.qualified_name.map(str::to_string),
            file: node.file.map(str::to_string),
            kind,
            rule_id,
        });
    }
    out
}

fn rule_matches(rule: &BoundaryRule, node: &BoundaryNodeRef<'_>) -> bool {
    let qn = node.qualified_name.unwrap_or("");
    let file = node.file.unwrap_or("");
    if !rule.annotations.is_empty() {
        let hit = rule.annotations.iter().any(|a| {
            node.annotations
                .iter()
                .any(|n| n.contains(a.as_str()) || a.contains(n.as_str()))
        });
        if hit {
            return true;
        }
    }
    if !rule.name_contains.is_empty() {
        let hit = rule
            .name_contains
            .iter()
            .any(|p| node.name.contains(p.as_str()) || qn.contains(p.as_str()));
        if hit {
            return true;
        }
    }
    if !rule.path_contains.is_empty() {
        return rule.path_contains.iter().any(|p| file.contains(p.as_str()));
    }
    false
}

/// Bundled Java/Jakarta + Python FastAPI/Flask catalogs.
pub fn bundled_boundary_catalogs() -> Vec<BoundaryCatalog> {
    vec![java_jakarta_catalog(), python_web_catalog()]
}

fn java_jakarta_catalog() -> BoundaryCatalog {
    BoundaryCatalog {
        id: "java-jakarta".into(),
        languages: vec!["java".into(), "kotlin".into(), "groovy".into()],
        rules: vec![
            BoundaryRule {
                id: "jaxrs-path".into(),
                kind: BoundaryKind::RestEndpoint,
                annotations: vec![
                    "Path".into(),
                    "GET".into(),
                    "POST".into(),
                    "PUT".into(),
                    "DELETE".into(),
                    "PATCH".into(),
                    "RequestMapping".into(),
                    "GetMapping".into(),
                    "PostMapping".into(),
                    "RestController".into(),
                ],
                name_contains: vec![],
                path_contains: vec![],
                http_methods: vec!["GET".into(), "POST".into(), "PUT".into(), "DELETE".into()],
            },
            BoundaryRule {
                id: "post-only".into(),
                kind: BoundaryKind::RestEndpoint,
                annotations: vec!["POST".into(), "PostMapping".into()],
                name_contains: vec![],
                path_contains: vec![],
                http_methods: vec!["POST".into()],
            },
            BoundaryRule {
                id: "mdb-jms".into(),
                kind: BoundaryKind::MessagingConsumer,
                annotations: vec![
                    "MessageDriven".into(),
                    "JmsListener".into(),
                    "RabbitListener".into(),
                    "KafkaListener".into(),
                ],
                name_contains: vec!["MDB".into(), "MessageBean".into()],
                path_contains: vec![],
                http_methods: vec![],
            },
            BoundaryRule {
                id: "scheduled".into(),
                kind: BoundaryKind::ScheduledJob,
                annotations: vec!["Scheduled".into(), "Timeout".into()],
                name_contains: vec![],
                path_contains: vec![],
                http_methods: vec![],
            },
            BoundaryRule {
                id: "main-cli".into(),
                kind: BoundaryKind::CliEntrypoint,
                annotations: vec![],
                name_contains: vec!["main".into()],
                path_contains: vec![],
                http_methods: vec![],
            },
            BoundaryRule {
                id: "test-path".into(),
                kind: BoundaryKind::Test,
                annotations: vec!["Test".into(), "ParameterizedTest".into()],
                name_contains: vec![],
                path_contains: vec!["/test/".into(), "/tests/".into()],
                http_methods: vec![],
            },
        ],
    }
}

fn python_web_catalog() -> BoundaryCatalog {
    BoundaryCatalog {
        id: "python-web".into(),
        languages: vec!["python".into()],
        rules: vec![
            BoundaryRule {
                id: "fastapi-flask".into(),
                kind: BoundaryKind::RestEndpoint,
                annotations: vec![],
                name_contains: vec![
                    "@app.route".into(),
                    "@router.".into(),
                    "APIRouter".into(),
                    "FastAPI".into(),
                    "@app.get".into(),
                    "@app.post".into(),
                ],
                path_contains: vec!["/routers".into(), "/routes".into()],
                http_methods: vec!["GET".into(), "POST".into()],
            },
            BoundaryRule {
                id: "celery-consumer".into(),
                kind: BoundaryKind::MessagingConsumer,
                annotations: vec![],
                name_contains: vec!["@celery.task".into(), "@shared_task".into()],
                path_contains: vec![],
                http_methods: vec![],
            },
            BoundaryRule {
                id: "pytest".into(),
                kind: BoundaryKind::Test,
                annotations: vec![],
                name_contains: vec!["test_".into()],
                path_contains: vec!["/tests/".into(), "/test_".into()],
                http_methods: vec![],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_vs_messaging() {
        let cats = bundled_boundary_catalogs();
        let anns_rest = vec!["Path".into(), "GET".into()];
        let anns_mdb = vec!["MessageDriven".into()];
        let nodes = vec![
            BoundaryNodeRef {
                name: "getCart",
                qualified_name: Some("com.example.CartEndpoint.getCart"),
                file: Some("src/CartEndpoint.java"),
                language: Some("java"),
                annotations: &anns_rest,
            },
            BoundaryNodeRef {
                name: "onMessage",
                qualified_name: Some("com.example.OrderMDB.onMessage"),
                file: Some("src/OrderMDB.java"),
                language: Some("java"),
                annotations: &anns_mdb,
            },
        ];
        let labels = classify_boundaries(&cats, &nodes, None);
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].kind, BoundaryKind::RestEndpoint);
        assert_eq!(labels[1].kind, BoundaryKind::MessagingConsumer);
    }

    #[test]
    fn post_allowlist_filters_get() {
        let cats = bundled_boundary_catalogs();
        let get_anns = vec!["GET".into(), "Path".into()];
        let post_anns = vec!["POST".into(), "Path".into()];
        let nodes = vec![
            BoundaryNodeRef {
                name: "list",
                qualified_name: None,
                file: None,
                language: Some("java"),
                annotations: &get_anns,
            },
            BoundaryNodeRef {
                name: "create",
                qualified_name: None,
                file: None,
                language: Some("java"),
                annotations: &post_anns,
            },
        ];
        let labels = classify_boundaries(&cats, &nodes, Some(&["POST".into()]));
        assert!(labels.iter().all(|l| l.name == "create"));
    }

    #[test]
    fn python_catalog_loads() {
        let cats = bundled_boundary_catalogs();
        assert!(cats.iter().any(|c| c.id == "python-web"));
        let nodes = [BoundaryNodeRef {
            name: "get_products",
            qualified_name: Some("app.coolstore.routers.get_products"),
            file: Some("app/coolstore/routers.py"),
            language: Some("python"),
            annotations: &[],
        }];
        let labels = classify_boundaries(&cats, &nodes, None);
        assert_eq!(labels[0].kind, BoundaryKind::RestEndpoint);
    }
}
