---
name: rgctl-gate
description: "CI and policy gates. Use for rgctl gate workflow. Spawn rgctl -f json; parse schema_version from stdout."
rgctl-managed: true
metadata:
  generatedBy: "rgctl 0.0.0-dev"
---

# Gate workflow

**When:** Policy checks and temporal PR gates.

### Policy check

**User intent:** *"Validate changes against project policies before committing"*

```bash
rgctl -r "$REPO" -f json check --policy-file policy.json
```

Blast-radius policy schema (`max_impact_nodes`, `forbidden_crossings`, …) — see [docs/policy-format.md](../../docs/policy-format.md). Named rules like `no-controller-direct-db-access` are **not** built-in ids. Report `passed` + `violations`.

### Temporal PR gate

```bash
rgctl -r "$REPO" -f json pr-check --policy-file rgctl-pr-policy.json --base-ref origin/main --head-ref HEAD --strict
rgctl -r "$REPO" -f json check --temporal --policy-file policy.json --base-ref origin/main --head-ref HEAD
```

Exit code 1 means violations. Parse JSON for violation details.


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

