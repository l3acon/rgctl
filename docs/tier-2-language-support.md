# Tier 2 language support — declarative `tags.scm` query plugins

This document explains how **Tier 2** languages (`handler = "queries"`) work in rgctl: how [`QueryLanguagePlugin`](../crates/rgctl-lang-runtime/src/query_plugin.rs) compiles and executes tree-sitter `queries/tags.scm` files, how symbols, containers, imports, and call edges are derived from query captures, and how to add or promote a Tier 2 language.

**Audience:** contributors adding a new Tier 2 language, tuning `queries/tags.scm` extraction, or evaluating when to promote a language to [Tier 1](tier-1-language-support.md).

**Related docs:** [tier-1-language-support.md](tier-1-language-support.md) (custom `LanguagePlugin` + CFG/PDG/taint + Layer F), [language-testing-guide.md](language-testing-guide.md), [languages/README.md](languages/README.md).

---

## 1. Tier 1 vs Tier 2 overview

rgctl supports **~69 language plugins** across two primary tree-sitter tiers (plus structural mainframe extractors) configured in [`languages.toml`](../languages.toml):

| Capability | Tier 1 (`handler = "custom"`) | Tier 2 (`handler = "queries"`) |
|------------|-------------------------------|--------------------------------|
| **Plugin implementation** | Custom Rust [`LanguagePlugin`](../crates/rgctl-plugin-api/src/lib.rs) per crate (`src/plugin.rs`) | Shared [`QueryLanguagePlugin`](../crates/rgctl-lang-runtime/src/query_plugin.rs) driven by `queries/tags.scm` + [`LanguageConfig`](../crates/rgctl-lang-runtime/src/config.rs) |
| **Grammar verification** | `{id}-ast-coverage.json` manifest mapping every named AST node kind | `rgctl_ast_coverage::check_tags_scm` capture vocabulary gate + grammar compilation test |
| **Symbols extracted** | Functions, methods, classes, interfaces, structs, enums, modules, fields, constructors, imports | Functions, methods, classes, interfaces, modules, namespaces, types, enums, macros, fields, constants, imports |
| **Container qualification** | AST scope stack (`ScopeStack`) + package/module FQN | Line-span containment (`Container::member`) via binary search (`partition_point`) |
| **Relations (`Calls`, `References`, `Implements`)** | Custom AST walkers + receiver/type hints + MRO (`Overrides`) + framework DI | `@reference.call` → `Calls` (attributed to innermost enclosing function/method), `@reference.class` → `References`, `@reference.implementation` → `Implements`, `@import.path` → `Import` |
| **CFG / PDG / taint / Layer F** | **Required** (`--with-cfg`, `--with-taint`, `cpg mutations`) | Not enabled (structural graph, call graph, blast radius, communities, and search work out of the box) |

---

## 2. How `QueryLanguagePlugin` works (`rgctl-lang-runtime`)

Every Tier 2 crate (`crates/rgctl-lang-{id}/`) embeds its `queries/tags.scm` at compile time (`include_str!("../queries/tags.scm")`) inside a static [`LanguageConfig`](../crates/rgctl-lang-runtime/src/config.rs) and registers a [`QueryLanguagePlugin`](../crates/rgctl-lang-runtime/src/query_plugin.rs):

1. **One-time query compilation & vocabulary validation (`QueryLanguagePlugin::from_config`)**:
   - Compiles `tags.scm` against the language's `tree_sitter::Language` via `tree_sitter::Query::new`.
   - Builds a [`CaptureIndex`](../crates/rgctl-lang-runtime/src/queries.rs), verifying that every `@capture` name in `tags.scm` is in [`ALLOWED_TAG_CAPTURES`](../crates/rgctl-lang-runtime/src/queries.rs) and that at least one `@definition.*`, `@reference.*`, or `@import.path` capture is present.
   - Exposes the raw `tags.scm` bytes via `cache_fingerprint()` so `.rgctl/` incremental caches automatically invalidate whenever `tags.scm` changes.
2. **Pooled parsing (`parse_with_pool`)**:
   - Uses thread-local tree-sitter parser pooling (`rgctl_plugin_api::parse_with_pool`) so multi-file `rgctl discover` runs with zero per-file parser allocation overhead.
3. **Single-pass streaming query execution (`extract_with_query`)**:
   - Streams `QueryCursor::matches` over the parsed AST and populates `Symbol` and `Relation` records in two lightweight passes over the match stream.

### 2.1 Allowed `@capture` vocabulary

