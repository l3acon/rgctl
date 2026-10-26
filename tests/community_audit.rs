//! Phase 15 community detection audit — isolation, determinism, modularity.

use rgctl::analysis::{
    CommunityDetector, HubStripPolicy, PetGraphView, default_community_edge_types,
    verify_community_connectedness,
};
use rgctl::graph::CodeGraph;
use rgctl::graph::backend::GraphBackend;
use rgctl::graph::schema::{Edge, EdgeType, Node, NodeType};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn insert_call(backend: &mut rgctl::graph::backend::MemoryBackend, from: Uuid, to: Uuid) {
    backend
        .insert_edge(Edge::new(from, to, EdgeType::Calls))
        .unwrap();
}

fn build_clique(
    backend: &mut rgctl::graph::backend::MemoryBackend,
    prefix: &str,
    n: usize,
) -> Vec<Uuid> {
    let mut ids = Vec::with_capacity(n);
    for i in 0..n {
        let node = Node::new(NodeType::Function, format!("{prefix}_{i}"));
        ids.push(node.id);
        backend.insert_node(node).unwrap();
    }
    for i in 0..n {
        for j in 0..n {
            if i != j {
                insert_call(backend, ids[i], ids[j]);
            }
        }
    }
    ids
}

/// Two dense call cliques in one module folder; only `Contains` links to parent.
#[test]
fn dumbbell_coupling_contains_does_not_merge_domains() {
    let mut graph = CodeGraph::new();
    let backend = graph.backend_mut();

    let parent = Node::new(NodeType::Module, "shared_pkg");
    let id_parent = parent.id;
    backend.insert_node(parent).unwrap();

    let clique_a = build_clique(backend, "auth", 5);
    let clique_b = build_clique(backend, "billing", 5);

    for id in clique_a.iter().chain(clique_b.iter()) {
        backend
            .insert_edge(Edge::new(id_parent, *id, EdgeType::Contains))
            .unwrap();
    }

    let view = PetGraphView::from_backend(backend).unwrap();
    let result = CommunityDetector::new()
        .detect_with_view_filtered(&view, &[EdgeType::Calls])
        .unwrap();

    let mut function_labels = std::collections::HashSet::new();
    for id in clique_a.iter().chain(clique_b.iter()) {
        function_labels.insert(result.assignments[id]);
    }

    assert_eq!(
        function_labels.len(),
        2,
        "Calls-only detection must yield two behavioral communities, got {function_labels:?}"
    );

    let parent_label = *result.assignments.get(&id_parent).unwrap();
    let func_labels: Vec<usize> = clique_a
        .iter()
        .chain(clique_b.iter())
        .map(|id| result.assignments[id])
        .collect();
    assert!(
        !func_labels.contains(&parent_label),
        "module label must not subsume behavioral communities"
    );
}

/// Symmetrical tie topology must produce identical assignments across repeated runs.
#[test]
fn non_determinism_stress_100_runs_stable() {
    let mut graph = CodeGraph::new();
    let backend = graph.backend_mut();

    let left = Node::new(NodeType::Function, "left_hub");
    let right = Node::new(NodeType::Function, "right_hub");
    let center = Node::new(NodeType::Function, "center");
    let id_left = left.id;
    let id_right = right.id;
    let id_center = center.id;
    backend.insert_node(left).unwrap();
    backend.insert_node(right).unwrap();
    backend.insert_node(center).unwrap();

    insert_call(backend, id_left, id_center);
    insert_call(backend, id_right, id_center);

    let view = PetGraphView::from_backend(backend).unwrap();
    let detector = CommunityDetector::new();
    let baseline = detector
        .detect_with_view_filtered(&view, &[EdgeType::Calls])
        .unwrap();

    for run in 0..100 {
        let again = detector
            .detect_with_view_filtered(&view, &[EdgeType::Calls])
            .unwrap();
        assert_eq!(
            baseline.assignments, again.assignments,
            "assignment drift on run {run}"
        );
    }
}

