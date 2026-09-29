# Puppet extraction honesty (Tier 1)

FQN conventions:

- Classes / defined types: Puppet name as declared (`profile::nginx`, `apache::vhost`)
- Resources: `{Type}[{title}]` (e.g. `Package[nginx]`)
- Nodes: `node:<name>` or `node:/regex/` for regex / default node names
- Functions: `{module}::{name}` when module path known; else `{file_stem}::{name}`
- Modules: `metadata.json` `name` field, or directory module name
- Type aliases: declared name (`Profile::Port`)

## Schema keep-list (pre-tree-sitter stubs retained)

| Kind | Symbol / Node | Notes |
|------|---------------|--------|
| Module | `PuppetModule` | From `metadata.json` / path |
| Class | `PuppetClass` | `class_definition` |
| Defined type | `PuppetDefinedType` | `defined_resource_type` |
| Resource | `PuppetResource` | `resource_declaration` |
| Variable | `PuppetVariable` | Decl / assignment sites |
| Fact | `PuppetFact` | `$facts[...]` best-effort |
| Node | `PuppetNode` | **New** — `node_definition` (not a module) |
| Function | `Function` | `function_declaration` |
| Type alias | `TypeAlias` | `type_declaration` |

**Edges retained:** `DependsOnModule`, `IncludesClass`, `InheritsClass`, `RequiresResource`, `UsesFact`. Do **not** reuse Ansible edges (`IncludesRole`, …) for Puppet `include`.

## Limits

- No Puppet catalog compiler, environment, or modulepath filesystem resolution beyond literal names / adjacent `metadata.json`
- No ERB→Jinja2, Hiera→vars, or Facter fact-mapping translation (graph coverage of `.pp` only)
- Collectors / exported resources may be unresolved (`metadata.unresolved`)
- Layer F: parameters → `fields[]`; **no language constructors** (C-like; no `.<init>` required)
- **F6 waiver:** Puppet has no OOP field-write mutation shape comparable to Java `obj.field =`; golden `cpg mutations` is deferred. F1 (parameter fields) and F3 (typed params) are enforced in plugin unit tests.
- Ruby plugin indexes `.rb` only — does not substitute for Puppet DSL

See also: [languages/puppet.md](languages/puppet.md) · [tier-1-language-support.md](tier-1-language-support.md) · OpenSpec `add-puppet-tier1-language-support`.