All `queries/tags.scm` files are gated by `ALLOWED_TAG_CAPTURES` in [`crates/rgctl-lang-runtime/src/queries.rs`](../crates/rgctl-lang-runtime/src/queries.rs) and `check_tags_scm` in `rgctl-ast-coverage`:

| Capture name | Role | Emitted graph artifact |
|--------------|------|------------------------|
| `@name` | Symbol / callee identifier | Node text becomes `Symbol.name` or target of `Relation` |
| `@doc` | Documentation comment | Stripped comment text stored in `Symbol.documentation` |
| `@definition.function` | Function / subroutine / procedure | `SymbolType::Function` (counts as caller scope + container) |
| `@definition.method` | Method / message handler | `SymbolType::Method` (counts as caller scope + container) |
| `@definition.class` | Class / struct / derived type / contract / model | `SymbolType::Class` (counts as container) |
| `@definition.interface` | Interface / protocol / trait / typeclass | `SymbolType::Interface` (counts as container) |
| `@definition.module` | Module / package / library unit | `SymbolType::Module` (counts as container) |
| `@definition.namespace` | Namespace / table | `SymbolType::Namespace` (counts as container) |
| `@definition.type` | Type alias / data type / newtype | `SymbolType::Type` (counts as container) |
| `@definition.enum` | Enum definition | `SymbolType::Enum` (counts as container) |
| `@definition.macro` | Macro / rule definition | `SymbolType::Macro` |
| `@definition.field` | Struct/class field or property | `SymbolType::Field` |
| `@definition.enummember` | Enum variant / constructor | `SymbolType::EnumVariant` |
| `@definition.constant` | Constant binding | `SymbolType::Constant` |
| `@definition.import` | Import declaration node | `SymbolType::Import` |
| `@import.path` | Imported module/path literal or identifier | `SymbolType::Import` (with normalized path & `metadata.import_path`) |
| `@reference.call` | Function / method / subroutine call site | `RelationType::Calls` (`from` = enclosing function or `<module>`, `to` = `@name`) |
| `@reference.class` | Type / class reference | `RelationType::References` |
| `@reference.implementation` | Trait / interface / behaviour implementation | `RelationType::Implements` |
| `@local.scope`, `@local.definition`, `@local.reference` | Lexical scope helpers | Ignored for graph emission; allowed for upstream `tags.scm` compatibility |

### 2.2 Header-to-body span promotion (`definition_span_node`)

In several grammars (such as Fortran), the identifier (`@name`) lives inside a header child statement rather than directly on the outer block node:

```
(function                          ; lines 4..10 (full function body)
  (function_statement              ; line 4 only (header statement)
    (name) @name)
  ...
  (end_function_statement))
```

When a pattern captures `(function_statement (name) @name) @definition.function`, `definition_span_node` in [`crates/rgctl-lang-runtime/src/queries.rs`](../crates/rgctl-lang-runtime/src/queries.rs) automatically checks whether the captured node is a `*_statement` / `*_header` child (`function_statement`, `subroutine_statement`, `module_statement`, `submodule_statement`, `program_statement`, `interface_statement`, `derived_type_statement`, `function_header`, `class_header`, `module_header`) and promotes the recorded symbol span to the parent block node (`function`, `subroutine`, `module`, `program`, etc.).

This guarantees that:
- `Symbol.location.start_line..=end_line` covers the **entire** function or module body.
- Line-span containment accurately nests functions inside modules (`math::add`) and attributes call expressions inside the body to the enclosing function (`add -> helper`).

### 2.3 Declaration-header call guard (`is_inside_declaration_header`)

In grammars like Julia (`function foo(x) ... end` where the signature `foo(x)` uses a `call_expression` node) or Lisp/Scheme/Racket (`(define (foo x) ...)` where the formals list `(foo x)` is a `list` node), a naive `@reference.call` pattern would match the function's own declaration signature as a self-call.

`is_inside_declaration_header` in [`crates/rgctl-lang-runtime/src/queries.rs`](../crates/rgctl-lang-runtime/src/queries.rs) inspects the parent chain of every `@reference.call` match and skips matches that serve as:
- The `signature` child of a `function_definition`, `short_function_definition`, or `macro_definition` (or inside a `typed_expression` / `where_clause` signature in Julia).
- The second child (formals list `(name args...)`) of a `(define ...)` or `(defn ...)` list in Scheme/Racket/Clojure/Elisp.

### 2.4 Span-containment qualification & caller attribution

