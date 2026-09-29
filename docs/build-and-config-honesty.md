# Build manifests & configuration graph — honesty notes

OpenSpec change: [`openspec/changes/archive/2026-09-29-add-build-and-config-graph/`](../openspec/changes/archive/2026-09-29-add-build-and-config-graph/).

## Ingest routing

| Route | Examples | Emit |
|-------|----------|------|
| **Manifest** | `pom.xml`, `Cargo.toml`, `package.json`, `go.mod`, `build.gradle(.kts)` | `Dependency` + `DependsOn` (not flat ConfigKeys for deps) |
| **Config** | `*.properties`, `*.yml`/`*.yaml`, `*.toml`, `*.json`, allowlisted `*.xml` | `ConfigKey` with spans |
| **Workflow** | `.github/workflows/*.yml` | YAML ConfigKeys today (Job/BuildStep deferred) |
| **Ignore** | lockfiles, unknown XML | skipped |

Non-POM XML is **allowlisted** (`web.xml`, `persistence.xml`, `config.xml`, … or under `src/main/resources/`). Random docs XML stays Ignore to protect Gate A node counts.

## Declared vs transitive

v1 extracts **declared** dependencies only. Lockfiles (`Cargo.lock`, `go.sum`, `package-lock.json`, …) are Ignore. No Maven reactor / Gradle resolution.

## Gradle

Static regex for `implementation` / `api` / `testImplementation`-style string coordinates. Dynamic/`project(...)`/version catalogs → incomplete; do not claim full Gradle fidelity.

## Spring / Quarkus config linking

- `@Value("${key}")` / `@Value("${key:default}")` → key without default
- `@ConfigProperty(name = "...")`
- Matching is exact ConfigKey path, then normalized (`-`/`_` → `.`, lowercased)
- **No stub ConfigKeys** for missing keys in v1

## Kantra providers

`java.dependency` / `go.dependency` match `Dependency` nodes by coordinate name (version bounds best-effort / often N/A when only G:A stored).  
`builtin.xml` / `builtin.json` reparse files on demand (minimal XPath/`$.a.b` subset) — **no DOM in `.rgctl/`**.

## Query surface

Prefer GQL:

```text
nodes(Dependency) { name }
nodes(ConfigKey) { name }
```

Optional CLI `rgctl dependencies list` / `rgctl config list` deferred; use GQL until shipped.

## Cold discover deltas (`rgctl-tests/ecommerce-java`)

Measured after `rm -rf .rgctl` + release `rgctl discover .` (fixture includes `kantra_cache/` + correctness JSON):

| Metric | Count | Notes |
|--------|------:|-------|
| Total nodes | 1346 | full fixture tree |
| `Dependency` | 18 | 9 Maven coords as `maven:G:A` + 9 bare `G:A` external stubs from other DependsOn resolvers (same artifacts) |
| `ConfigKey` (all) | 335 | inflated by `kantra_cache/*.json` / correctness fixtures |
| `ConfigKey` in `application.properties` | 13 | intended app config surface |
| `UsesConfig` | 2 | `JwtTokenProvider.<init>` → `app.jwt.secret`, `app.jwt.expiration-ms` |

Gate A (linux cold, ref M3 Pro): **wall=146.9s**, nodes=2_701_573 — within **145s +10%**.

## Planning

Track progress only in OpenSpec `tasks.md`. Do **not** update `.github/TASK_PLAN.md`.
