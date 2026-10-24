# rgctl-graph

Graph storage and query layer for [rgctl](https://github.com/sshaaf/rgctl).

Default discover artifact is **`.rgctl/graph.snapshot.bin`** — an **`RGCL` v2** memory-mapped columnar snapshot (`ColumnarGraphMmap`, `SnapshotNodeStore`). See [docs/graph-storage-architecture.md](../../docs/graph-storage-architecture.md).

## Modules

| Module | Role |
|--------|------|
| `schema` | Typed `Node` / `Edge`, `NodeType`, `EdgeType` |
| `backend::MemoryBackend` | In-memory store + indexes (ingest / mutation path) |
| `backend::GraphBackend` | Minimal trait (insert/get/query) |
| `query` | Mini filter language (`type:Function`, …) |
| `structured_query` | Zero-hydration `find` / `callers` / `callees` / `relations` / `inventory` over mmap |
| `snapshot` | Legacy prepared / mmap snapshots (v1 bincode) |
| `columnar_snapshot` | **`RGCL` v2** columnar mmap (64-byte `NodeRow`, 40-byte `EdgeRow`, CSR `out_offsets`/`in_offsets`, string arena, cold `NodeExtension` / `cold_ext`) |
| `graph_compactor` | `DeltaSegment` tombstone bitmap + `GraphCompactor` atomic snapshot rewrite (`update` / `watch`) |
| `code_graph` | High-level `CodeGraph` API |
| `csr` / `segmented_spill` | CSR projection helpers and spill-backed commit |

## Default storage (`RGCL` v2)

- **Hot-column scans** (`node_name_str`, `node_type_at`, `node_file_path_str`, `node_qualified_name_str`) run with **zero `cold_ext` deserializations**.
- Per-file extraction cache: `.rgctl/facts/`.
- Cross-repo unresolved calls/imports/routes park in `.rgctl/parked_relations.bin` (`RBPK`); `rgctl merge … --output-dir` resolves them.
- Legacy `graph.db` / `graph.json` only with `--write-json-graph` (opt-in; not the default discover output).

## GraphBackend vs MemoryBackend

`GraphBackend` covers basic CRUD and string queries. Performance APIs (`edge_topology_typed`, batch iterators, typed indexes) require `MemoryBackend` directly — the only production in-memory backend today. Query CLIs prefer mmap `ColumnarGraphMmap` / `SnapshotNodeStore` over full hydration.

## Analysis integration

`rgctl-analysis` builds `PetGraphView` / `FlatGraphIndex` from `edge_topology_typed()` or mmap snapshots. Do not use `MemoryBackend::calculate_pagerank` for new code — use `rgctl-analysis::CentralityAnalyzer` / `FastPageRank`.

## Tests

```bash
cargo test -p rgctl-graph
cargo clippy -p rgctl-graph -- -D warnings
```

Golden-repo validation: [scripts/validate-golden-repos.sh](../../scripts/validate-golden-repos.sh)
