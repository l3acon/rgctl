# rgctl Skill

A skill for answering structural questions about codebases using the rgctl CLI graph.

## Quick Stats

- **Main skill:** router + structured verb tables
- **Reference files:** command encyclopedia, workflows (assembled), communities & policy
- **Workflow families:** discover, impact, flow, search, migrate, kantra, gate
- **Agent query path:** `find` / `callers` / `callees` / `relations` / `inventory` / `status` (no Cypher)

## Structure

```
skills/rgctl/
├── SKILL.md                              # Main skill
├── README.md                             # This file
├── workflows/                            # Source for workflow skills + references/workflows.md (assembled at build)
└── references/
    ├── command-encyclopedia.md           # All commands with JSON samples
    ├── workflows.md                      # Generated from workflows/ at build (do not edit by hand)
    └── communities-and-policy.md         # Community detection + CI policy
```

## What's Covered

### Main SKILL.md (Always Loaded)

- When to use rgctl
- **CLI subprocess workflow** — spawn `rgctl -f json` for agents
- **Workflow families:**
  1. Discovery & Indexing
  1b. Konveyor Kantra rules (`--with-kantra`)
  2. Query & Search (structured verbs + communities)
  3. Impact & Safety (includes policy checks)
  4. Metrics & Analysis
  5. Code Analysis (CFG/PDG/slicing)
  6. Export & Visualization
- **NL routing table** (user utterances → commands)
- Failure playbook

### References (Loaded On-Demand)

#### command-encyclopedia.md
- Structured query verbs and domain commands
- JSON sample responses
- Prerequisites and pitfalls
- "What to report" guidelines

#### workflows.md (generated)
- Assembled from `workflows/*.md` when the agent pack is built (`cargo build`)
- Edit fragments under `workflows/` (e.g. `migrate.md`, `kantra.md`); order comes from `agent-pack/manifest.yaml`

#### communities-and-policy.md
- **Community Detection:** list, semantic scope, ownership workflows
- **CI Policy Checks:** schema, CI integration, calibration

## Design Principles

✅ **Progressive disclosure** - Main skill lean, details in references
✅ **Workflow-centric** - Organized by user intent, not commands
✅ **CLI-first** - Agents use `rgctl -f json` structured verbs
✅ **Clear routing** - Natural language → command mapping
✅ **No Cypher in skill surface** - Agents must not invent MATCH strings

## Installation

From a target repository (not the rgctl source tree unless you are dogfooding):

```bash
rgctl install --skill --tools cursor,claude,codex,antigravity,agents
```

Installs meta skill `rgctl`, workflow skills (`rgctl-discover`, …). See [Agent pack walkthrough](../../docs/guides/agent-skill.md).

**Maintainers:** edit workflow bodies under `workflows/`; regenerate `references/workflows.md` with `assemble_workflows_reference` (see `rgctl-agent-pack-codegen` test `workflows_reference_matches_fragments`).

## See Also

- [User Guide](../../docs/user-guide.md) - Complete CLI tutorial
- [Agent recipes](../../docs/agent-recipes.md) - Copy-paste CLI workflows
- [HTTP Server and Dashboard](../../docs/guides/http-server-and-dashboard.md) - Optional `rgctl serve` for dashboard
- [JSON API](../../docs/json-api.md) - Schema specifications
- [All Guides](../../docs/guides/README.md) - Feature-specific guides
