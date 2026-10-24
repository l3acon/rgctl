# Discovering and Indexing a Codebase

## Introduction

The `discover` command is the foundation of every rgctl workflow. It parses your source code, builds a **code knowledge graph** of functions, classes, modules, and their relationships (calls, contains, imports), and runs configurable analytics on the result. Every other rgctl command — `find`, `blast-radius`, `metrics`, and the rest — reads from the graph that `discover` produces.

Think of `discover` as the indexing step: you run it once (or after significant code changes), and then query the graph as many times as you like without re-parsing.

## Use Cases

- **Onboarding to an unfamiliar codebase.** Run `discover` to build an inventory of every function, class, and call relationship so you can query the structure instead of reading files one by one.
- **Pre-migration analysis.** Enable `--with-harmonic` and `--export-migration-hints` (optionally `--with-cfg`) to generate a dependency-aware migration roadmap before refactoring a monolith.
- **CI integration.** Run `discover` in CI so downstream commands like `check` / `review check` can enforce architectural policies on every pull request.
- **Deep program analysis.** Enable `--with-cfg` to pre-build the whole-repo v3 CFG/PDG archive and field-write index — required by `taint --sink`, `cpg mutations`, and richest `inspect`. **`slice` / `cpg flows` build single-function CFG/PDG lazily** without upfront `--with-cfg`.
- **Compiler-exact calls.** Overlay a SCIP index with `--scip <PATH.scip>` to upgrade ambiguous call edges (`resolution = "scip"`).
- **Git churn & co-change.** Enable `--with-git [--git-since 90.days]` for `CoChangesWith` edges and `metrics --churn`.
- **Multi-repo.** After per-repo discover, `rgctl merge` resolves parked cross-repo relations into one snapshot.

## Example Project

