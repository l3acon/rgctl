---
name: rgctl-impact
description: "Blast radius and impact. Use for rgctl impact workflow. Spawn rgctl -f json; parse schema_version from stdout."
rgctl-managed: true
metadata:
  generatedBy: "rgctl 0.0.0-dev"
---

# Impact workflow

**When:** Before refactors, renames, or API changes.

### Blast radius

**User intent:** *"What's the impact if I change the signature of `updateQuantity`?"*

```bash
rgctl -r "$REPO" -f json blast-radius updateQuantity --depth 2
```

Report `metrics.score`, `topology.direct_callers`, impact size. Add `--class` / `--file` if ambiguous.

### Relationship between two symbols

**User intent:** *"What's the relationship between A and B?"*

1. Resolve symbols → bounded CALLS/DEPENDSON traversal
2. Report hops, shared neighbors, files
3. If no direct path but asymmetric dependency, fall back to `blast-radius` on each


## Agent loop

1. Parse the user question (natural language).
2. Run `rgctl -f json <command> …` (or `rgctl serve` + HTTP for repeated queries).
3. Parse `schema_version` and payload from **stdout** only.
4. Summarize facts; do not dump raw JSON.
5. Re-query if the graph may be stale after edits.

**Never** redirect stderr to `/dev/null`. If `.rgctl/` exists and the question is structural, use rgctl before ripgrep or bulk file reads.

```bash
export REPO=/path/to/repo
rgctl -r "$REPO" -f json <command>
```

