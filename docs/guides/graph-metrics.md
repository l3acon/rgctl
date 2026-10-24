# Graph Metrics

## Introduction

The `metrics` command runs **network analytics** on your code knowledge graph: pull-based CSR **FastPageRank** for structural importance, **SampledBetweenness** for bridge functions, **Fast-Leiden** community modularity, optional **HyperBallHarmonic** centrality (from `discover --with-harmonic`), and **git churn** hotspots (from `discover --with-git`). These are production graph algorithms applied to the structure of your code.

Metrics give you a quantitative view of architecture — which functions are critical hubs, which bridge modules, how well code separates into clusters, and which files churn most recently.

## Use Cases

- **Identify critical functions.** PageRank reveals the most structurally important production symbols.
- **Find architectural bottlenecks.** High betweenness indicates functions that bridge otherwise disconnected parts of the codebase.
- **Measure modularity.** Fast-Leiden communities + modularity score quantify how well code separates into cohesive groups.
- **Prioritize refactoring.** Focus effort on high-PageRank, high-betweenness functions where changes have the largest structural impact.
- **Track hotspots over time.** Combine structural metrics with `metrics --churn` after `discover --with-git`.

## Example Project

This guide uses the **CoolStore** (`example/coolstore`). Make sure you have run `discover` first:

CoolStore examples use `-l java` to index the Java backend only (skip Angular/bower).

```bash
rgctl -r example/coolstore discover -l java
# Optional overlays used later in this guide:
# rgctl -r example/coolstore discover -l java --with-harmonic
# rgctl -r example/coolstore discover -l java --with-git --git-since 90.days
```

## Step-by-Step

### 1. PageRank (FastPageRank + NodeRole)

Pull-based CSR **FastPageRank** (double-buffered, adaptive tolerance) ranks symbols by recursive importance via incoming edges. Dual tracks exist for behavioral (`Calls`) vs structural (`STRUCTURAL_EDGE_TYPES`) projections.

By default, **`NodeRole`** filtering keeps **Production** hotspots first and drops `Test`, `Generated`, and `ExternalStub` nodes from the default top list. Pass `--include-tests` to include test helpers and stubs.

```bash
rgctl -r example/coolstore -f json metrics --pagerank
```

**Output (truncated):**

```json
{
  "pagerank": {
    "converged": true,
    "iterations": 9,
    "max_delta": 3.6429738108079146e-07,
    "top": [
      {
        "file_path": "src/main/java/com/redhat/coolstore/model/CatalogItemEntity.java",
        "kind": "class",
        "name": "CatalogItemEntity",
        "node": "9da23d53-5e6e-5267-99d2-a6069d8e2e8f",
        "pagerank": 0.006735939393751206,
        "qualified_name": "com.redhat.coolstore.model.CatalogItemEntity"
      },
      {
        "file_path": "src/main/java/com/redhat/coolstore/model/Order.java",
        "kind": "class",
        "name": "Order",
        "node": "3f133891-b72c-5d9d-bd7a-daa4e444cd7a",
        "pagerank": 0.0067091538522855675,
        "qualified_name": "com.redhat.coolstore.model.Order"
      }
    ]
  },
  "schema_version": 1
}
```

**What this tells you:**

- **`converged: true`** -- PageRank converged (here in 9 iterations on the Java-only graph).
- **`top`** -- highest-ranked **production** nodes after `NodeRole` filtering (domain types such as `CatalogItemEntity` / `Order`).
- Each entry includes `node` (UUID), `name`, `kind`, `qualified_name`, and `file_path` when available.

Include tests / stubs or raise iteration budget:

```bash
rgctl -r example/coolstore -f json metrics --pagerank --include-tests --iterations 50
```

### 2. Betweenness Centrality (SampledBetweenness)

**SampledBetweenness** uses $K=256$ degree-stratified Brandes pivots (sequential Brandes below ~4,096 nodes). High-betweenness functions are architectural bridges — removing them would disconnect parts of the call graph.

```bash
rgctl -r example/coolstore -f json metrics --betweenness
```

**Output (truncated):**