This guide uses the **CoolStore** — a Java EE e-commerce application. It lives in `example/coolstore` (local clone of [konveyor-ecosystem/coolstore](https://github.com/konveyor-ecosystem/coolstore); fetch scripts may place it at `example/coolstore-weblogic` — symlink or `-r` accordingly). Prefer **`-l java`** for the backend; the tree also ships a large Angular/bower frontend.

## Choosing what to index

`discover` takes an optional `PATH` argument. How it combines with `-r` / `--repo` matters:

| Pattern | Indexes | When to use |
|---------|---------|-------------|
| `cd example/coolstore && rgctl discover .` | Current directory | **Recommended** in tutorials and day-to-day use |
| `rgctl -r example/coolstore discover` | The `-r` path (no `PATH` arg) | Scripts, agents (`export REPO=…`) |
| `rgctl discover /abs/path/to/coolstore` | Absolute path | One-shot from any cwd |

**Pitfall:** `rgctl -r example/coolstore discover .` does **not** index `example/coolstore`. The positional `.` becomes the session root (usually your **shell cwd**), so `-r` is ignored. That can scan the wrong tree and fail on large parent directories.

**Artifacts:** `discover` writes snapshots under **`{repo}/.rgctl/`**. Add `.rgctl/` to `.gitignore`.

## Step-by-Step

### 1. Basic Discovery

From the repository root:

```bash
cd example/coolstore
rgctl discover . -l java
```

Or from anywhere, without a `PATH` argument:

```bash
rgctl -r example/coolstore discover -l java
```

### Discover profiles (recommended flag sets)

| Goal | Suggested flags |
|------|-----------------|
| **Structural** (query find/callers/…) | `discover .` (default columnar `graph.snapshot.bin`) |
| **Slice / flows** | structural discover is enough (lazy single-fn CFG/PDG); optional `--with-cfg` to pre-warm |
| **Taint / mutations / batch inspect** | `--with-cfg` (+ `--with-taint` / `--with-ast-skeleton` if needed) |
| **SCIP overlay** | `--scip index.scip` |
| **Git churn / co-change** | `--with-git [--git-since 90.days]` then `metrics --churn` |
| **Full staged pipeline** | `--full` (CFG + dashboard + harmonic + semantic; not taint/security) |
| **Migration roadmap** | `--export-migration-hints --with-harmonic` (+ preset/order) |
| **Kantra** | `--with-kantra` (+ `--kantra-target` …) |
| **Constrained / cgroup** | `--with-limits max-mem-mb=4096,threads=1` (or `RGCTL_WITH_LIMITS`) |
| **Dirty tree refresh** | prefer `rgctl update` / `rgctl watch` / `serve --watch` — not full rediscover |

`rgctl discover --help` groups flags under Paths & filters · Pipeline features · Migration · Session / limits.

### Full pipeline

```bash
rgctl -r example/coolstore discover -l java --full
```

This prints a plan, finishes a basic (queryable) index, then runs CFG + dashboard + harmonic centrality and a vocab semantic index. `--full` does not enable taint or secret scanning.

### Constrained / container discover

```bash
rgctl discover . --with-limits max-mem-mb=4096,threads=1
```

Or set `RGCTL_WITH_LIMITS=max-mem-mb=4096,threads=1`. Soft RSS budget (~95% abort), thread cap, and smaller spill buffers — default discover stays unconstrained.

**Output:**

```
[>] rgctl discover
[✓] Loaded 26 files from snapshot -> 608 nodes, 1709 edges
[✓] rgctl discover finished in ~300ms
```

**What happened:**

- With `-l java`, rgctl indexed CoolStore’s Java sources only (**26** files → **608** nodes / **1709** edges; inventory reports **152** functions on the current tree).
- The graph snapshot was written to `example/coolstore/.rgctl/graph.snapshot.bin`.
- Basic discover finishes in well under a second on this corpus.

### 2. SCIP overlay and git churn

```bash
# Compiler-exact call resolution overlay (optional .scip protobuf)
rgctl -r example/coolstore discover -l java --scip /path/to/index.scip

# Recency-weighted churn + CoChangesWith edges
rgctl -r example/coolstore discover -l java --with-git --git-since 90.days
rgctl -r example/coolstore -f json metrics --churn
rgctl -r example/coolstore -f json relations SomeFile.java --edge cochangeswith
```

Artifacts: `.rgctl/analysis/git_churn.json`. Co-change edges are excluded from default call traversal / PageRank.

### 3. Discovery with Deep Analysis (`--with-cfg`)

To **pre-build** control-flow graphs, PDGs, dominance, and the field-write index for every function, add `--with-cfg`. Needed for `taint --sink`, `cpg mutations`, and batch `inspect`. Not required for `slice` / `cpg flows` (lazy append).

```bash
rgctl -r example/coolstore discover -l java --with-cfg
```

**Output:**

```
[>] rgctl discover
[✓] Loaded 26 files from snapshot -> 608 nodes, 1709 edges

✓ Control flow analysis:
  Field writes indexed: 41
  CFG/PDG/Dominance: 150 functions analyzed
  Skipped: 2 functions (unsupported_language=0, missing_source=0, analysis_error=2)
[✓] rgctl discover finished in ~300ms
```

**What happened:**

- In addition to the basic graph, rgctl built a CFG, PDG, and dominator tree for each analyzed Java function (**150** on this tree; inventory still lists **152** function symbols).
- It indexed **41** field-write sites (`cpg status.field_write_count`), enabling `cpg mutations`.
- The v3 analysis archive was written to `.rgctl/analysis/cfg_pdg.archive.bin` (`cfg_pdg.bin` TOC layout).
- Per-file extraction facts land under `.rgctl/facts/` for incremental update/watch reuse.
- A small number of functions may be skipped for analysis errors.

### 4. Multi-repo merge

Park unresolved cross-repo calls/imports/routes during each repo’s discover (`parked_relations.bin`), then resolve:

```bash
rgctl discover services/auth
rgctl discover services/billing
rgctl merge services/auth services/billing --output-dir .rgctl-merged
rgctl -r .rgctl-merged -f json callers validateToken
```

Four-tier resolution: `ExactFqn` → `ImportScopedSuffix` → `UniqueBareName` → `AmbiguousStub`.

### 5. Full Analysis with Dashboard and Migration

For the most complete analysis, combine all the deep-analysis flags:

```bash
rgctl -r example/coolstore discover -l java \
  --with-cfg \
  --with-dashboard \
  --with-harmonic \
  --export-migration-hints
```

This enables:

| Flag | Purpose |
|------|---------|
| `--with-cfg` | Build CFG, PDG, and dominator trees for every function |
| `--with-dashboard` | Export a static dashboard bundle to `.rgctl/dashboard/` |
| `--with-harmonic` | Compute harmonic centrality (needed for migration ranking) |
| `--export-migration-hints` | Write `.rgctl/migration_plan.json`; with `-f json`, stdout is the migration plan (`schema_version` 2, `steps`) instead of DiscoverResponse telemetry |

Without `--export-migration-hints`, `discover -f json` emits DiscoverResponse telemetry (`command: "discover"`, `metrics`, `schema_version: 2`) — not the migration plan.

### 6. Filtering by Language

CoolStore tutorials in these guides use `-l java` (same as `--languages java`) so the Angular/bower frontend is not indexed:

```bash
rgctl -r example/coolstore discover --languages java
```

Omit `-l` / `--languages` only when you intentionally want a multi-language index of the full tree (much larger; vendor JS dominates).

**Language catalog:** Tier-1 deep-extraction & CFG/PDG languages ship with 100% grammar-verified `*-ast-coverage.json` manifests; the full plugin catalog in `languages.toml` covers **~60** languages and config/IaC formats (not all are Tier-1 deep analysis).

### 7. Excluding Directories

To skip vendor or generated code when indexing more than Java:

```bash
rgctl -r example/coolstore discover --exclude bower_components
```

### 8. Inspecting Artifacts

After discovery, artifacts live under **`{repo}/.rgctl/`**:

| Path | Content |
|------|---------|
| `.rgctl/graph.snapshot.bin` | Default **RGCL** v2 columnar mmap snapshot |
| `.rgctl/facts/` | Per-file `FileExtractionFacts` cache (incremental update/watch) |
| `.rgctl/parked_relations.bin` | Unresolved cross-repo relations (`RBPK`) for `merge` |
| `.rgctl/analysis/cfg_pdg.archive.bin` | v3 CFG/PDG/dominance archive (`--with-cfg` and/or lazy slice/flows append) |
| `.rgctl/analysis/git_churn.json` | Git churn overlay (`--with-git`) |
| `.rgctl/dashboard/manifest.json` | Dashboard metadata (with `--with-dashboard`) |
| `.rgctl/migration_plan.json` | Migration roadmap (with `--export-migration-hints`) |

`--write-json-graph` is an **opt-in** legacy export; it is not the default discover output (`graph.db` / `graph.json` are not the primary store).

## Discover Options Reference

| Option | Description |
|--------|-------------|
| `--with-cfg` | Pre-build per-function CFG, PDG, dominators + field-write index |
| `--with-taint` | Discover-time taint analysis (implies `--with-cfg`) |
| `--with-dfg-loops` | Classify loop-carried data dependencies on PDGs |
| `--with-ast-skeleton` | Write coarse AST skeleton archive |
| `--scip <PATH.scip>` | SCIP protobuf overlay for compiler-exact call resolution |
| `--with-git` / `--git-since` | Co-change edges + churn hotspots (default window `90.days`) |
| `--with-security` | Secret scanning |
| `--with-dashboard` | Export static dashboard bundle |
| `--with-harmonic` | HyperBallHarmonic centrality for migration ranking |
| `--export-migration-hints` | Write migration roadmap JSON (and emit it on stdout with `-f json`) |
| `--with-limits …` | Soft RSS / thread / spill budgets (containers) |
| `--write-json-graph` | Opt-in legacy JSON graph files |
| `-l, --languages` | Restrict to specific languages |
| `-e, --exclude` | Exclude directories by name |
| `-v, --verbose` | Debug logging with stage profiling |
| `--full` | Staged pipeline: basic → CFG/dashboard/harmonic → semantic index |

Also: `rgctl merge <REPOS>... --output-dir <DIR>` (Lifecycle) after per-repo discovers.

## Benefits

- **Single command to index an entire codebase.** No build system integration, no compilation required — rgctl works directly on source files.
- **Incremental depth.** Start with basic discovery in under a second; use lazy `slice`/`flows` or opt into `--with-cfg` / `--with-taint` only when you need whole-repo procedural archives.
- **Multi-language support.** Tier-1 deep-analysis languages plus the full ~60-plugin catalog in `languages.toml`.
- **Foundation for all other commands.** Every query, metric, and analysis reads from the graph `discover` produces.

## Related Guides

- [Structured graph queries](structured-query.md) — query the graph that `discover` builds
- [Watch mode and incremental update](watch-mode.md) — `update` / `watch` / `serve --watch`
- [Graph Metrics](graph-metrics.md) — PageRank, betweenness, Fast-Leiden, churn
- [Program Slicing](program-slicing.md) — lazy CFG/PDG slicing
- [Migration Planning](migration-planning.md) — use the `--export-migration-hints` output
- [HTTP Server and Dashboard](http-server-and-dashboard.md) — serve the `--with-dashboard` output in a browser
- [CLI Command Reference](cli-command-reference.md) — full Lifecycle flag map
