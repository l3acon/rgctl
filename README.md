# rgctl - Code knowledge graph for LLM agents.

[![Release](https://img.shields.io/github/v/release/sshaaf/rgctl?style=for-the-badge&logo=github&color=0ea5e9)](https://github.com/sshaaf/rgctl/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/sshaaf/rgctl/total?style=for-the-badge&logo=github&color=22c55e)](https://github.com/sshaaf/rgctl/releases)
[![Stars](https://img.shields.io/github/stars/sshaaf/rgctl?style=for-the-badge&logo=github)](https://github.com/sshaaf/rgctl/stargazers)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?style=for-the-badge)](LICENSE)

[![Docs](https://img.shields.io/badge/docs-shaaf.dev%2Frgctl-2563eb?style=flat-square&logo=readthedocs&logoColor=white)](https://shaaf.dev/rgctl)
[![Website](https://img.shields.io/github/actions/workflow/status/sshaaf/rgctl/website.yml?branch=main&style=flat-square&label=website)](https://shaaf.dev/rgctl)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-555?style=flat-square)](https://github.com/sshaaf/rgctl/releases/latest)
[![tree-sitter](https://img.shields.io/badge/parser-tree--sitter-brightgreen?style=flat-square)](https://tree-sitter.github.io/tree-sitter/)
[![Agents](https://img.shields.io/badge/agents-Cursor%20%7C%20Claude%20%7C%20Codex%20%7C%20Antigravity-111827?style=flat-square)](https://shaaf.dev/rgctl/docs/guides/agent-skill/)
[![Tier 1](https://img.shields.io/badge/languages-14%20Tier%201-8b5cf6?style=flat-square)](https://shaaf.dev/rgctl/docs/languages/)

[![C](https://img.shields.io/badge/C-A8B9CC?style=flat-square&logo=c&logoColor=black)](docs/languages/README.md)
[![C++](https://img.shields.io/badge/C%2B%2B-00599C?style=flat-square&logo=cplusplus&logoColor=white)](docs/languages/README.md)
[![C#](https://img.shields.io/badge/C%23-512BD4?style=flat-square&logo=csharp&logoColor=white)](docs/languages/README.md)
[![Go](https://img.shields.io/badge/Go-00ADD8?style=flat-square&logo=go&logoColor=white)](docs/languages/README.md)
[![Groovy](https://img.shields.io/badge/Groovy-4298B8?style=flat-square&logo=apachegroovy&logoColor=white)](docs/languages/README.md)
[![Java](https://img.shields.io/badge/Java-ED8B00?style=flat-square&logo=openjdk&logoColor=white)](docs/languages/README.md)
[![JavaScript](https://img.shields.io/badge/JavaScript-F7DF1E?style=flat-square&logo=javascript&logoColor=black)](docs/languages/README.md)
[![Kotlin](https://img.shields.io/badge/Kotlin-7F52FF?style=flat-square&logo=kotlin&logoColor=white)](docs/languages/README.md)
[![PHP](https://img.shields.io/badge/PHP-777BB4?style=flat-square&logo=php&logoColor=white)](docs/languages/README.md)
[![Python](https://img.shields.io/badge/Python-3776AB?style=flat-square&logo=python&logoColor=white)](docs/languages/README.md)
[![Ruby](https://img.shields.io/badge/Ruby-CC342D?style=flat-square&logo=ruby&logoColor=white)](docs/languages/README.md)
[![Rust](https://img.shields.io/badge/Rust-000000?style=flat-square&logo=rust&logoColor=white)](docs/languages/README.md)
[![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=flat-square&logo=typescript&logoColor=white)](docs/languages/README.md)
[![Puppet](https://img.shields.io/badge/Puppet-FFAE1A?style=flat-square&logo=puppet&logoColor=black)](docs/languages/README.md)
[![Markdown](https://img.shields.io/badge/Markdown-000000?style=flat-square&logo=markdown&logoColor=white)](docs/guides/markdown-context-graph.md)

> Index once (`discover`), then ask callers, blast radius, control/data-flow slices, clones, communities, and PR call-path deltas — compact deterministic JSON for agents, not grepping the tree.

**What the R stands for:** **R**ust · **R**eachability · **R**ich graph (30+ typed relations, Hybrid CPG, mmap snapshots).

### Without `rgctl` vs with `rgctl`

| Dimension | Without (grep / dump files) | With `rgctl` |
|-----------|-----------------------------|--------------|
| **Tool calls for “who calls X transitively?”** | Many greps + ad-hoc file reads | 1–2 (`discover` once, then `callers` / `blast-radius`) |
| **Context tokens** | Whole files / search dumps into the prompt | Compact `-f json` payloads (`schema_version`) |
| **Transitive reachability & PR review** | Easy to miss or hallucinate | Precomputed SCC reachability, `blast-radius`, `review paths` |
| **Control & data flow** | Manual line-by-line tracing | Statement-level `slice`, `cpg flows`, `inspect` (CFG / PDG / dominance) |
| **Error recovery for agents** | Raw exit codes or unstructured stderr | Structured JSON error envelope (`error_code`, `candidates`, `recovery_command`) |

### Battle-tested at scale

Cold discover and zero-hydration mmap queries on real-world repositories (see [AGENTS.md](AGENTS.md) Gate A/B and [profile notes](docs/internal/profile.md)):

- **Linux kernel** (~71k C files): cold discover in **~134 s**
- **metasfresh** (Java EE `--full` with CFG/CPG + analytics): **~25.7 s**
- **Node.js** (`test/` JS `--with-cfg`): **~4.8 s**
- **Kubernetes**, **LLVM**, **Roslyn**, **rustc**, **Home Assistant**, **Magento 2**, **Discourse**, **VS Code**, **JetBrains/kotlin**, **Gradle**
- **14 Tier-1 languages** + Markdown/ERB, backed by memory-mapped columnar snapshots (`graph.snapshot.bin`) and incremental updates (`rgctl update` / `rgctl serve --watch`).

```bash
rgctl discover . --with-cfg
rgctl -f json status
rgctl -f json find --type function --limit 20
rgctl -f json callers MyService --depth 1
rgctl -f json blast-radius MyService --class OrderHandler
rgctl -f json review paths --base-ref origin/main --head-ref HEAD

# Install skills + /rgctl:review slash command into your coding agent
rgctl install --skill --tools cursor,claude,codex,antigravity,agents
```

https://github.com/user-attachments/assets/15ec6d91-f716-4cbd-a873-e982ba3c6dca

---

## Try it (5 minutes)

### 1. Install

**One-line release binary** (macOS / Linux):

```bash
curl -fsSL https://shaaf.dev/rgctl/install.sh | sh
rgctl --version
```

Installs to `${RGCTL_INSTALL_DIR:-$HOME/.local/bin}`. Or download a prebuilt archive from [GitHub Releases](https://github.com/sshaaf/rgctl/releases/latest) and place `rgctl` on your `PATH`.

**Or build from source** (Rust **1.85+**):

```bash
git clone https://github.com/sshaaf/rgctl.git
cd rgctl
cargo build --release --locked --bin rgctl
# If ort/ONNX link fails on your platform: add --no-default-features
export PATH="$PWD/target/release:$PATH"
```

Details, PATH setup, and troubleshooting: **[Installation](docs/installation.md)**.

### 2. Index the in-tree demo

```bash
cd rgctl-tests/ecommerce-java   # or pass -r <PATH> from anywhere
rgctl discover . --with-cfg
rgctl -f json status            # shows graph, CFG/CPG, semantic & skill readiness
```

Artifacts land in `{repo}/.rgctl/`. Run `rgctl update` (or `rgctl serve --watch`) for fast incremental updates after code edits.

### 3. Ask the graph

```bash
# Inventory & symbol lookup
rgctl -f json inventory --by type
rgctl -f json find --type function --package com.ecommerce.service --limit 10

# Callers, callees & typed relations
rgctl -f json callers ProductService --depth 1
rgctl -f json relations --edge calls --limit 20

# Impact before you edit a symbol (disambiguate overloads with --file / --class / --line)
rgctl -f json blast-radius processOrder --class OrderService

# Control-flow, PDG & program slicing (requires discover --with-cfg)
rgctl -f json inspect OrderService::createOrder cfg
rgctl -f json slice --function OrderService::createOrder --line 42

# Duplicate & structural clone detection
rgctl -f json clones --mode exact --min-loc 5 --exclude test

# PR call-path impact vs base branch
rgctl -f json review paths --base-ref origin/main --head-ref HEAD
```

- **For agents & scripts:** Always pass **`-f json`** (or `RGCTL_FORMAT=json`). Both data and structured errors (`ok: false`, `error_code`, `candidates`, `recovery_command`) are emitted on `stdout` ([JSON API](docs/json-api.md)). Never scrape `stderr`.
- **For humans in a terminal:** Default text output formats symbols as `kind  name  file:line` when `stdout` is a TTY and stays pipe-friendly (`1` column) when piped.

---

## Use with coding agents

Install the bundled agent pack (`rgctl` + `rgctl-review` skills and the `/rgctl:review` slash command) into your IDE or CLI agent:

```bash
rgctl install --skill --tools cursor,claude,codex,antigravity,agents
```

- **`rgctl` skill:** Routes natural-language architecture, impact, slicing, clone, community, and migration questions to deterministic `rgctl -f json` commands.
- **`rgctl-review` skill (`/rgctl:review`):** Runs a local PR/branch structural review (`review paths`, `review check`, blast radius) and writes `REVIEW_REPORT.md` + a four-section verdict without posting to GitHub/GitLab.

See the [Agent pack guide](docs/guides/agent-skill.md). For *your* application repo, you can also copy [USER_AGENTS_TEMPLATE.md](docs/agents/USER_AGENTS_TEMPLATE.md) into `AGENTS.md`.

---

## What it does

Commands are organized into six groups (`rgctl --help`):

| Group | You need… | Command |
|-------|-----------|---------|
| **Lifecycle** | Index repo, incremental refresh, readiness check | `discover`, `update`, `status`, `serve --watch` |
| **Query** | Find symbols, callers, callees, typed edges, GQL | `find`, `inventory`, `callers`, `callees`, `relations`, `gql` |
| **Analysis** | Upstream impact (“what breaks if I edit X?”) | `blast-radius` |
| **Analysis** | CFG / PDG / dominance / backward & forward slices | `slice`, `inspect`, `cpg` (needs `discover --with-cfg`) |
| **Analysis** | Exact (Type-1), bloom & SESE/1-WL fragment clones | `clones` (`--mode exact \| bloom \| fragment`) |
| **Analysis** | Centrality hotspots & label-propagation clusters | `metrics`, `communities` |
| **Analysis** | Natural-language search over functions or docs | `semantic` (`index`, `query`) |
| **Security** | OSV advisories, lockfile deps, taint & OpenVEX | `security` (`vuln triage`, `vuln analyze`, `deps check`, `taint`) |
| **Policy & PRs** | PR call-path deltas & CI blast-radius gates | `review paths`, `review check` (`pr-check`), `check` |
| **Policy & Migration** | Konveyor Kantra rules, custom YAML rules, roadmap | `discover --with-kantra`, `kantra`, `rules`, `discover --export-migration-hints` |
| **Meta & Export** | Snapshot diff, subgraph export, browser UI | `diff`, `export` (JSON, GraphML, Mermaid, Obsidian), `serve` |

- **Step-by-step feature guides (CoolStore):** **[docs/guides](docs/guides/README.md)**
- **Concepts & architecture overview:** **[Introduction](docs/Introduction.md)**
- **Full CLI walkthrough (ecommerce-java):** **[User Guide](docs/user-guide.md)**

---

## Languages

**14 Tier-1 tree-sitter plugins:** **C, C++, C#, Go, Groovy, Java, JavaScript, Kotlin, PHP, Puppet, Python, Ruby, Rust, TypeScript**, plus **ERB** and **Markdown** (`.md` / `.mdx` document context graph).

Each language plugin’s AST coverage is tracked in `crates/rgctl-lang-*/{id}-ast-coverage.json` and rendered at [shaaf.dev/rgctl/docs/languages/](https://shaaf.dev/rgctl/docs/languages/) — see [Languages](docs/languages/README.md).

---

## Contributing

We welcome contributors across four tracks — see **[CONTRIBUTING.md](CONTRIBUTING.md)** to get started:

1. **Language Plugins & AST Coverage:** Add or deepen tree-sitter extractors ([contributor-checklist.md](docs/contributor-checklist.md), [language-testing-guide.md](docs/language-testing-guide.md)).
2. **Declarative Taint & Kantra Rules:** Expand security source/sink/sanitizer rules and migration rule packs ([vulnerability-and-osv-analysis.md](docs/guides/vulnerability-and-osv-analysis.md)).
3. **Agent Host Adapters & Skills:** Improve agent workflows, slash commands, and IDE host adapters ([agent-skill.md](docs/guides/agent-skill.md)).
4. **Graph & CPG Algorithms:** Work on CSR projections, reachability, dominance, SESE clone detection, and columnar storage ([analysis-architecture.md](docs/analysis-architecture.md), [graph-storage-architecture.md](docs/graph-storage-architecture.md)).

---

## Docs

| Doc | For |
|-----|-----|
| [Installation](docs/installation.md) | One-line install, build from source, PATH setup |
| [Introduction](docs/Introduction.md) | What / why / capability map |
| [Guides](docs/guides/README.md) | 19 step-by-step feature guides |
| [User Guide](docs/user-guide.md) | `ecommerce-java` + full CLI reference |
| [JSON API](docs/json-api.md) | `-f json` schemas, `schema_version`, error envelopes |
| [Docs index](docs/README.md) | Complete documentation map |
| [AGENTS.md](AGENTS.md) | Performance rules & cold gates for contributing to *this* repo |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contributor tracks, dev setup, test matrix |
| [Latest release](docs/releases/v0.4.21.md) · [Unreleased](docs/releases/unreleased.md) | Release notes & upcoming changelog |

---

## License

MIT — see [LICENSE](LICENSE).
