# Groovy extraction honesty

OpenSpec: [`openspec/changes/add-kotlin-groovy-tier1-language-support/`](../openspec/changes/add-kotlin-groovy-tier1-language-support/).

Grammar pin: **`tree-sitter-groovy` 0.1.2** (compatible with workspace `tree-sitter` **0.25**).

## Ingest routing

| Path | Route |
|------|--------|
| `*.groovy` | Groovy language plugin |
| `*.gradle` (scripts, other) | Groovy language plugin when registered |
| **`build.gradle`** (basename) | **Manifest only** — Dependency regex extractors |

Manifest basename wins over language extension mapping.

## Dynamic language limits

Groovy is highly dynamic (MOP, `metaClass`, `GString`, `evaluate`). Tier 1 still requires:

- Symbols for classes / methods / identifiable closures
- `Calls` where callee name is **syntactic** (same-class `helper()`, `Type.method(...)`)
- **No invented** call targets for pure dynamic dispatch — mark unresolved / omit edge

## FQN

- Package from `package_declaration` when present.
- Methods: `Type.method`; constructors: `Type.<init>` when `constructor_declaration` is present **or** when a `method_declaration` is named after the enclosing class (common Groovy grammar shape).

## Taint

Script-style sinks (`Runtime.exec`, process builders, SQL concat) are pattern-based. `GString` / `evaluate()` flows are best-effort with honesty — not full string-solver.

## Layer F (field writes)

- Java-shaped `field_access` LHS on `assignment_expression` → CFG `DefVar::Field`.
- Typed locals/params via `field_write_locals` (`visit_groovy`); dynamic/`def` locals without an explicit type stay unresolved.

## Non-goals

- Full Gradle DSL / version catalog resolution (manifest route remains separate).
- Complete MOP / ExpandoMetaClass call graphs.
