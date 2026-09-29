# Kotlin extraction honesty

OpenSpec: [`openspec/changes/add-kotlin-groovy-tier1-language-support/`](../openspec/changes/add-kotlin-groovy-tier1-language-support/).

Grammar pin: **`tree-sitter-kotlin-ng` 1.1.0** (workspace `tree-sitter` **0.25**). Do **not** use crates.io `tree-sitter-kotlin` 0.3.8 — it requires `tree-sitter` &lt; 0.23 and conflicts with the workspace `links`.

## Ingest routing

| Path | Route |
|------|--------|
| `*.kt` | Kotlin language plugin |
| `*.kts` (scripts, other) | Kotlin language plugin |
| **`build.gradle.kts`** (basename) | **Manifest only** — Dependency extractors; language plugin must not win |

Registry checks `classify_ingest_path` **before** extension → language plugin so Manifest basenames stay exclusive (AGENTS.md / build-and-config graph).

## FQN rules

- Package from `package_header` → prefix for types and top-level functions.
- Nested types: `Outer.Inner`.
- Methods: `Type.method`; constructors: `Type.<init>` (`is_constructor: true`).
- Companion object members: qualify under companion / enclosing class as emitted (document in symbol metadata).

## Calls

Best-effort on `call_expression` / navigation call forms. No points-to; unresolved receivers stay without a false `Calls` edge where callee cannot be named.

## CFG / suspend

- Standard control flow: `if_expression`, `when_expression`, loops, `try_expression`.
- **`suspend` / coroutine interprocedural CFG** is honesty-limited in v1 (bodies still get intra-procedural CFG; no full continuation graph).

## Layer F (field writes)

- Assignments to `navigation_expression` (`order.status = …`, `this.status = …`) produce CFG `DefVar::Field` facts.
- Typed locals/params via `field_write_locals` (`visit_kotlin`) — formal `parameter` / `class_parameter` and typed `property_declaration` locals only (no inference).

## Non-goals (this change)

- Replacing Gradle manifest Dependency extraction.
- Code → Maven `Dependency` blast-radius edges.
- Full Kotlin reflection / reified generics resolution.
