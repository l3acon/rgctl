# CLI Command Reference & Architecture Guide

This guide documents the complete `rgctl` command taxonomy across its six functional groups (`Lifecycle`, `Query`, `Analysis`, `Security`, `Policy`, and `Meta`), explains how overlapping/compatibility commands map to canonical verbs, and provides copy-pasteable examples for every workflow.

---

## 1. Global Options

Global flags apply to every `rgctl` command and can be passed before or after the subcommand:

| Flag | Environment | Default | Description |
|------|-------------|---------|-------------|
| `-r, --repo <PATH>` | — | Current directory (`.`) | Target repository root containing `.rgctl/` |
| `-d, --db <PATH>` | — | `<repo>/.rgctl/graph.snapshot.bin` | Explicit path to the graph snapshot artifact |
| `-f, --format <FMT>` | `RGCTL_FORMAT` | `text` | Output format (`text`, `json`, `table`, `compact`). Agents should always pass `-f json`. |
| `-o, --output <FILE>` | — | `stdout` | Write command output to `<FILE>` instead of `stdout` |

---

## 2. Command Taxonomy & Canonical Map

`rgctl --help` organizes all commands into six domain groups:

```text
Commands:
Lifecycle:
  discover      Index and analyze a codebase
  status        Session graph status (snapshot presence, digest, node/edge counts, source staleness; no rediscover)
  update        Incremental structural graph update for changed files (not a full discover / analysis rebuild)
  watch         Watch the repo and apply incremental graph updates on save (notify, default 25ms debounce)
  merge         Merge multiple discovered repos, resolving parked cross-repo relations
  serve         Serve the analysis dashboard and GQL query API over HTTP
  diff          Diff two columnar graph snapshots (cold diff profiling / compare path)

Query:
  find          Find symbols by name/type over the mmap graph index (no GQL)
  callers       Incoming CALLS neighbors for a symbol
  callees       Outgoing CALLS neighbors for a symbol
  relations     Typed edge traversal; omit SYMBOL for seedless set-wide scan
  inventory     Aggregate symbol/edge counts (includes zero-count schema kinds for type/edge)
  gql           Experimental graph query language — prefer find / callers / relations / inventory for common queries

Analysis:
  blast-radius  Macro impact / blast radius for a symbol
  slice         Line-level program slice or taint trace (builds & caches single-function CFG/PDG lazily on demand)
  taint         Sink-first taint (--sink + --source external); requires discover --with-cfg
  inspect       Inspect raw CFG / PDG / dominance for a function symbol
  metrics       Network & churn analytics (PageRank, betweenness, communities, git churn)
  semantic      Opt-in semantic search over function symbols (separate index artifact)
  communities   List or refresh named communities (analysis overlay)
  clones        Clone detection: exact Type-1 (code_hash), Type-2 Bloom LSH, or Type-3/4 SESE+1-WL fragments
  cpg           Hybrid CPG façade (topology + CFG/PDG archive)

Security:
  vuln          OSV vulnerability triage / analyze
  deps          Dependency inventory match against OSV
  security      Security / OSV entrypoints (aliases of top-level vuln, deps, taint)

Policy:
  check         CI policy gateway
  review        Temporal PR analysis (review paths / review check)
  rules         Evaluate Konveyor-shaped rules against the session (Kantra engine)

Meta:
  export        Export graph or projections
  install       Install bundled agent pack (skills, optional policy)
  help          Print this message or the help of the given subcommand(s)
```

### Canonical Command vs Compatibility Aliases

Several commands have historical or namespace aliases for backwards compatibility. Always prefer the **Canonical Command**:

| Intent / Workflow | Canonical Command | Compatibility Alias(es) | Notes |
|-------------------|-------------------|-------------------------|-------|
| Find symbols by name/type/glob/annotation | `rgctl find` | `rgctl query find` | Zero-hydration hot-column scan over `graph.snapshot.bin` |
| Incoming / outgoing `Calls` neighbors | `rgctl callers` / `rgctl callees` | `rgctl query callers`, `rgctl query callees`, `rgctl cpg calls` | `callers`/`callees` support `--depth`, `--scope`, `--package`, `--methods` |
| Traverse any typed edge (`Overrides`, `CoChangesWith`, `Extends`, …) | `rgctl relations` | `rgctl query relations` | Supports seeded (`<SYMBOL>`) or seedless repo-wide scans |
| Count symbols/edges by type, language, file, or community | `rgctl inventory` | `rgctl query inventory` | Includes zero-count schema kinds for `--by type` / `--by edge` |
| Incremental file re-indexing | `rgctl update --files <PATHS>` | `rgctl discover --files <PATHS>` | `update` coordinates with live `watch` / `serve --watch` locks |
| Statement-level backward/forward program slicing | `rgctl slice` | `rgctl cpg slice`, `rgctl cpg flows` | `slice` auto-resolves enclosing function from `file:line`, lazily caches `cfg_pdg.bin`, and supports `--with-alias` and `--taint` |
| Inspect function PDG / CFG / Dominator tree | `rgctl inspect <SYM> cfg\|pdg\|dom` | `rgctl cpg pdg <SYM>` | `inspect` covers all 3 procedural layers (`cfg`, `pdg`, `dom`) |
| Temporal PR policy gate | `rgctl review check` | `rgctl pr-check`, `rgctl check --temporal` | Evaluates `New` vs `Existing` vs `Resolved` violations + calendar grace windows |
| OSV vulnerability & dependency reachability | `rgctl vuln`, `rgctl deps`, `rgctl taint` | `rgctl security {vuln, deps, taint}` | Top-level verbs and `security` subcommands share identical flags |