/// Two disconnected dense cliques should score high modularity when correctly grouped.
#[test]
fn modularity_dense_clique_high_q() {
    let mut graph = CodeGraph::new();
    let backend = graph.backend_mut();
    let _ = build_clique(backend, "auth", 8);
    let _ = build_clique(backend, "billing", 8);

    let view = PetGraphView::from_backend(backend).unwrap();
    let result = CommunityDetector::new()
        .detect_with_view_filtered(&view, default_community_edge_types())
        .unwrap();

    assert!(
        result.modularity > 0.4,
        "partitioned dense cliques modularity too low: {}",
        result.modularity
    );
}

/// Singleton-per-node partition on a dense mesh must score below zero.
#[test]
fn modularity_singleton_partition_negative_q() {
    let mut graph = CodeGraph::new();
    let backend = graph.backend_mut();
    let ids = build_clique(backend, "mesh", 10);

    let view = PetGraphView::from_backend(backend).unwrap();
    let labels: HashMap<_, _> = view
        .index_uuid_iter()
        .enumerate()
        .map(|(i, (idx, _))| (idx, i))
        .collect();

    let q = CommunityDetector::new().calculate_modularity(&view, &labels, &[EdgeType::Calls]);
    assert!(q < 0.0, "singleton partition should yield Q < 0, got {q}");
    assert!(ids.len() == 10);
}

/// Large mock monorepo community detection must stay under 150 ms (release).
#[test]
#[ignore = "performance gate: run with `cargo test --release --test community_audit -- --ignored`"]
fn community_150k_nodes_under_150ms() {
    let graph = build_monorepo_mock(150_000, 700_000);
    let view = PetGraphView::from_backend(graph.backend()).unwrap();

    let start = Instant::now();
    CommunityDetector::new()
        .detect_with_view_filtered(&view, default_community_edge_types())
        .unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(150),
        "community detection regression: {elapsed:?}"
    );
}

fn build_monorepo_mock(nodes: usize, edges: usize) -> CodeGraph {
    let mut graph = CodeGraph::new();
    let backend = graph.backend_mut();
    let mut ids = Vec::with_capacity(nodes);
    for i in 0..nodes {
        let node = Node::new(NodeType::Function, format!("n{i}"));
        ids.push(node.id);
        backend.insert_node(node).unwrap();
    }
    for e in 0..edges {
        let from = ids[e % nodes];
        let to = ids[(e * 13 + 7) % nodes];
        if from != to {
            backend
                .insert_edge(Edge::new(from, to, EdgeType::Calls))
                .unwrap();
        }
    }
    graph
}

/// Bridge topology where greedy Louvain can strand disconnected leaves; Fast-Leiden must not.
#[test]
fn leiden_every_community_is_connected_subgraph() {
    let mut graph = CodeGraph::new();
    let backend = graph.backend_mut();

    // Two leaf groups linked only through bridge B:
    //   L0--L1--B--R0--R1  plus L0--L1 and R0--R1 dense edges.
    let mut left = Vec::new();
    let mut right = Vec::new();
    for i in 0..4 {
        let l = Node::new(NodeType::Function, format!("left_{i}"))
            .with_qualified_name(format!("pkg::left_{i}"));
        let r = Node::new(NodeType::Function, format!("right_{i}"))
            .with_qualified_name(format!("pkg::right_{i}"));
        left.push(l.id);
        right.push(r.id);
        backend.insert_node(l).unwrap();
        backend.insert_node(r).unwrap();
    }
    let bridge = Node::new(NodeType::Function, "bridge").with_qualified_name("pkg::bridge");
    let id_b = bridge.id;
    backend.insert_node(bridge).unwrap();

    for ids in [&left, &right] {
        for i in 0..ids.len() {
            for j in 0..ids.len() {
                if i != j {
                    insert_call(backend, ids[i], ids[j]);
                }
            }
        }
    }
    insert_call(backend, left[0], id_b);
    insert_call(backend, id_b, left[0]);
    insert_call(backend, right[0], id_b);
    insert_call(backend, id_b, right[0]);

    let view = PetGraphView::from_backend(backend).unwrap();
    let allowed = &[EdgeType::Calls];
    let result = CommunityDetector::new()
        .with_hub_policy(HubStripPolicy::Off)
        .detect_with_view_filtered(&view, allowed)
        .unwrap();

    verify_community_connectedness(&view, &result, allowed)
        .expect("every non-infra community must induce a connected subgraph");
}