After collecting all definitions and sorting them by `(start_line, end_line, name)`:
1. **Container qualification (`qualified_name`)**: For each symbol, `innermost_enclosing_container` uses `partition_point` over sorted container spans (`Class`, `Interface`, `Module`, `Namespace`, `Enum`, `Type`, `Function`, `Method`) to find the tightest strictly enclosing container and formats `Symbol.qualified_name` as `{container}::{name}`.
2. **Caller attribution (`Calls`)**: For each `@reference.call` match on `line`, `innermost_enclosing_function` uses `partition_point` over `Function` / `Method` spans (`start_line <= line <= end_line`) and picks the smallest-span enclosing function as `Relation.from` (falling back to `"<module>"` for top-level script calls).

---

## 3. Tier 2 language catalog (42 languages)

All Tier 2 languages registered in [`languages.toml`](../languages.toml) with `handler = "queries"` (37 prior + Apex, Pascal/Delphi, Perl, Ada, MATLAB):

| Language | ID | Crate | Grammar crate | Extensions | `function_kinds` | `class_kinds` |
|----------|----|-------|---------------|------------|------------------|---------------|
| **Ada** | `ada` | `rgctl-lang-ada` | `tree-sitter-ada` | `.adb`, `.ads`, `.ada` | `subprogram_declaration`, `subprogram_body` | `full_type_declaration`, `task_type_declaration` |
| **Apex (Salesforce)** | `apex` | `rgctl-lang-apex` | `tree-sitter-sfapex` | `.cls`, `.trigger`, `.apex` | `method_declaration`, `constructor_declaration` | `class_declaration`, `interface_declaration`, `trigger_declaration` |
| **Clojure** | `clojure` | `rgctl-lang-clojure` | `tree-sitter-clojure` | `.clj`, `.cljs`, `.cljc` | `list_lit` | — |
| **CMake** | `cmake` | `rgctl-lang-cmake` | `tree-sitter-cmake` | `.cmake` | `function_def` | — |
| **CSS** | `css` | `rgctl-lang-css` | `tree-sitter-css` | `.css` | `keyframes_statement` | `rule_set` |
| **CUDA** | `cuda` | `rgctl-lang-cuda` | `tree-sitter-cuda` | `.cu`, `.cuh` | `function_definition` | `struct_specifier`, `class_specifier` |
| **D** | `d` | `rgctl-lang-d` | `tree-sitter-d` | `.d` | `function_declaration` | `class_declaration`, `struct_declaration` |
| **Emacs Lisp** | `elisp` | `rgctl-lang-elisp` | `tree-sitter-elisp` | `.el` | `function_definition` | — |
| **Elm** | `elm` | `rgctl-lang-elm` | `tree-sitter-elm` | `.elm` | `value_declaration` | `type_declaration` |
| **Erlang** | `erlang` | `rgctl-lang-erlang` | `tree-sitter-erlang` | `.erl`, `.hrl` | `fun_decl`, `function_clause` | `record_decl` |
| **Fortran** | `fortran` | `rgctl-lang-fortran` | `tree-sitter-fortran` | `.f90`, `.f95`, `.f03`, `.f08`, `.F90` | `function`, `subroutine` | `derived_type_definition` |
| **F#** | `fsharp` | `rgctl-lang-fsharp` | `tree-sitter-fsharp` | `.fs`, `.fsi`, `.fsx` | `function_or_value_defn` | `type_definition` |
| **GDScript** | `gdscript` | `rgctl-lang-gdscript` | `tree-sitter-gdscript` | `.gd` | `function_definition` | `class_definition` |
| **Gleam** | `gleam` | `rgctl-lang-gleam` | `tree-sitter-gleam` | `.gleam` | `function` | `type_definition` |
| **GLSL** | `glsl` | `rgctl-lang-glsl` | `tree-sitter-glsl` | `.glsl`, `.vert`, `.frag`, `.comp` | `function_definition` | `struct_specifier` |
| **GraphQL** | `graphql` | `rgctl-lang-graphql` | `tree-sitter-graphql` | `.graphql`, `.gql` | `operation_definition`, `fragment_definition` | `object_type_definition` |
| **Haskell** | `haskell` | `rgctl-lang-haskell` | `tree-sitter-haskell` | `.hs` | `function` | `class`, `data_type`, `newtype` |
| **HTML** | `html` | `rgctl-lang-html` | `tree-sitter-html` | `.html`, `.htm` | `script_element` | `element` |
| **JSON** | `json` | `rgctl-lang-json` | `tree-sitter-json` | `.json` | — | `object` |
| **Julia** | `julia` | `rgctl-lang-julia` | `tree-sitter-julia` | `.jl` | `function_definition` | `struct_definition` |
| **Makefile** | `make` | `rgctl-lang-make` | `tree-sitter-make` | `.mk`, `.mak` | `rule` | — |
| **MATLAB** | `matlab` | `rgctl-lang-matlab` | `tree-sitter-matlab` | `.matlab`, `.m` | `function_definition` | `class_definition` |
| **Nix** | `nix` | `rgctl-lang-nix` | `tree-sitter-nix` | `.nix` | `function_expression` | — |
| **Objective-C** | `objc` | `rgctl-lang-objc` | `tree-sitter-objc` | `.m`, `.mm` | `function_definition`, `method_definition` | `class_interface`, `class_implementation` |
| **Pascal / Delphi** | `pascal` | `rgctl-lang-pascal` | `tree-sitter-pascal` | `.pas`, `.dpr`, `.dpk`, `.pp`, `.lpr` | `defProc`, `declProc` | `declClass`, `declIntf` |
| **Perl** | `perl` | `rgctl-lang-perl` | `ts-parser-perl` | `.pl`, `.pm`, `.t` | `subroutine_declaration_statement` | — |
| **OCaml** | `ocaml` | `rgctl-lang-ocaml` | `tree-sitter-ocaml` | `.ml`, `.mli` | `value_definition` | `class_definition` |
| **Odin** | `odin` | `rgctl-lang-odin` | `tree-sitter-odin` | `.odin` | `procedure_declaration` | `struct_declaration` |
| **PowerShell** | `powershell` | `rgctl-lang-powershell` | `tree-sitter-powershell` | `.ps1`, `.psm1` | `function_statement` | `class_statement` |
| **Prisma** | `prisma` | `rgctl-lang-prisma` | `tree-sitter-prisma` | `.prisma` | — | `model_declaration`, `enum_declaration` |
| **Protocol Buffers** | `proto` | `rgctl-lang-proto` | `tree-sitter-proto` | `.proto` | `rpc` | `message` |
| **R** | `r` | `rgctl-lang-r` | `tree-sitter-r` | `.r`, `.R` | `function_definition` | — |
| **Racket** | `racket` | `rgctl-lang-racket` | `tree-sitter-racket` | `.rkt` | `list` | — |
| **Scheme** | `scheme` | `rgctl-lang-scheme` | `tree-sitter-scheme` | `.scm`, `.ss` | `list` | — |
| **Solidity** | `solidity` | `rgctl-lang-solidity` | `tree-sitter-solidity` | `.sol` | `function_definition` | `contract_declaration` |
| **Starlark** | `starlark` | `rgctl-lang-starlark` | `tree-sitter-starlark` | `.star`, `.bzl` | `function_definition` | — |
| **Svelte** | `svelte` | `rgctl-lang-svelte` | `tree-sitter-svelte-ng` | `.svelte` | `script_element` | `element` |
| **TOML** | `toml` | `rgctl-lang-toml` | `tree-sitter-toml-ng` | `.toml` | — | `table` |
| **Verilog / SystemVerilog** | `verilog` | `rgctl-lang-verilog` | `tree-sitter-verilog` | `.v`, `.sv`, `.svh` | `function_declaration`, `task_declaration` | `class_declaration`, `module_declaration` |
| **VHDL** | `vhdl` | `rgctl-lang-vhdl` | `tree-sitter-vhdl` | `.vhd`, `.vhdl` | `subprogram_definition` | `component_declaration` |
| **YAML** | `yaml` | `rgctl-lang-yaml` | `tree-sitter-yaml` | `.yaml`, `.yml` | — | — |
| **Zig** | `zig` | `rgctl-lang-zig` | `tree-sitter-zig` | `.zig`, `.zon` | `function_declaration` | `struct_declaration` |

