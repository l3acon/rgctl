# rgctl-graph Storage Architecture

The `rgctl-graph` crate stores the code knowledge graph: typed nodes and edges, secondary indexes, persistence, and a mini query language. Analysis algorithms live in `rgctl-analysis`.

## Layers

| Layer | Module | Role |
|-------|--------|------|
| Schema | `schema.rs` | `Node`, `Edge`, `NodeType`, `EdgeType` |
| Backend | `backend/memory.rs` | `MemoryBackend` — RwLock maps + indexes |
| Interning | `intern.rs` | `Arc<str>` dedup for index keys |
| Query | `query.rs` | Filter language (`type:Function`, compound `\|`) |
| Persistence | `snapshot.rs`, `columnar_snapshot.rs` | v1 bincode / **`RGCL` v2** columnar mmap |
| Compaction | `graph_compactor.rs` | `DeltaSegment` tombstones + `GraphCompactor` atomic rewrite |
| API | `code_graph.rs` | `CodeGraph` wrapper |

## Default artifact: `RGCL` v2 columnar snapshot

`discover` / `update` / `watch` / `merge` write **`.rgctl/graph.snapshot.bin`** — an `RGCL` v2 memory-mapped columnar snapshot (`ColumnarGraphMmap`, `SnapshotNodeStore`):

| Component | Layout |
|-----------|--------|
| Hot node row | Fixed **64-byte** `NodeRow` |
| Hot edge row | Fixed **40-byte** `EdgeRow` |
| CSR | `out_offsets` / `in_offsets` |
| Strings | Shared string arena |
| Cold payload | `NodeExtension` via `cold_ext` (deserialized only when needed) |

**Zero-hydration hot accessors** (`node_name_str`, `node_type_at`, `node_file_path_str`, `node_qualified_name_str`) scan name/type/path/FQN columns **without** deserializing `cold_ext`. BLAKE3 `content_digest` in the header drives sidecar invalidation.

Legacy **`graph.db` / `graph.json`** are written only with `--write-json-graph` — not the default discover output.

## MemoryBackend

- **Nodes:** `HashMap<Uuid, Node>` under `RwLock`
- **Edges:** `Vec<Edge>` under `RwLock`
- **Indexes:** name, type, label, property, edge-type
- **Query cache:** invalidated on every mutation

Zero-clone APIs for analysis: `edge_topology_typed()`, `for_each_node`, `find_node_ids_by_*`.

## Snapshots

| Format | Version | Open cost |
|--------|---------|-----------|
| JSON | legacy (`--write-json-graph`) | Full parse |
| bincode mmap | v1 | Deserialize payload |
| columnar mmap (`RGCL`) | **v2 (default)** | Header + indexes only |

## Incremental updates & facts

| Artifact | Role |
|----------|------|
| `.rgctl/facts/` | Per-file `FileExtractionFacts` cache |
| `DeltaSegment` | Tombstone bitmap (`invalidated_nodes`) for changed files |
| `GraphCompactor` | Stream-compact live nodes/edges → atomic snapshot rename (`update`, `watch`, `serve --watch`) |

## Multi-repo park-and-resolve

Unresolved cross-repo calls/imports/routes are parked in **`.rgctl/parked_relations.bin`** (`RBPK`). `rgctl merge` merges $N$ repo snapshots with 4-tier resolution (`ExactFqn` → `ImportScopedSuffix` → `UniqueBareName` → `AmbiguousStub`) and rewires external stubs to real target UUIDs.

## CFG/PDG archive (analysis sidecar)

Per-function CFG/PDG lives beside the graph under `.rgctl/analysis/` as a **v3 indexed archive** (`CfgPdgArchiveMmap`): 32-byte `TocEntry` directory, `memmap2` zero-copy lookup, and atomic single-function append via `ensure_cfg_pdg_for_source` (used by `slice` / `cpg flows` without whole-repo `discover --with-cfg`).

## SQLite (blast-radius only)

rgctl does **not** store the knowledge graph in SQLite. The only SQLite usage is `macro_call_index.db` in `rgctl-analysis` — a precomputed **blast-radius lookup cache** written at `discover` and read by `blast-radius` T0 (`MacroCallLookupDb`). Structured query verbs, export, and metrics use the columnar mmap graph.

## Downstream

```
MemoryBackend / ColumnarGraphMmap → rgctl-analysis::PetGraphView → algorithms
```

See also: [analysis-architecture.md](analysis-architecture.md), [cli-io-sanity-qe.md](cli-io-sanity-qe.md), [CLI command reference](guides/cli-command-reference.md).
