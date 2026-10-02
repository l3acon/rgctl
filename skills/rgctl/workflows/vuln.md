# Vuln / deps / reachability workflow

**When:** OSV / CVE impact — “are we affected?”, “is it reachable?”, OpenVEX.

**Prerequisites for sink-first / blast classify:** index with `rgctl discover --with-cfg` (add `--with-taint` only if you need discover-time flows). Taint remains **opt-in**.

**Pipeline:**

| Step | Command |
|------|---------|
| P0 Normalize OSV | `rgctl -f json vuln triage --osv ./advisory.json` |
| P1 Deps match | `rgctl -r "$REPO" -f json deps check --osv ./advisory.json` (+ `--include-jars lib`) |
| P2 Package imports | `rgctl -r "$REPO" -f json find --package '<coords>' --type import` |
| P3 Facade callers | `rgctl -r "$REPO" -f json callers <Symbol> --package '<coords>' --methods readValue,…` |
| P4 Boundary blast | `rgctl -r "$REPO" -f json blast-radius <Symbol> --classify-boundary` |
| P5 Sink-first taint | `rgctl -r "$REPO" -f json taint --sink ObjectMapper.readValue --source external` |
| P6 Orchestrated VEX | `rgctl -r "$REPO" -f json vuln analyze --osv ./advisory.json --include-jars lib` |

**Verdicts:** deps `not_affected` | `affected_candidate`. Analyze `exploitability`: `not_affected` | `not_exploitable` | `exploitable` | `under_investigation`. OpenVEX statuses map accordingly; unresolved sinks alone MUST NOT force `not_affected` without caller evidence.

**Honesty:** OSV `versions[]` may be backport series; short Maven groups use the resolver table (no silent wrong guess). Bundled JAR / `node_modules` scans are **opt-in**. Zero imports ≠ library absent when `bundled_presence` / deps match.

**Multi-language:** same CLI; Maven/npm/Cargo/Go/PyPI/(NuGet/Ruby/Composer stubs) resolver; Java/Jakarta + Python web boundary catalogs; declarative taint packs (`TaintRuleSet` overlays for OSV methods — no hardcoded `detect_*`).