```json
{
  "betweenness": [
    {
      "file_path": "src/main/java/com/redhat/coolstore/rest/CartEndpoint.java",
      "kind": "function",
      "name": "add",
      "node": "d0a70ef7-c170-5a07-90a2-c061373666c2",
      "qualified_name": "com.redhat.coolstore.rest.CartEndpoint.add",
      "score": 0.0009528547582929628
    },
    {
      "file_path": "src/main/java/com/redhat/coolstore/service/ShoppingCartService.java",
      "kind": "function",
      "name": "priceShoppingCart",
      "node": "28de0bfd-1b19-5c3c-bcbf-26947d067654",
      "qualified_name": "com.redhat.coolstore.service.ShoppingCartService.priceShoppingCart",
      "score": 0.00073808863588171
    }
  ],
  "schema_version": 1
}
```

**What this tells you:**

- Functions with the highest betweenness scores are the most critical bridges (here: `CartEndpoint.add`, then `priceShoppingCart`).
- These are where a bug or breaking change would propagate most widely across otherwise separate modules.
- Low betweenness means a function is "internal" to a single cluster.

### 3. Community Detection (Fast-Leiden)

Community detection partitions the graph with **Fast-Leiden** (local moving + BFS connected-subgraph refinement + coarsening) after $\mu + 4\sigma$ **`HubStripPolicy`**. The `modularity` score (0--1) measures partition quality. Field names like `louvain_community_id` are schema-compat only — see [Community Detection](community-detection.md).

```bash
rgctl -r example/coolstore -f json metrics --communities
```

**Output:**

```json
{
  "communities": {
    "assignments": 608,
    "count": 473,
    "modularity": 0.3893411219138413
  },
  "schema_version": 1
}
```

**What this tells you:**

- **`count`** -- distinct communities on the Java-only CoolStore graph.
- **`assignments`** -- nodes assigned to a community.
- **`modularity: ~0.39`** -- values above 0.3 indicate meaningful structure; above 0.5 indicates strong modularity.

### 4. Harmonic centrality (HyperBallHarmonic)

Harmonic centrality is computed at discover time with **HyperBallHarmonic** ($p=6$ HLL) when you pass `--with-harmonic`. It feeds migration ranking; re-run discover to refresh:

```bash
rgctl -r example/coolstore discover -l java --with-harmonic
```

### 5. Git churn hotspots

After indexing with git history overlay:

```bash
rgctl -r example/coolstore discover -l java --with-git --git-since 90.days
rgctl -r example/coolstore -f json metrics --churn
```

Ranks files/functions by recency-weighted `git_hotspot_score` (see also `relations --edge cochangeswith` and `.rgctl/analysis/git_churn.json`).

### 6. Combining Metrics

```bash
rgctl -r example/coolstore -f json metrics --pagerank --betweenness --communities
```

### 7. Resolving Node UUIDs

```bash
rgctl -r example/coolstore -f json find priceShoppingCart --type function --exact
rgctl -r example/coolstore blast-radius priceShoppingCart
```

## Understanding the Algorithms

| Metric | Implementation | High Score Means |
|--------|----------------|-----------------|
| **PageRank** | Pull CSR FastPageRank + `NodeRole` filter | Many important production callers depend on this symbol |
| **Betweenness** | SampledBetweenness ($K=256$ Brandes pivots) | Bridges otherwise disconnected modules |
| **Communities** | Fast-Leiden + HubStripPolicy | Strong, well-separated functional clusters (modularity) |
| **Harmonic** | HyperBallHarmonic (`discover --with-harmonic`) | Reachable / central for migration ordering |
| **Churn** | `git_hotspot_score` (`discover --with-git`) | Recently / frequently edited hotspots |

## Benefits

- **Quantitative architecture analysis.** Replace subjective assessments with concrete scores.
- **Production-first hotspots.** Default PageRank excludes tests and stubs unless `--include-tests`.
- **Measure modularity.** Track whether your codebase is becoming more or less modular over time.
- **Temporal signal.** Pair structural metrics with git churn and co-change edges.
- **Fast.** Metrics run in-memory on the mmap snapshot / CSR indexes.

## Related Guides

- [Discovering and Indexing a Codebase](discovering-and-indexing.md) -- must run `discover` before metrics; `--with-git` / `--with-harmonic` overlays
- [Community Detection](community-detection.md) -- Fast-Leiden labels and `communities list`
- [Blast Radius Analysis](blast-radius-analysis.md) -- per-function impact analysis that complements metrics
- [Migration Planning](migration-planning.md) -- metrics feed into migration ordering
- [CLI Command Reference](cli-command-reference.md) -- full `metrics` flag map
