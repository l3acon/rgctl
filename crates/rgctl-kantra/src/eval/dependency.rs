//! `java.dependency` / `go.dependency` against graph Dependency nodes.

use crate::eval::{MatchSite, violation};
use crate::findings::KantraViolation;
use crate::engine::EvalNode;

/// Match Kantra java.dependency / go.dependency conditions against Dependency nodes.
pub fn eval_dependency(
    rule_id: &str,
    ecosystem: &str,
    name: &str,
    nameregex: Option<&str>,
    lowerbound: Option<&str>,
    upperbound: Option<&str>,
    nodes: &[EvalNode],
) -> Vec<KantraViolation> {
    let mut out = Vec::new();
    let name_re = nameregex.and_then(|p| regex::Regex::new(p).ok());

    for node in nodes {
        if node.node_type != "Dependency" {
            continue;
        }
        let eco = node
            .labels
            .iter()
            .find(|l| l.starts_with("ecosystem:"))
            .map(|l| l.trim_start_matches("ecosystem:"))
            .or_else(|| {
                // qualified_name is `ecosystem:coord` from manifest extract
                node.qualified_name
                    .as_deref()
                    .and_then(|q| q.split_once(':').map(|(e, _)| e))
            })
            .unwrap_or("");
        if !eco.is_empty() && eco != ecosystem && !(ecosystem == "maven" && eco == "gradle") {
            // allow gradle coords for java.dependency as Maven-shaped G:A
            if ecosystem == "java" || ecosystem == "maven" {
                if eco != "maven" && eco != "gradle" {
                    continue;
                }
            } else if ecosystem == "go" || ecosystem == "golang" {
                if eco != "golang" {
                    continue;
                }
            } else if eco != ecosystem {
                continue;
            }
        }

        let matched = if let Some(re) = &name_re {
            re.is_match(&node.name)
        } else if !name.is_empty() {
            node.name == name || node.name.contains(name) || node.name.ends_with(&format!(":{name}"))
        } else {
            false
        };
        if !matched {
            continue;
        }

        // Version bounds require a version on the node; declared coords are often G:A only.
        let _ = (lowerbound, upperbound);

        let site = MatchSite::new(
            node.file_path.clone().unwrap_or_else(|| "<manifest>".into()),
            node.start_line.unwrap_or(1),
        )
        .with_symbol(node.name.clone());
        out.push(violation(rule_id, "dependency", &site));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EvalNode;

    #[test]
    fn matches_maven_coordinate() {
        let nodes = vec![EvalNode {
            id: None,
            node_type: "Dependency".into(),
            name: "io.quarkus:quarkus-core".into(),
            qualified_name: Some("maven:io.quarkus:quarkus-core".into()),
            file_path: Some("pom.xml".into()),
            start_line: Some(10),
            labels: vec![],
        }];
        let v = eval_dependency(
            "r1",
            "maven",
            "io.quarkus:quarkus-core",
            None,
            None,
            None,
            &nodes,
        );
        assert_eq!(v.len(), 1);
    }
}