---

## 3. Detailed Group Reference

### 3.1 Lifecycle Commands (`discover`, `status`, `update`, `watch`, `merge`, `serve`, `diff`)

#### `rgctl discover [PATH]`
Indexes source files, resolves cross-file & cross-language symbols (including C3/mixin/interface MRO `Overrides` and framework route/DI wiring), compiles the zero-copy columnar snapshot (`.rgctl/graph.snapshot.bin`), and computes Stage 1 analytics (`analysis_results.bin`, `blast_engine.bin`, `macro_call_index.bin`, `parked_relations.bin`).

- **Default cold path (fast, zero-hydration):**
  ```bash
  rgctl discover .
  rgctl discover . -l rust,typescript -e "target/**,node_modules/**"
  ```
- **Compiler-exact SCIP overlay (`--scip`):**
  Upgrades ambiguous call edges using a pre-generated `.scip` protobuf index (`resolution = "scip"`):
  ```bash
  rgctl discover . --scip index.scip
  ```
- **Git temporal co-change & churn overlay (`--with-git`):**
  Streams `git log --numstat`, computes recency-weighted file churn (`git_hotspot_score`) and Jaccard co-change edges (`EdgeType::CoChangesWith`), and writes `.rgctl/analysis/git_churn.json`:
  ```bash
  rgctl discover . --with-git --git-since 90.days
  ```
- **Deep procedural analysis (`--with-cfg`, `--with-taint`, `--with-dfg-loops`, `--with-ast-skeleton`):**
  Pre-builds the indexed v3 `cfg_pdg.bin` archive and `field_write.bin` mutation index across all functions (note: single-function `rgctl slice` works lazily even without `--with-cfg`):
  ```bash
  rgctl discover . --with-cfg --with-taint
  ```
- **Migration roadmap & dashboard (`--export-migration-hints`, `--with-harmonic`, `--with-dashboard`):**
  ```bash
  rgctl discover . --with-harmonic --export-migration-hints --migration-preset risk_mitigation
  ```
- **Staged full pipeline (`--full`) & container resource budgets (`--with-limits`):**
  ```bash
  rgctl discover . --full --with-limits max-mem-mb=4096,threads=4
  ```

#### `rgctl status`
Checks `.rgctl/` snapshot presence, `graph_digest`, node/edge counts, and whether working-tree source files have changed since the snapshot was written (never triggers a rebuild).
```bash
rgctl -f json status
```

#### `rgctl update [PATH]` & `rgctl watch [PATH]`
Incrementally re-extracts modified/added/deleted files, cascades reverse call-dependency hops (`--cascade-depth`, default `1`), and compacts `graph.snapshot.bin`.
```bash
rgctl update --files src/order.rs,src/payment.rs
rgctl update --since origin/main
rgctl watch .
```

#### `rgctl merge <REPOS>... --output-dir <OUTPUT_DIR>`
Merges multiple discovered repositories into a unified multi-repo columnar snapshot, resolving cross-repository calls, imports, and HTTP/RPC routes parked in each repo's `.rgctl/parked_relations.bin` using 4-tier park-and-resolve matching (`ExactFqn` → `ImportScopedSuffix` → `UniqueBareName` → `AmbiguousStub`) and rewiring external stubs to real target definitions:
```bash
rgctl discover services/auth
rgctl discover services/billing
rgctl merge services/auth services/billing --output-dir .rgctl-merged
rgctl -r .rgctl-merged -f json callers validateToken
```

#### `rgctl serve [PATH]` & `rgctl diff --base <BASE> --head <HEAD>`
- `rgctl serve --open --watch`: Serves the interactive web dashboard (`http://127.0.0.1:8080`) and HTTP query API with live incremental file watching.
- `rgctl diff --base <BASE_SNAP> --head <HEAD_SNAP>`: Zero-cold-touch columnar snapshot diffing by `StableEntityKey`.

---

