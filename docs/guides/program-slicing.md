# Program Slicing

## Introduction

The `slice` command performs **line-level program slicing** -- it extracts the minimal set of statements in a function that affect (or are affected by) a specific variable at a specific line. This is a classic program analysis technique that lets you focus on exactly the code that matters for understanding a data flow, without reading the entire function.

Slicing works in two directions: **backward** (what statements contributed to this variable's value?) and **forward** (what statements does this variable's value influence?).

## Lazy CFG/PDG (no upfront `discover --with-cfg`)

`rgctl slice` does **not** require a whole-repo `discover --with-cfg` first. When the target function is missing from the v3 indexed archive (`.rgctl/analysis/cfg_pdg.bin` / on-disk `cfg_pdg.archive.bin`), `ensure_cfg_pdg_for_source` parses only that file, builds a **single function** CFG/PDG, and **atomically appends** its `TocEntry` to the archive.

- **`--function` is optional:** when omitted, `slice` auto-resolves the innermost enclosing function from `file:line`.
- **`--with-alias`:** expands the criterion via intra-procedural may-alias heuristics (copies / field bases).
- **`--taint`:** enables taint tracking on the slice criterion.
- Reaching definitions track lexical **`(name, scope_id)`** pairs so block shadowing does not create false edges, while preserving RHS-before-LHS for `let x = x + 1`.

Whole-repo **`rgctl taint --sink`** and **`rgctl cpg mutations`** still need `discover --with-cfg` (full archive / field-write index). Prefer `discover --with-cfg` when you will `inspect` many functions or run batch CPG mutation audits.

## Use Cases

- **Bug investigation.** Trace a variable's value backward to find where an incorrect computation originates.
- **Impact analysis at the statement level.** Before changing a line, see which other lines in the function would be affected.
- **Understanding complex functions.** Reduce a long function to just the statements relevant to a specific variable.
- **Security review.** Trace untrusted input forward to see where it flows within a function.
- **Code comprehension.** Focus on the minimal subset of code that produces a particular output.

## Example Project

This guide uses the **CoolStore** (`example/coolstore`). A structural index is enough to start slicing (lazy CFG/PDG fill-in):

CoolStore examples use `-l java` to index the Java backend only (skip Angular/bower).

```bash
rgctl -r example/coolstore discover -l java
# Optional: pre-build the whole-repo archive for faster multi-function inspect/mutations
# rgctl -r example/coolstore discover -l java --with-cfg
```

## Step-by-Step

### 1. Backward Slice

Find all statements that contribute to the value of `sc` at line 68 in `ShoppingCartService.java`. `--function` may be omitted (auto from line); shown here for clarity:

```bash
rgctl -r example/coolstore -f json slice \
  ./src/main/java/com/redhat/coolstore/service/ShoppingCartService.java \
  --line 68 --variable sc --direction backward
```

**Output:**

```json
{
  "criterion": {
    "line": 68,
    "variable": "sc"
  },
  "direction": "backward",
  "edges": [
    {
      "kind": "data",
      "source": "node_0",
      "target": "node_1",
      "variable": "sc"
    },
    {
      "kind": "data",
      "source": "node_0",
      "target": "node_1",
      "variable": "sci"
    },
    {
      "kind": "data",
      "source": "node_0",
      "target": "node_1",
      "variable": "getQuantity"
    }
  ],
  "file": "./src/main/java/com/redhat/coolstore/service/ShoppingCartService.java",
  "lines": [
    66,
    68
  ],
  "nodes": [
    {
      "id": "node_0",
      "kind": "Expression",
      "label": "sc.setCartItemPromoSavings(\n                            sc.getCartItemPromoSavings() + sci.getPromoSavings() * sci.getQuantity())",
      "line": 66
    },
    {
      "id": "node_1",
      "kind": "Expression",
      "label": "sc.setCartItemTotal(sc.getCartItemTotal() + sci.getPrice() * sci.getQuantity())",
      "line": 68
    }
  ],
  "reduction_percent": 83.33333333333334,
  "schema_version": 1
}
```

**What this tells you:**

