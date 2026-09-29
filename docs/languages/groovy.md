# Groovy language support

Tier 1 custom plugin (`rgctl-lang-groovy`) using **`tree-sitter-groovy` 0.1.2**.

See honesty limits: [groovy-extract-honesty.md](../groovy-extract-honesty.md).

## Discover

```bash
rgctl discover . -l groovy --with-cfg
```

Extensions: `.groovy`, `.gradle` — except basename `build.gradle` (Manifest).

## Tests

| Kind | Command / path |
|------|----------------|
| Unit | `cargo test -p rgctl-lang-groovy` |
| Langfeatures | `cargo test --release --test groovy_langfeatures` |
| CFG / taint | `cargo test --release --test groovy_cfg_analysis` / `groovy_taint` |
| Layer F | `rgctl-analysis` `groovy_cfg_captures_field_write_and_query` |
| Fixture | `rgctl-tests/ecommerce-groovy` |
| Dashboard | `cargo test --release --test dashboard_ecommerce_groovy` |
| Smoke script | `rgctl-tests/gql-verification-smoke/verify-extraction-gql-groovy.sh` |
| Gate B | `groovy_cold_discover_within_baseline` (ignored; set `RGCTL_GROOVY_REPO`) |

```bash
RGCTL=target/release/rgctl ./rgctl-tests/gql-verification-smoke/verify-extraction-gql-groovy.sh
```
