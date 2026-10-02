# Vuln / deps workflow

**When:** OSV / CVE impact questions — “are we affected?”, “is this library in our tree?”, multi-ecosystem supply-chain triage.

**Pipeline (P0–P1):** triage → deps check (fast exit). Reachability / OpenVEX come later (`add-vuln-reachability-vex`).

| Intent | Command |
|--------|---------|
| Normalize OSV | `rgctl -f json vuln triage --osv ./advisory.json` |
| Match deps (manifests) | `rgctl -r "$REPO" -f json deps check --osv ./advisory.json` |
| Include bundled JARs | `… deps check --osv ./advisory.json --include-jars lib` |
| Include node_modules | `… deps check --osv ./advisory.json --include-node-modules .` |

**Verdicts:** `not_affected` | `affected_candidate`. Candidate ≠ exploitable — still need callers / taint / VEX.

**Honesty:** OSV `versions[]` may be a backport series (`versions_array_not_authoritative`). Vendor Maven suffixes (e.g. `-rhlw-NNNN`) use the Maven version engine. Bundled scans are **opt-in** (never part of default `discover`).

**Multi-language:** Maven + Cargo + npm (+ Go) version engines; manifests via existing extractors; JAR embedded poms + `node_modules` adapters.