- **`lines: [66, 68]`** -- only 2 lines out of the entire function are relevant to the value of `sc` at line 68.
- **`reduction_percent: 83.3%`** -- the slice eliminated 83% of the function's statements, leaving only the minimal relevant subset.
- **`nodes`** -- the actual code at each line. Line 66 sets `cartItemPromoSavings` on `sc`, and line 68 sets `cartItemTotal` -- both modify the `sc` object.
- **`edges`** -- the data-dependency edges connecting the nodes. The variable `sc` flows from node_0 to node_1, along with `sci` and `getQuantity`.
- The backward direction means: "what code contributes to `sc`'s state at line 68?"

### 2. Forward Slice with alias expansion

Trace how `sc` at line 68 influences later statements, expanding may-aliases:

```bash
rgctl -r example/coolstore -f json slice \
  ./src/main/java/com/redhat/coolstore/service/ShoppingCartService.java \
  --line 68 --variable sc --direction forward --with-alias
```

Equivalent façade (also lazy; `--function` optional):

```bash
rgctl -r example/coolstore -f json cpg flows \
  ./src/main/java/com/redhat/coolstore/service/ShoppingCartService.java \
  --line 68 --variable sc --direction forward --with-alias
```

**Output (flows shape):**

```json
{
  "direction": "forward",
  "file": "./src/main/java/com/redhat/coolstore/service/ShoppingCartService.java",
  "function": "priceShoppingCart",
  "line": 68,
  "lines": [64, 68, 81, 83],
  "reduction_percent": 66.66666666666667,
  "schema_version": 1,
  "steps": [
    {
      "code": "for-each sc.getShoppingCartItemList()",
      "line": 64
    },
    {
      "code": "sc.setCartItemTotal(sc.getCartItemTotal() + sci.getPrice() * sci.getQuantity())",
      "line": 68
    },
    {
      "code": "ps.applyShippingPromotions(sc)",
      "line": 81
    },
    {
      "code": "sc.setCartTotal(sc.getCartItemTotal() + sc.getShippingTotal())",
      "line": 83
    }
  ],
  "variable": "sc"
}
```

**What this tells you:**

- **`lines: [64, 68, 81, 83]`** -- 4 lines are influenced by `sc` at line 68.
- **`reduction_percent: 66.7%`** -- two-thirds of the function is irrelevant to this data flow.
- **`steps`** -- the flow path: the for-each loop (line 64), the cart item total calculation (line 68), shipping promotions (line 81), and the final cart total (line 83).
- This shows the complete computation chain from item pricing through to the final cart total.

## Slice Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `FILE` | Yes | Path to the source file (relative to `--repo`) |
| `--line` | Yes | Line number of the slicing criterion |
| `--variable` | Yes | Variable name at the criterion line |
| `--function` | No | Function name; when omitted, auto-resolved from `file:line` |
| `--direction` | No | `backward` (default) or `forward` |
| `--with-alias` | No | Expand criterion via may-alias heuristics (copies / field bases) |
| `--taint` | No | Enable taint tracking on the slice |
| `--view` | No | `text` (default), `cfg`, or `pdg` |
| `--language` | No | Override language detection |

## Understanding the Output

| Field | Meaning |
|-------|---------|
| `lines` | The line numbers included in the slice |
| `reduction_percent` | How much of the function was eliminated (higher = more focused) |
| `nodes` | PDG nodes with code labels and line numbers |
| `edges` | Data and control dependency edges between nodes |
| `criterion` | The starting point (line + variable) of the slice |

## Benefits

- **Precision.** Only see the statements that actually matter for a specific data flow.
- **On-demand CFG/PDG.** First slice of a function builds and caches the v3 archive entry — no whole-repo `--with-cfg` tax.
- **Quantified focus.** The `reduction_percent` tells you exactly how much irrelevant code was eliminated.
- **Bidirectional.** Trace data backward to its origins or forward to its consumers.
- **Statement-level granularity.** More precise than function-level blast-radius analysis.
- **Bug isolation.** Quickly narrow down where a value was computed incorrectly.

## Related Guides

- [Discovering and Indexing a Codebase](discovering-and-indexing.md) -- structural `discover` is enough; `--with-cfg` is optional pre-warm
- [Hybrid CPG](hybrid-cpg.md) -- `cpg flows` wraps slicing with a higher-level interface
- [Inspecting CFG, PDG, and Dominance](inspecting-cfg-pdg-dominance.md) -- examine the raw PDG that slicing uses
- [Blast Radius Analysis](blast-radius-analysis.md) -- function-level impact analysis (vs. statement-level slicing)
- [CLI Command Reference](cli-command-reference.md) -- `slice` / `taint` / `cpg` map
