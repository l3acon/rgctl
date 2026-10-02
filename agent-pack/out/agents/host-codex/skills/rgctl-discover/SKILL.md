---
name: rgctl-discover
description: "Index and discover. Use for rgctl discover workflow. Spawn rgctl -f json; parse schema_version from stdout."
rgctl-managed: true
metadata:
  generatedBy: "rgctl 0.0.0-dev"
---

# Discover workflow

**When:** First use, rebuild after large changes, or incremental `--files` update.

| Intent | Command |
|--------|---------|
| Index repo | `cd "$REPO" && rgctl discover .` or `rgctl -r "$REPO" discover` |
| Full pipeline | `discover . --full` |
| Incremental | `discover --files path1,path2` (requires existing `.rgctl/`) |

**Fast path:** If `.rgctl/` exists and the user did not ask to rebuild, do **not** re-run discover.

Common flags: `--with-cfg` (CFG/PDG archive), `--with-ast-skeleton`, `--with-dfg-loops` (loop-carried PDG tags). Migration plan output is the **migrate** workflow; Konveyor rules are the **kantra** workflow — do not conflate them with a plain index.

Artifacts live at `{repo}/.rgctl/`. Check CFG readiness with `rgctl -f json cpg status` before slice/PDG workflows.


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