**`.m` disambiguation:** both Objective-C and MATLAB use `.m`. Discovery/extract peeks the first 4 KiB (`rgctl_registry::sniff_objc_vs_matlab`): MATLAB `function` / `classdef` / `%` headers route to `matlab`; `#import` / `@interface` / `@implementation` route to `objc` (default). Explicit `-l matlab` / `-l objc` still force the named plugin.

### Mainframe structural plugins (custom, not `tags.scm`)

COBOL (`cobol`), PL/I (`pli`), and JCL (`jcl`) are registered as **custom** structural extractors (no tree-sitter grammar) so batch `EXEC PGM`, `PERFORM`/`CALL`, `%INCLUDE`/`COPY`, and `DD DSN` dataset edges feed the same call graph / blast-radius / community pipeline. See Tier 1 notes in [tier-1-language-support.md](tier-1-language-support.md#mainframe-triad).

---

## 4. Step-by-step: add a new Tier 2 language

### Step 1 — Create `crates/rgctl-lang-{id}/`

Create the crate directory with the following structure:

```
crates/rgctl-lang-{id}/
  Cargo.toml
  queries/
    tags.scm
  src/
    config.rs
    lib.rs
```

1. **`Cargo.toml`**:

```toml
[package]
name = "rgctl-lang-{id}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "rgctl Tier-2 language plugin: {id}"

[dependencies]
rgctl-lang-runtime = { workspace = true }
rgctl-plugin-api = { workspace = true }
rgctl-registry = { workspace = true }
tree-sitter-{grammar} = "0.xx"

[dev-dependencies]
rgctl-ast-coverage = { workspace = true }
```

2. **`queries/tags.scm`**:
   Write S-expression patterns using captures from the [allowed vocabulary](#21-allowed-capture-vocabulary). Every definition or reference pattern must include an `@name` capture (except `@import.path`):

```scheme
; Tier-2 Fortran tags for rgctl (`handler = "queries"`).

(function_statement (name) @name) @definition.function

(subroutine_statement (name) @name) @definition.function

(module_statement (name) @name) @definition.module

(derived_type_definition
  (derived_type_statement
    (type_name) @name)) @definition.class

(call_expression (identifier) @name) @reference.call

(subroutine_call
  subroutine: (identifier) @name) @reference.call

(use_statement
  (module_name) @import.path)
```

3. **`src/config.rs`**:

```rust
use rgctl_lang_runtime::LanguageConfig;

pub const TAGS_SCM: &str = include_str!("../queries/tags.scm");

pub static CONFIG: LanguageConfig = LanguageConfig {
    id: "{id}",
    extensions: &["ext"],
    function_kinds: &["function_definition"],
    class_kinds: &["class_definition"],
    enable_complexity: false,
    enable_type_inference: false,
    regex_patterns: None,
    tags_scm: Some(TAGS_SCM),
};
```

4. **`src/lib.rs`** (including the vocabulary gate and extraction smoke test):

```rust
mod config;

use rgctl_lang_runtime::QueryLanguagePlugin;
use rgctl_registry::LanguageRegistry;
use std::sync::Arc;

pub use config::{CONFIG, TAGS_SCM};

pub fn register(registry: &mut LanguageRegistry) {
    let plugin = QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_{grammar}::LANGUAGE.into())
        .expect("compile {id} tags.scm");
    registry.register_language_plugin(Arc::new(plugin));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgctl_ast_coverage::check_tags_scm;
    use rgctl_plugin_api::LanguagePlugin;
    use std::path::Path;

    #[test]
    fn tags_scm_passes_ast_coverage_vocabulary_gate() {
        let issues = check_tags_scm("{id}", TAGS_SCM);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn extract_smoke() {
        let plugin =
            QueryLanguagePlugin::from_config(&CONFIG, || tree_sitter_{grammar}::LANGUAGE.into())
                .expect("compile");
        let source = br#"..."#;
        let result = plugin.extract_all(Path::new("test.ext"), source).expect("extract_all");
        assert!(!result.symbols.is_empty());
    }
}
```

### Step 2 — Register in `languages.toml` and workspace

1. Add `[languages.{id}]` under the Tier 2 section of [`languages.toml`](../languages.toml):

```toml
[languages.{id}]
handler = "queries"
plugin = "QueryLanguagePlugin"
module = "rgctl_lang_{id}"
crate = "tree-sitter-{grammar}"
extensions = ["ext"]
aliases = ["{id}"]
function_kinds = ["function_definition"]
class_kinds = ["class_definition"]
import_kinds = []
enable_complexity = false
enable_type_inference = false
tags_scm = "crates/rgctl-lang-{id}/queries/tags.scm"
```

2. Add `crates/rgctl-lang-{id}` to the workspace `Cargo.toml` and register `rgctl_lang_{id}::register(registry)` in [`crates/rgctl-languages/src/lib.rs`](../crates/rgctl-languages/src/lib.rs).
3. Run `cargo test -p rgctl-lang-{id} -p rgctl-languages` to verify grammar compilation, `check_tags_scm` vocabulary validation, and extraction smoke tests.
