# Puppet

Tier 1 plugin for Puppet DSL manifests (`.pp`). Extracts classes, defined types, resources, nodes, functions, type aliases, module metadata deps, and typed Puppet edges.

## Implementation

| | |
|---|---|
| **Plugin crate** | `crates/rgctl-lang-puppet` (`PuppetPlugin`) |
| **Grammar** | `tree-sitter-puppet` **1.3.0** |
| **Extensions** | `.pp` |
| **Discover** | `rgctl discover . -l puppet --with-cfg` |
| **CFG / taint** | Enabled (`LanguageAnalysisProfile`) |

AST coverage: `crates/rgctl-lang-puppet/puppet-ast-coverage.json` (CI: `puppet_ast_coverage_manifest_matches_grammar`).

## What is extracted

### Nodes

- `PuppetClass` / `PuppetDefinedType` / `PuppetResource` / `PuppetVariable` / `PuppetFact` / `PuppetModule` / `PuppetNode`
- `Function` — Puppet 4+ `function_declaration`
- `TypeAlias` — `type_declaration`

### Edges

| Edge | Meaning |
|------|---------|
| `IncludesClass` | `include` |
| `InheritsClass` | `inherits` |
| `RequiresResource` | `->` / `~>` / `require` |
| `DependsOnModule` | `metadata.json` dependencies |
| `UsesFact` | `$facts[...]` |
| `Calls` | `function_call` (often unresolved) |

## Honesty limits

See [puppet-extract-honesty.md](../puppet-extract-honesty.md). No catalog compiler, ERB/Hiera translation, or Ansible edge reuse. Layer F: parameters as `fields[]`; no constructors.

## Verification

```bash
cargo test -p rgctl-lang-puppet --lib
rgctl discover rgctl-tests/ecommerce-puppet -l puppet --with-cfg -v
```
