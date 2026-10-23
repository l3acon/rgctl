//! Centrality performance benchmarks with hard regression gates.
//!
//! Run: `cargo bench --bench centrality_benchmarks`
//! Large gate: `cargo test --release --test centrality_audit -- --ignored`

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use rgctl::analysis::{
    FastPageRank, FlatGraphIndex, PetGraphView, SEMANTIC_INDEX_SCHEMA_VERSION, SIGN_HASH_MODEL_ID,
    SampledBetweenness, SemanticEntry, SemanticIndex, hamming_top_k,
};
use rgctl::graph::CodeGraph;
use rgctl::graph::backend::GraphBackend;
use rgctl::graph::schema::{Edge, EdgeType, Node, NodeType};
use std::path::Path;
use std::time::{Duration, Instant};
use uuid::Uuid;

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

fn bench_petgraph_view_centrality_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("petgraph_view_centrality_large");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));

    let graph = build_monorepo_mock(150_000, 700_000);
    let view = PetGraphView::from_backend(graph.backend()).unwrap();
    let index = FlatGraphIndex::from_view(&view, &[EdgeType::Calls]);
    let engine = FastPageRank::new(20, 0.85);

    group.throughput(Throughput::Elements(index.flat_edges.len() as u64));
    group.bench_function("pagerank_150k_700k_calls", |b| {
        b.iter(|| {
            black_box(engine.compute_flat(&index));
        });
    });

    // Hard gate embedded in bench warmup path (fails loudly on regression).
    let start = Instant::now();
    engine.compute_flat(&index);
    let latency = start.elapsed();
    assert!(
        latency < Duration::from_millis(10),
        "FastPageRank 150k/700k regression: {latency:?}"
    );

    group.finish();
}

fn bench_sampled_betweenness(c: &mut Criterion) {
    let mut group = c.benchmark_group("sampled_betweenness");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));

    let graph = build_monorepo_mock(20_000, 80_000);
    let view = PetGraphView::from_backend(graph.backend()).unwrap();
    let index = FlatGraphIndex::from_view(&view, &[EdgeType::Calls]);

    group.throughput(Throughput::Elements(index.node_count as u64));
    group.bench_function("sampled_betweenness_20k_64pivots", |b| {
        b.iter(|| {
            black_box(SampledBetweenness::compute_flat(&index, 64, 42));
        });
    });

    let large = build_monorepo_mock(150_000, 700_000);
    let large_view = PetGraphView::from_backend(large.backend()).unwrap();
    let large_index = FlatGraphIndex::from_view(&large_view, &[EdgeType::Calls]);
    group.bench_function("sampled_betweenness_150k_256pivots", |b| {
        b.iter(|| {
            black_box(SampledBetweenness::compute_flat(&large_index, 256, 42));
        });
    });
    let start = Instant::now();
    SampledBetweenness::compute_flat(&large_index, 256, 42);
    let latency = start.elapsed();
    assert!(
        latency < Duration::from_millis(250),
        "SampledBetweenness 150k/k=256 regression: {latency:?}"
    );

    group.finish();
}

fn bench_hamming_top_k(c: &mut Criterion) {
    let mut group = c.benchmark_group("hamming_top_k");
    group.sample_size(20);

    let dims = 256;
    let rows = 50_000;
    let stride = dims / 8;
    let mut binary = vec![0u8; rows * stride];
    for (i, byte) in binary.iter_mut().enumerate() {
        *byte = (i % 251) as u8;
    }
    let index = SemanticIndex {
        schema_version: SEMANTIC_INDEX_SCHEMA_VERSION,
        model_id: SIGN_HASH_MODEL_ID.into(),
        dimensions: dims,
        graph_digest: None,
        model_path: None,
        tokenizer_path: None,
        entries: (0..rows)
            .map(|i| SemanticEntry {
                node_id: Uuid::from_u128(i as u128 + 1),
                name: format!("f{i}"),
                qualified_name: None,
                file_path: None,
                code_hash: None,
                node_type: None,
                kind: None,
                semantic_tags: None,
            })
            .collect(),
        binary_embeddings: binary,
    };
    let query = vec![0u8; stride];

    group.throughput(Throughput::Elements(rows as u64));
    group.bench_function("hamming_top_k_50k_k10", |b| {
        b.iter(|| black_box(hamming_top_k(&index, &query, 10)));
    });
    group.finish();
}

fn bench_kafka_centrality_optional(c: &mut Criterion) {
    let kafka_path = Path::new("example/kafka");
    if !kafka_path.exists() {
        return;
    }

    use rgctl_pipeline::{PipelineConfig, ProcessingPipeline};
    use rgctl_registry::LanguageRegistry;
    use std::sync::Arc;

    let registry = Arc::new(LanguageRegistry::new());
    let pipeline = ProcessingPipeline::with_config(
        registry,
        PipelineConfig {
            show_progress: false,
            ..PipelineConfig::default()
        },
    );
    let (graph, _) = pipeline
        .process_repository(kafka_path)
        .expect("kafka index");
    let view = PetGraphView::from_backend(graph.backend()).unwrap();
    let engine = FastPageRank::new(20, 0.85);

    c.bench_function("kafka_behavioral_pagerank", |b| {
        b.iter(|| black_box(engine.compute(&view, &[EdgeType::Calls])));
    });
}

criterion_group!(
    benches,
    bench_petgraph_view_centrality_large,
    bench_sampled_betweenness,
    bench_hamming_top_k,
    bench_kafka_centrality_optional,
);
criterion_main!(benches);
