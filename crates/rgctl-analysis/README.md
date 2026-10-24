# rgctl-analysis

Graph analysis algorithms for [rgctl](https://github.com/sshaaf/rgctl): CSR centrality (pull-based PageRank, sampled Brandes betweenness, HyperBall harmonic), Fast-Leiden community detection, SCC bitset blast radius, cross-language MRO & override resolution, SCIP compiler-index overlay, git co-change churn, lexical-scope-aware CFG/PDG construction, lazy v3 archive slicing, field-sensitive taint analysis, exact/Bloom/SESE+1-WL clone detection, architectural policy & PR review gates, and binary-quantized hybrid semantic search.

See [docs/analysis-architecture.md](../../docs/analysis-architecture.md) for the three-tier graph model.

## Module index

### Core Topology, Centrality & Community Detection

| Module | Graph input | Algorithm | Complexity |
|--------|-------------|-----------|------------|
| `graph_utils` | Backend / `SnapshotNodeStore` | CSR topology projection (`PetGraphView`), bounded BFS (`TraversalConfig`) | $O(V+E)$ |
| `callgraph` | Backend / view | Compact `u32` call adjacency (`CallGraph`) | $O(V+E)$ |
| `structural_topology` | Snapshot / view | Structural containment & module hierarchy projection | $O(V+E)$ |
| `centrality` | `PetGraphView` / `FlatGraphIndex` | Pull-based CSR `FastPageRank` (double-buffered, adaptive tolerance), degree centrality, dual-track behavioral (`Calls`) vs structural (`STRUCTURAL_EDGE_TYPES`), `NodeRole` filtering (`Production`, `Test`, `Generated`, `ExternalStub`, `Entrypoint`) | $O(k \cdot E)$ PageRank ($<10\text{ ms}$ at 150K/700K) |
| `centrality_approx` | `FlatGraphIndex` | `SampledBetweenness` ($K=256$ degree-stratified Brandes pivots; sequential $<4{,}096$ nodes, Rayon `fold`/`reduce` above) and `HyperBallHarmonic` ($p=6$ HyperLogLog distance balls, `--with-harmonic`) | $O(K \cdot (V+E))$ BC; $O(R \cdot E)$ HyperBall |
| `community` | `PetGraphView` / `FlatGraphIndex` | **Fast-Leiden** (local moving queue + BFS connected-subgraph refinement + coarsening) with $\mu + 4\sigma$ pre-clustering `HubStripPolicy` (`infrastructure_community_id`) and deterministic tie-breaking | $O(\text{levels} \cdot E)$ |
| `community_label` | `AnalysisResults` + node paths | PageRank-weighted path/package community naming & label deduplication | $O(V)$ |
| `community_query` | `AnalysisResults` + snapshot | Community membership lookup & virtual/infrastructure community filtering | $O(1)$ / $O(V_c)$ |
| `dependency` | `PetGraphView` | Kosaraju cyclic dependency detection (`CircularDependency`) & reverse BFS | $O(V+E)$ |
| `complexity` | `ColdMetadataDb` / backend | Single-pass AST cyclomatic & cognitive complexity aggregation (`ComplexityAnalyzer`) | $O(F)$ |
| `results` | Columnar tables | Dense columnar metric storage (`AnalysisResults`: centrality, community, complexity, blast radius, structural sketch) | $O(1)$ compact-ID lookup |
| `cold_metadata` / `node_lookup` | `SnapshotNodeStore` | Zero-hydration hot-column accessors (`node_name_str`, `node_type_at`, `node_file_path_str`) + on-demand `NodeLookup` | $O(1)$ |

### Blast Radius, Review Paths & Migration Planning

| Module | Graph input | Algorithm | Complexity |
|--------|-------------|-----------|------------|
| `blast_radius_scc` | Call `PetGraphView` | Tarjan/Kosaraju SCC condensation + topological bitset reachability (`BlastRadiusEngine`); on-demand reachability fallback for ultra-flat graphs | $O(V + E \cdot W/64)$ build; $O(1)$ bitset query |
| `blast_engine_snapshot` | `BlastRadiusEngine` | Binary persistence (`blast_engine.bin`) keyed by `graph_digest` | $O(V_{\text{SCC}})$ I/O |
| `blast_radius` | `PetGraphView` | Bounded reverse-BFS caller impact (`BlastRadiusAnalyzer`) | $O(V+E)$ |
| `macro_call_index` / `macro_call_lookup` | `BlastRadiusResult` + SQLite/bin | Precomputed FQN symbol lookup (`macro_call_index.bin` / `.db`) for instant CLI symbol resolution | $O(1)$ lookup |
| `review_paths` | Base & head snapshots | PR call-path delta analysis (`build_review_paths_report`): upstream/downstream call diffs, blast-radius shifts, ambiguity reporting | $O(\Delta V \cdot d_{\max})$ |
| `migration` | Community graph + `AnalysisResults` | Inter-community dependency condensation & weighted topological sort (`compute_migration_plan`) | $O(V_c + E_c)$ |

### Cross-Language Resolution, SCIP Overlay & Git Churn

| Module | Graph input | Algorithm | Complexity |
|--------|-------------|-----------|------------|
| `mro` | Inheritance subgraph (`Extends`, `Implements`, `Includes`, `MixesIn`) | Multi-strategy Method Resolution Order (`MroIndex`): C3 linearization (Python), mixin stack (Ruby), inherent-first (Rust), class+interface (Java/C#/TS/Kotlin), C++ virtual bases, Go struct promotion; emits `EdgeType::Overrides` and retargets polymorphic `Calls` | $O(T \cdot d^2 + E_{\text{calls}})$ |
| `scip_overlay` | Snapshot + `.scip` protobuf | Compiler-exact SCIP occurrence & symbol table overlay (`apply_scip_overlay`, `--scip`), injecting ground-truth `Calls` edges | $O(\text{occurrences})$ |
| `git_churn` | Git history + backend | Streaming `git log --numstat` parser (`compute_and_apply_git_churn`, `--with-git`), exponential recency decay $e^{-\lambda \cdot \text{age}}$, mega-commit filter ($>50$ files), Jaccard co-change similarity ($J \ge 0.25$, top-$K$) emitting `EdgeType::CoChangesWith` | $O(\text{commits} \cdot \bar{f}^2)$ |

### CFG, Dominance, Scope-Aware PDG, Lazy Slicing, Taint & CPG

| Module | Graph input | Algorithm | Complexity |
|--------|-------------|-----------|------------|
| `cfg` / `cfg_builder` | Tree-sitter AST | Intra-procedural `ControlFlowGraph` construction with lexical `scope_id` tracking on `DefVar` & `Statement`; supports single-parse AST reuse (`build_cfg_for_function_in_tree`) | $O(\text{AST nodes})$ |
| `dominance` | `ControlFlowGraph` | Cooper–Harvey–Kennedy iterative immediate dominators (`idom`) + Lengauer–Tarjan reverse-CFG post-dominators (`pdom`) + `verify_idom_acyclic` | $O(V^2)$ idom / $O(V+E)$ pdom |
| `def_use` / `dataflow` | `ControlFlowGraph` | Lexical-scope-aware reaching definitions (`compute_reaching_definitions`) keyed by `(name, scope_id)` bitvectors (prevents block-shadowing false edges while preserving `let x = x + 1` RHS-before-LHS dataflow) | $O(B \cdot D)$ |
| `pdg` | `ControlFlowGraph` + `ReachingDefs` | `ProgramDependenceGraph` combining post-dominance frontier control dependencies and scope-aware `DefUse` / `LoopCarried` data dependencies | $O(V_{\text{stmt}} + E_{\text{dep}})$ |
| `cfg_pdg_archive` | `CfgPdgRecord` stream / mmap | Indexed v3 `cfg_pdg.bin` archive with 32-byte `TocEntry` directory, `memmap2` zero-copy lookup (`CfgPdgArchiveMmap::get_record`), and lazy single-function build + atomic TOC append (`ensure_cfg_pdg_for_source`) | $O(\log F)$ lookup; $O(1)$ fn lazy append |
| `slicing` | `ProgramDependenceGraph` | Backward and forward intra-procedural program slicing (`BackwardSlicer`, `ForwardSlicer`) | $O(V_{\text{PDG}} + E_{\text{PDG}})$ |
| `interprocedural_cfg` / `interprocedural_slicing` | Call graph + per-fn PDGs | Context-sensitive interprocedural CFG view (`InterproceduralCFG`) and cross-function backward/forward slicing (`InterproceduralSlicer`) | $O(\text{visited PDGs})$ |
| `blast_slice_handoff` | `BlastRadiusEngine` + `CfgPdgArchive` | Bridges macro blast-radius callers to micro PDG parameter/return slice seeds (`trace_blast_to_slices`) | $O(k \cdot (V_{\text{PDG}} + E_{\text{PDG}}))$ |
| `taint` / `taint_rules` | `ProgramDependenceGraph` + YAML/CWE catalog | Forward taint propagation (`TaintAnalyzer`) with sanitizer neutralization, field-sensitive (`base.field`) tracking, and CWE rule packs (`TaintRuleSet`) | $O(V_{\text{PDG}} + E_{\text{PDG}})$ |
| `alias` / `type_inference` | CFG / AST statements | Intra-procedural alias closure (`may_alias_names`) and local variable type inference (`TypeInferenceEngine`) | $O(\text{stmts})$ |
| `field_write` / `field_write_locals` | CFG + local types | Receiver-qualified field mutation indexing (`FieldWriteIndex`, `field_write.bin`) | $O(F \cdot \text{stmts})$ |
| `cpg` / `cpg_export` | Snapshot + `CfgPdgArchive` + `FieldWriteIndex` | Code Property Graph query facade (`cpg_flows`, `cpg_mutations`, `cpg_calls`, `cpg_function`) and DOT/JSON export | $O(\text{subgraph})$ |
| `ast_skeleton` | Tree-sitter AST | Normalized AST structural skeleton extraction & archive (`ast_skeleton.bin`) | $O(\text{AST nodes})$ |

### Clone Detection (Type 1–4)

| Module | Graph input | Algorithm | Complexity |
|--------|-------------|-----------|------------|
| `clones` | Function nodes | Type-1 exact `code_hash` grouping (`build_exact_report`) and Type-2 64-bit Bloom token-sketch LSH banding (`build_bloom_report`) with sidecar cache | $O(N)$ exact; $O(\text{bands} \cdot B^2)$ Bloom |
| `sese` | `ControlFlowGraph` | Single-Entry Single-Exit (SESE) hammock decomposition ($3 \le |S| \le 15$) (`extract_sese_regions`) | $O(V+E)$ |
| `wl_hash` | CFG SESE hammocks | 2-iteration 1-dimensional Weisfeiler–Lehman (1-WL) canonical color refinement (`WeisfeilerLehmanHasher`) | $O(V+E)$ |
| `fragment_clones` | Snapshot + `CfgPdgArchive` | Two-stage sub-function clone detector (`discover_fragment_clones`): Stage 1 bitwise Bloom popcount filter + Stage 2 1-WL hammock isomorphism | $O(N)$ Stage 1 + $O(\text{cand} \cdot (V+E))$ Stage 2 |

### Architectural Policy, Governance & Boundaries

| Module | Graph input | Algorithm | Complexity |
|--------|-------------|-----------|------------|
| `policy` | Graph / snapshot | Architectural rule evaluation (`PolicyRegistry`, `evaluate_policies`) | $O(\text{rules} \cdot E)$ |
| `scoped_policy` | Columnar snapshot + diff | PR-scoped upstream call-closure extraction (`collect_upstream_call_closure`) and bounded subset hydration (`hydrate_subset`) | $O(\Delta V + E_{\text{closure}})$ |
| `policy_diff` / `calendar_policy` / `violation_ledger` | Base & head policy runs | Temporal violation classification (`New`, `Existing`, `Resolved`), calendar grace-period disposition, and persistent ratchet ledger (`ViolationLedger`) | $O(\text{violations})$ |
| `boundary` | Graph nodes + bundled catalogs | Architectural boundary classification (`classify_boundaries`: HTTP, gRPC, DB, Queue, CLI, FFI) | $O(V \cdot \text{rules})$ |

### Semantic Search & Hybrid Graph Expansion

| Module | Graph input | Algorithm | Complexity |
|--------|-------------|-----------|------------|
| `semantic_vocab` | Identifier / doc tokens | Compiled vocabulary token-space accumulator (`vocab-accumulate-v1`, `rbvk` distilled binary matrix) | $O(\text{tokens} \cdot D)$ |
| `semantic_embedder` / `semantic_code_daemon` / `semantic_onnx` | Function signatures & bodies | Pluggable embedding backends: `vocab-accumulate` (default), `sign-hash`, and optional `semantic-onnx` / `code-daemon` Matryoshka embeddings | $O(\text{tokens} \cdot D)$ |
| `semantic_diffuse` | `CallGraph` + dense `f32` | Jacobi call-neighborhood feature diffusion (`diffuse_call_topology`) prior to quantization | $O(K \cdot E \cdot D)$ |
| `semantic_search` | Quantized bitvectors | 1-bit binary Hamming index (`SemanticIndex`) + hardware `popcnt` top-$K$ scan (`hamming_top_k`) | $O(N \cdot W_{64})$ |
| `semantic_fusion` | `SemanticIndex` + `AnalysisResults` | Late multi-signal rank fusion (`fuse_candidates`: Hamming similarity + token/FQN overlap + PageRank prior) | $O(\text{pool})$ |
| `semantic_hybrid` | `SemanticHit` + `BlastRadiusEngine` | Call-neighborhood and blast-radius context expansion around semantic hits (`expand_semantic_hits`) | $O(k \cdot d_{\text{expand}})$ |

Semantic search is **opt-in** (`rgctl semantic index`, default `vocab-accumulate-v1`). Use `--embedder code-daemon` for ONNX embeddings (requires `--features semantic-onnx`). See [semantic-search-design.md](../../docs/design/semantic-search-design.md).

## Fast-Leiden community detection

`CommunityDetector` in `community.rs` implements the **Fast-Leiden** algorithm ([Traag, Waltman & van Eck, 2019](https://doi.org/10.1038/s41598-019-41695-z)) directly on CSR `FlatGraphIndex` slices:

1. **Pre-clustering hub stripping (`HubStripPolicy`)**: Nodes whose total degree exceeds $\mu + k\sigma$ (`DEFAULT_HUB_SIGMA_K = 4.0`, when $V \ge 32$, capped at `DEFAULT_MAX_FROZEN_FRACTION`) are isolated before optimization and assigned to a dedicated `"Shared / Infrastructure"` community (`infrastructure_community_id`) so ubiquitous logging/utility helpers do not bridge unrelated domains.
2. **Phase 1 — Local moving queue**: Visits queued nodes using a `VecDeque<u32>` + `in_queue: Vec<bool>` bitset, moving each node to the neighbor community maximizing Newman modularity gain $\Delta Q$ with deterministic tie-breaking (`TieBreakStrategy::SmallestCommunityId`), and re-queuing out-of-community neighbors on move.
3. **Phase 2 — Leiden refinement (`refine_partition`)**: Splits any disconnected community into connected sub-communities via BFS (`verify_community_connectedness`), guaranteeing 0 disconnected communities.
4. **Phase 3 — Coarsening (`coarsen_graph`)**: Aggregates refined communities into weighted super-nodes and repeats for up to `max_levels` until modularity converges.

> Note: Legacy serialization fields named `louvain_community_id` in dashboard/migration payloads store the Fast-Leiden community IDs for schema compatibility.

## Running tests and benchmarks

```bash
cargo test -p rgctl-analysis
cargo clippy -p rgctl-analysis -- -D warnings
cargo bench --bench graph_benchmarks        # PetGraphView + blast radius
cargo bench --bench centrality_benchmarks   # FastPageRank (<10ms at 150K), SampledBetweenness (<250ms at 150K)
cargo bench --bench community_benchmarks    # Fast-Leiden community detection
```

## Conventions

- Build `PetGraphView` / `FlatGraphIndex` once per analysis pass; pass shared references to analyzers.
- Never cold-deserialize `NodeExtension` (`get_node` / `materialize_node_at`) in hot loops when CSR topology or hot columnar slices (`node_name_str`, `node_type_at`, `node_file_path_str`, `node_qualified_name_str`) suffice.
- Gate Rayon parallel iterators behind explicit vertex-count thresholds (e.g. $V \ge 10{,}000$ in `FastPageRank`, $V \ge 4{,}096$ in `SampledBetweenness`) to avoid thread-pool synchronization overhead on small graphs.
- Use `TraversalConfig` (default depth 10) for bounded BFS traversals.
- Prefer `BlastRadiusEngine` over `BlastRadiusAnalyzer` for whole-graph transitive impact queries.
- Discover uses **`analyze_columnar`** for centrality; profile stages with `RUST_LOG=info,profile=info rgctl discover -v`.
- No `unwrap()` in production paths; propagate errors with `rgctl_error::Result`.
- `Node` string-like fields (`name`, `qualified_name`, `file_path`) use `SharedStr`; in tests prefer passing `&str` directly to builders (e.g. `Node::new(..., "name")`) and only call `.into()` when assigning owned `String` values into `Option<SharedStr>` fields.