### 3.2 Structured Query Commands (`find`, `callers`, `callees`, `relations`, `inventory`)

All structured query commands execute directly against the memory-mapped columnar snapshot (`graph.snapshot.bin`) without hydrating cold node property payloads unless `--class` or `--show-attributes` is requested.

#### `rgctl find [PATTERN]`
```bash
# Glob search by name and type
rgctl -f json find "*Order*" --type class --lang java --limit 20

# Find all entities carrying an annotation (with attribute text)
rgctl -f json find --annotation Transactional,RestController --show-attributes

# Find imports/symbols scoped to a package coordinate (PURL / Maven GAV / crate)
rgctl -f json find --package "pkg:maven/com.fasterxml.jackson.core/jackson-databind"
```

#### `rgctl callers <SYMBOL>` & `rgctl callees <SYMBOL>`
```bash
# Direct and 2-hop callers, disambiguated by file/class/line
rgctl -f json callers processPayment --class PaymentService --depth 2

# Filter callers into specific methods of a package
rgctl -f json callers --package "com.fasterxml.jackson.databind" --methods readValue,readTree
```

#### `rgctl relations [SYMBOL] --edge <TYPE>`
Traverses any typed edge (`calls`, `uses`, `extends`, `implements`, `overrides`, `cochangeswith`, `annotatedwith`, `exposesroute`, `injects`, `violates`, `contains`, `imports`, `dependson`):
```bash
# Cross-language MRO override hierarchy for a method
rgctl -f json relations process --edge overrides --direction both

# Files that co-change with a given file in git history (requires discover --with-git)
rgctl -f json relations "src/checkout.rs" --edge cochangeswith

# Seedless scan: list all HTTP routes exposed across the repository
rgctl -f json relations --edge exposesroute --limit 100
```

#### `rgctl inventory --by <DIMENSION>`
Aggregates counts by `type`, `edge`, `lang`, `file`, `community`, or `import-prefix`:
```bash
rgctl -f json inventory --by type
rgctl -f json inventory --by edge
rgctl -f json inventory --by community
```

---

### 3.3 Deep Analysis Commands (`blast-radius`, `slice`, `inspect`, `metrics`, `communities`, `semantic`, `clones`, `cpg`)

#### `rgctl blast-radius <SYMBOL>`
Computes transitive upstream caller impact using the SCC + dense bitset engine (`blast_engine.bin`), with optional architectural boundary classification (`--classify-boundary`) and statement-level PDG slice hand-off (`--with-slices`):
```bash
rgctl -f json blast-radius ChargeCard --classify-boundary --boundary REST_ENDPOINT
rgctl -f json blast-radius ChargeCard --depth 3 --with-slices
```

#### `rgctl slice <FILE> --line <N> --variable <VAR>`
Computes lexical-scope-aware backward or forward program slices over the function's Program Dependence Graph (PDG).
- **Zero upfront CFG pass needed:** If `.rgctl/analysis/cfg_pdg.bin` does not exist or does not yet contain the target function, `rgctl slice` parses only `<FILE>`, builds the single function's CFG/PDG, and atomically appends its `TocEntry` to `cfg_pdg.bin`.
- **Auto-resolves enclosing function:** When `--function` is omitted, `slice` selects the innermost function span containing `--line`.
```bash
# Backward slice (what statements affect `total` at line 42?)
rgctl -f json slice src/cart.ts --line 42 --variable total

# Forward slice with intra-procedural alias expansion (copies & field bases)
rgctl -f json slice src/cart.ts --line 18 --variable req --direction forward --with-alias

# On-demand function taint check at criterion
rgctl -f json slice src/cart.ts --line 18 --variable userInput --taint
```

#### `rgctl inspect <SYMBOL> <cfg|pdg|dom>`
Inspects the raw Control Flow Graph (`cfg`), Program Dependence Graph (`pdg --edge-layer all|data|control --def-use`), or Cooper–Harvey–Kennedy / Lengauer–Tarjan Dominator Tree (`dom`):
```bash
rgctl -f json inspect OrderService::createOrder cfg
rgctl -f json inspect OrderService::createOrder pdg --edge-layer data --def-use
rgctl -f json inspect OrderService::createOrder dom
```

#### `rgctl metrics` & `rgctl communities`
- **Pull-based CSR PageRank (`--pagerank`)**: Automatically filters out `Test`, `Generated`, and `ExternalStub` nodes so production hotspots rank first (pass `--include-tests` to include test helpers).
- **Sampled Brandes Betweenness (`--betweenness`)**: Identifies architectural bridge functions across communities.
- **Fast-Leiden Communities (`--communities` / `rgctl communities list|label`)**: Connected-subgraph modularity clustering with pre-clustering $\mu + 4\sigma$ hub stripping.
- **Git Churn Hotspots (`--churn`)**: Ranks files and functions by recency-weighted `git_hotspot_score` (from `discover --with-git`).
```bash
rgctl -f json metrics --pagerank --betweenness --communities
rgctl -f json metrics --churn
rgctl -f json communities list
```

