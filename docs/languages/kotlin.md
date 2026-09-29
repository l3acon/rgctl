# Kotlin language support

Tier 1 custom plugin (`rgctl-lang-kotlin`) using **`tree-sitter-kotlin-ng` 1.1.0**.

See honesty limits: [kotlin-extract-honesty.md](../kotlin-extract-honesty.md).

## Discover

```bash
rgctl discover . -l kotlin --with-cfg
```

Extensions: `.kt`, `.kts` — except basename `build.gradle.kts` (Manifest / Dependency extractors).

## Tests

| Kind | Command / path |
|------|----------------|
| Unit | `cargo test -p rgctl-lang-kotlin` |
| Langfeatures | `cargo test --release --test kotlin_langfeatures` |
| CFG / taint | `cargo test --release --test kotlin_cfg_analysis` / `kotlin_taint` |
| Layer F | `rgctl-analysis` `kotlin_cfg_captures_field_write_and_query` |
| Fixture | `rgctl-tests/ecommerce-kotlin` |
| Dashboard | `cargo test --release --test dashboard_ecommerce_kotlin` |
| Smoke script | `rgctl-tests/gql-verification-smoke/verify-extraction-gql-kotlin.sh` |
| Gate B | `kotlin_cold_discover_within_baseline` (ignored; set `RGCTL_KOTLIN_REPO`) |

```bash
RGCTL=target/release/rgctl ./rgctl-tests/gql-verification-smoke/verify-extraction-gql-kotlin.sh
```
