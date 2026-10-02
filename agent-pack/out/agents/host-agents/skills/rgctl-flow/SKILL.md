---
name: rgctl-flow
description: "Data flow and slices. Use for rgctl flow workflow. Spawn rgctl -f json; parse schema_version from stdout."
rgctl-managed: true
metadata:
  generatedBy: "rgctl 0.0.0-dev"
---

# Flow workflow

**When:** Slices, PDG, taint, CPG data flows. Requires `discover --with-cfg`.

Check readiness: `rgctl -f json cpg status`.

### AST skeleton

**User intent:** *"Inspect the AST skeleton of `updateQuantity` to check its structure"*

```bash
rgctl discover . --with-ast-skeleton
rgctl -f json cpg ast updateQuantity
```

Coarse skeleton (`kind`, lines, `label`) — **not** a typed signature API (`params` / `return_type` are not emitted).

### Status + line slice

**User intent:** *"Confirm the CFG archive is ready, then slice how `quantity` is used in `updateQuantity`"*

```bash
rgctl -f json cpg status
rgctl -f json cpg slice src/cart/CartService.ts \
  --line 50 --variable quantity --function updateQuantity --view pdg
```

**`cpg slice` has no `--symbol`.** For whole-function CFG/PDG, use `inspect <Symbol> cfg|pdg` or `cpg pdg <Symbol>`.

CLI alias: `rgctl -f json slice FILE --line N --variable V [--function F] [--direction backward|forward]`.

### Field mutations

**User intent:** *"Check where `ShoppingCart` object fields are mutated"*

```bash
rgctl -f json cpg mutations --type ShoppingCart --exclude-ctors
```

### Data flows

**User intent:** *"Trace how the `quantity` variable flows into database queries"*

```bash
rgctl -f json cpg flows src/cart/CartService.ts \
  --line 50 --variable quantity --function updateQuantity --direction forward
```

### Loop-carried DFG

**User intent:** *"Check for loop-carried dependencies that prevent parallelization"*

```bash
rgctl discover . --with-cfg --with-dfg-loops
rgctl -f json inspect BatchProcessor.process pdg --edge-layer data
```

`--with-dfg-loops` **tags** edges during discover — it does not print a dedicated loop-hazard array. Look for `loop_carried` on PDG data deps.


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

