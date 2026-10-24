# rgctl-core

Public library facade for rgctl — one dependency for graph, analysis, pipeline, dashboard helpers, and related crates.

Graph persistence defaults to the **`RGCL` v2** columnar mmap snapshot (`.rgctl/graph.snapshot.bin`) from `rgctl-graph`. Analysis algorithms (Fast-Leiden communities, FastPageRank, lazy v3 `cfg_pdg.bin` slicing, SESE+1-WL fragment clones, MRO `Overrides`, SCIP overlay, git churn) live in `rgctl-analysis`.

## Unique code

Only `memory::MemoryMonitor` (RSS tracking via sysinfo) lives in this crate. All graph and analysis logic is in workspace crates re-exported from `lib.rs`.

## Re-export policy

Add a crate to `rgctl-core` when:

1. Library consumers commonly need it alongside graph/analysis APIs, and
2. The crate has a stable public surface suitable for re-export.

Prefer direct dependencies on domain crates (`rgctl-graph`, `rgctl-analysis`) for slim binaries or plugins that need only one layer.

## Key modules

| Re-export | Crate |
|-----------|-------|
| `graph` | `rgctl-graph` |
| `analysis` | `rgctl-analysis` |
| `pipeline` | `rgctl-pipeline` |
| `gql` | `rgctl-gql` |
| `incremental` | `rgctl-incremental` |

## Version

`rgctl_core::VERSION` matches this crate's package version.