#### `rgctl clones` (Type-1 Exact, Type-2 Bloom LSH, Type-3/4 SESE+1-WL Fragments)
- `--mode exact` (default): Type-1 identical `code_hash` function groups.
- `--mode bloom`: Type-2 64-bit token-sketch Bloom LSH Jaccard candidates (`--threshold 0.85`).
- `--mode fragment`: Two-stage sub-function clone detection (Stage 1 bitwise Bloom popcount pre-filter + Stage 2 Single-Entry Single-Exit hammock decomposition & 2-iteration 1-WL canonical color refinement):
```bash
# Repo-wide exact or Bloom function clones
rgctl -f json clones --mode exact --min-loc 10
rgctl -f json clones --mode bloom --threshold 0.85

# Sub-function SESE + 1-WL fragment clones seeded by symbol or file:start-end
rgctl -f json clones --mode fragment --seed "PaymentService::processRefund" --min-statements 4
rgctl -f json clones --mode fragment --seed "src/payment.rs:40-65"
```

#### `rgctl semantic <index|query|distill>`
1-bit quantized binary Hamming semantic search with compiled vocabulary embeddings (`vocab-accumulate-v1`), optional call-graph Jacobi diffusion (`--diffuse`), late multi-signal rank fusion, and call/blast-radius neighborhood expansion (`--expand`):
```bash
rgctl semantic index --diffuse
rgctl -f json semantic query "validate JWT bearer token" --expand neighbors --expand-depth 2
rgctl -f json semantic query "order checkout payment" --scope community
```

#### `rgctl cpg <status|function|calls|mutations|flows|ast|export|pdg|slice>`
Unified Code Property Graph (`L_repo` call graph + `L_proc` CFG/PDG/mutation/AST archives):
```bash
rgctl -f json cpg status
rgctl -f json cpg mutations --type OrderDTO --exclude-ctors --member status
rgctl -f json cpg flows src/OrderService.java --line 85 --variable order --with-alias
rgctl -f json cpg ast OrderService::submitOrder
rgctl cpg export --format graphson --output cpg.graphson --path-contains "order"
```

---

### 3.4 Security & Supply-Chain Commands (`vuln`, `deps`, `taint`)

```bash
# 1. Normalize an OSV advisory JSON
rgctl -f json vuln triage --osv GHSA-xxxx.json

# 2. Check manifest & lockfile dependency matches (plus optional JARs / node_modules)
rgctl -f json deps check --osv GHSA-xxxx.json --include-jars lib/

# 3. Full OSV reachability pipeline (manifest match -> import/callers -> PDG taint -> OpenVEX)
rgctl -f json vuln analyze --osv GHSA-xxxx.json

# 4. Sink-first backward taint reachability
rgctl -f json taint --sink "ObjectMapper.readValue" --source external --depth 8
```

---

### 3.5 Policy, PR Review & Governance Commands (`review`, `check`, `rules`)

#### `rgctl review <paths|check|export>`
- `rgctl review paths`: Computes structural before/after call-path deltas between a base snapshot/ref (`--base-ref origin/main`) and head (`--head-ref HEAD` or `--synthetic-head worktree`), reporting upstream caller spines, downstream callee spines, and blast-radius shifts.
- `rgctl review check`: Runs the scoped temporal PR policy gate (`New` vs `Existing` vs `Resolved` violations, calendar grace windows, violation ratchet ledger, and optional `--bisect` commit attribution).
- `rgctl review export`: Emits a single versioned JSON review envelope (`.rgctl/workflows/review.json` or `--stdout`) combining call-path deltas and policy evaluation.
```bash
rgctl -f json review paths --base-ref origin/main --head-ref HEAD
rgctl -f json review check --policy-file policy.json --base-ref origin/main --strict-calendar
rgctl -f json review export --base-ref origin/main --policy-file policy.json --stdout
```

#### `rgctl check` & `rgctl rules run`
```bash
# Whole-repo static policy check (or temporal with --temporal)
rgctl -f json check --policy-file policy.json --strict

# Konveyor Kantra ruleset evaluation against the session graph
rgctl -f json rules run ./rulesets/springboot --target quarkus
```

---

### 3.6 Meta Commands (`export`, `install`)

```bash
# Export graph to JSON, GraphML, DOT (Graphviz), Mermaid, or Obsidian vault
rgctl export --export-format mermaid --export-output architecture.mmd

# Install bundled AI agent skills (Cursor, Claude Code, Codex, AGENTS.md)
rgctl install --skill --tools all
```
