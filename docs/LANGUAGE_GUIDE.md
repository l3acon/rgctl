# Language guide

> **Moved.** Language support pages are generated on the website from
> `crates/rgctl-lang-*/{id}-ast-coverage.json` (see [languages/README.md](languages/README.md)).

**Tier-1 deep-analysis** languages ship grammar-verified `*-ast-coverage.json` + single-pass `extract_all` (`ScopeStack` + `KindDispatchTable` + `parse_with_pool`). That is distinct from the ~60 language/config plugins in [`languages.toml`](../languages.toml). Drift gates: `RGCTL_AST_COVERAGE_STRICT=1` (build), `RGCTL_AST_COVERAGE_TRACK=1` (runtime hit tracking in depth tests).

Contributor checklist: [tier-1-language-support.md](tier-1-language-support.md) · [language-testing-guide.md](language-testing-guide.md) · [contributor-checklist.md](contributor-checklist.md).