/// Shared Logger hub with in-degree 30 must land in infrastructure_community_id.
#[test]
fn statistical_hub_stripping_isolates_logger_hub() {
    let mut graph = CodeGraph::new();
    let backend = graph.backend_mut();

    let domains = ["auth", "billing", "inventory"];
    let mut domain_ids: Vec<Vec<Uuid>> = Vec::new();
    for domain in domains {
        domain_ids.push(build_clique(backend, domain, 10));
    }

    let logger =
        Node::new(NodeType::Function, "Logger::info").with_qualified_name("util::Logger::info");
    let id_logger = logger.id;
    backend.insert_node(logger).unwrap();

    for clique in &domain_ids {
        for &id in clique {
            insert_call(backend, id, id_logger);
        }
    }

    let view = PetGraphView::from_backend(backend).unwrap();
    let result = CommunityDetector::new()
        .with_hub_policy(HubStripPolicy::Statistical { k: 3.0 })
        .with_min_nodes_for_hub_strip(20)
        .detect_with_view_filtered(&view, &[EdgeType::Calls])
        .unwrap();

    let infra = result
        .infrastructure_community_id
        .expect("Logger::info must become an infrastructure hub");
    assert_eq!(
        result.assignments.get(&id_logger).copied(),
        Some(infra),
        "Logger::info must sit in infrastructure_community_id"
    );

    let mut domain_labels = HashSet::new();
    for clique in &domain_ids {
        let labels: HashSet<_> = clique.iter().map(|id| result.assignments[id]).collect();
        assert_eq!(
            labels.len(),
            1,
            "each domain clique must share one community label, got {labels:?}"
        );
        domain_labels.extend(labels);
    }
    domain_labels.remove(&infra);
    assert_eq!(
        domain_labels.len(),
        3,
        "auth/billing/inventory must remain three separate communities, got {domain_labels:?}"
    );
}

/// Community partitions by qualified_name must be invariant under insertion order.
#[test]
fn node_insertion_order_invariance() {
    fn build_with_seed(seed: u64) -> (CodeGraph, f64, BTreeSet<BTreeSet<String>>) {
        let mut names: Vec<String> = (0..8)
            .map(|i| format!("auth_{i}"))
            .chain((0..8).map(|i| format!("billing_{i}")))
            .collect();
        // Deterministic LCG Fisher–Yates.
        let mut state = seed;
        for i in (1..names.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (state as usize) % (i + 1);
            names.swap(i, j);
        }

        let mut graph = CodeGraph::new();
        let backend = graph.backend_mut();
        let mut id_by_name = HashMap::new();
        for name in &names {
            let node =
                Node::new(NodeType::Function, name.clone()).with_qualified_name(name.clone());
            id_by_name.insert(name.clone(), node.id);
            backend.insert_node(node).unwrap();
        }
        for prefix in ["auth", "billing"] {
            for i in 0..8 {
                for j in 0..8 {
                    if i == j {
                        continue;
                    }
                    let a = id_by_name[&format!("{prefix}_{i}")];
                    let b = id_by_name[&format!("{prefix}_{j}")];
                    insert_call(backend, a, b);
                }
            }
        }

        let view = PetGraphView::from_backend(backend).unwrap();
        let result = CommunityDetector::new()
            .with_hub_policy(HubStripPolicy::Off)
            .detect_with_view_filtered(&view, &[EdgeType::Calls])
            .unwrap();

        let mut by_label: HashMap<usize, BTreeSet<String>> = HashMap::new();
        for (id, label) in &result.assignments {
            let node = backend.get_node(*id).unwrap().unwrap();
            let qname = node
                .qualified_name
                .as_deref()
                .unwrap_or(node.name.as_str())
                .to_string();
            by_label.entry(*label).or_default().insert(qname);
        }
        let partition: BTreeSet<BTreeSet<String>> = by_label.into_values().collect();
        (graph, result.modularity, partition)
    }

    let (_g0, q0, p0) = build_with_seed(1);
    for seed in [2u64, 3, 5, 7, 11] {
        let (_g, q, p) = build_with_seed(seed);
        assert_eq!(p, p0, "partition drift at seed {seed}");
        assert!(
            (q - q0).abs() < 1e-9,
            "modularity drift at seed {seed}: {q} vs {q0}"
        );
    }
}
