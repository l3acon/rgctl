# Community Detection

## Introduction

The `communities` command lets you **list and label functional clusters** that rgctl detects in your codebase. During `discover` (and via `metrics --communities`), **Fast-Leiden** partitions the call graph into groups of tightly connected functions. The `communities` command explores those clusters and generates human-readable labels.

Communities reveal the implicit architecture of your code — groups of functions that work together closely even if they span multiple files or packages.

> **Schema note:** JSON / analysis fields such as `louvain_community_id` retain that name for **backward schema compatibility only**. The detector does **not** run Louvain or classic label propagation (Raghavan et al.); the algorithm is Fast-Leiden with hub stripping (below).

## Algorithm (Fast-Leiden + HubStripPolicy)

`CommunityDetector` (`crates/rgctl-analysis/src/community.rs`) runs on the CSR `FlatGraphIndex`:

| Stage | What happens |
|-------|----------------|
| **0. Hub strip** | $\mu + 4\sigma$ **`HubStripPolicy`** isolates ubiquitous utilities into a shared **`"Shared / Infrastructure"`** bucket (`infrastructure_community_id`) so hubs do not glue unrelated domains into one mega-community. |
| **1. Local moving** | Queue-based modularity ascent ($\Delta Q$) moves nodes into neighboring communities. |
| **2. Refinement** | BFS connected-subgraph refinement (`refine_partition`) guarantees **0 disconnected communities**. |
| **3. Coarsening** | Super-node aggregation / coarsening for multilevel quality. |

CLI surfaces: `rgctl metrics --communities`, `rgctl communities list`, `rgctl communities label`.

## Use Cases

- **Understand implicit architecture.** See how functions cluster together beyond the package/directory structure.
- **Identify cohesive modules.** Large communities with clear labels indicate well-defined functional domains.
- **Spot coupling problems.** An unexpectedly large community might indicate tight coupling between what should be separate modules.
- **Plan microservice extraction.** Communities map naturally to potential service boundaries.
- **Navigate unfamiliar code.** Browse communities to get a high-level map of what the codebase does.

## Example Project

This guide uses the **CoolStore** (`example/coolstore`). Make sure you have run `discover` first:

CoolStore examples use `-l java` to index the Java backend only (skip Angular/bower).

```bash
rgctl -r example/coolstore discover -l java
```

## Step-by-Step

### 1. List Communities

List all detected communities, sorted by member count (largest first):

```bash
rgctl -r example/coolstore -f json communities list
```

**Output (truncated):**

```json
{
  "communities": [
    {
      "id": 177,
      "label": "Order",
      "member_count": 25
    },
    {
      "id": 397,
      "label": "coolstore.service::CatalogItemEntity",
      "member_count": 20
    },
    {
      "id": 143,
      "label": "ShoppingCart",
      "member_count": 15
    },
    {
      "id": 306,
      "label": "coolstore.model::shoppingCartToJson",
      "member_count": 14
    },
    {
      "id": 608,
      "label": "Infrastructure / Common Library",
      "member_count": 12
    }
  ],
  "modularity": 0.39620256943546045,
  "schema_version": 1,
  "written": false
}
```

**What this tells you:**

- **`id`** -- the community's unique identifier (use with `inventory --by community` and scoped `find`).
- **`label`** -- a heuristic label generated from the most representative member names and paths.
- **`member_count`** -- how many members belong to this community.
- Largest communities on the Java-only CoolStore index are domain clusters (`Order`, `ShoppingCart`, `coolstore.service::*`) plus an **Infrastructure / Common Library** bucket from hub stripping — not vendor JS.
- Envelope also includes `modularity` and `written` (whether labels were persisted this run).

### 2. Explore community members

Community labels are heuristics (often `package::symbol`). Use the label’s package prefix as a `--scope` on `find`:

```bash
rgctl -r example/coolstore -f json find --type function \
  --scope com.redhat.coolstore.model --limit 20
```

Pair with `communities list` to map ids → labels, and `inventory --by community` for sizes.

### 3. Refresh Community Labels

If community labels are missing or stale, regenerate them:

```bash
rgctl -r example/coolstore communities label --write
```

The `--write` flag persists the updated labels into the analysis results, so subsequent `communities list` calls use the new labels.

### 4. Community inventory

```bash
rgctl -r example/coolstore -f json inventory --by community | jq '.counts[:5]'
```

### 5. Semantic Search by Community

Use semantic search with community scope to find communities matching a concept:

```bash
rgctl -r example/coolstore semantic index
rgctl -r example/coolstore -f json semantic query "checkout" \
  --scope community --limit 10
```

This finds entire communities whose member functions are semantically related to "checkout".

## Understanding Community Labels

rgctl generates community labels heuristically from member names and file paths:

| Label Pattern | Meaning |
|---------------|---------|
| `coolstore.model::shoppingCartToJson` | Functions from the `coolstore.model` package, anchored by a representative symbol |
| `Infrastructure / Common Library` | Cross-cutting utilities isolated by $\mu + 4\sigma$ hub stripping (`infrastructure_community_id`) |
| `coolstore.service::CatalogItemEntity` | Package-prefix heuristic from member names/paths |
| `Order` / `ShoppingCart` | Domain-type labels when a single type dominates the cluster |

## Benefits

- **Automatic architecture map.** Community detection reveals the implicit structure of your code without any manual annotation.
- **Data-driven boundaries.** Communities are based on actual call relationships, not directory layout or naming conventions.
- **Microservice candidates.** Well-separated communities with clear labels are natural extraction targets.
- **Coupling visibility.** Large or oddly-named communities surface unexpected coupling between modules.
- **Composable with other commands.** Community labels work with scoped `find`, semantic search, and migration planning.

## Related Guides

- [Discovering and Indexing a Codebase](discovering-and-indexing.md) -- `discover` runs community detection
- [Graph Metrics](graph-metrics.md) -- community modularity score via `metrics --communities`
- [Semantic Search](semantic-search.md) -- community-scoped semantic queries
- [Structured graph queries](structured-query.md) -- `find`, `inventory`, and `communities list`
- [Migration Planning](migration-planning.md) -- communities define migration extraction steps
- [CLI Command Reference](cli-command-reference.md) -- `metrics` / `communities` flags
