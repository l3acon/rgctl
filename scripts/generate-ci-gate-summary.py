#!/usr/bin/env python3
"""Emit rgctl supremacy CI gate dashboard to $GITHUB_STEP_SUMMARY + JSON artifact.

Reads optional JSON lines from `.rgctl-ci-gate-status.jsonl` (one object per gate:
`{"gate": "...", "status": "PASS"|"FAIL"|"SKIP", "detail": "..."}`).
Missing entries default to SKIP so the summary still renders when steps are skipped.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

GATES = [
    (
        "AST Coverage (STRICT + TRACK)",
        "100% grammar mapped, >0 runtime hits",
        "ast_coverage",
    ),
    (
        "N-Way Graph Determinism (graph_determinism_qe)",
        "Bit-identical digest across 1 vs 8 threads",
        "graph_determinism_qe",
    ),
    (
        "Zero Cold-Touch (columnar_zero_cold_touch_gate)",
        "cold_ext_deserialization_count == 0",
        "columnar_zero_cold_touch_gate",
    ),
    (
        "Cross-Language MRO, Overrides & DI (8.4 / 9.3)",
        "C3, Mixins, Inherent, Routes & DI",
        "cross_language_mro_and_di",
    ),
    (
        "CFG/PDG v3 Lazy Slicing & Shadowing (9.4)",
        "Single-fn TOC append & scope-ID isolation",
        "slicing",
    ),
    (
        "Fast-Leiden & Centrality Role Audit (9.5)",
        "0 disconnected communities, < 10ms PageRank",
        "community_audit",
    ),
    (
        "25K/80K Scale Perf Gate (scale_perf_ci_gate)",
        "< 15.0s full columnar + diff + compact + clones",
        "scale_perf_ci_gate",
    ),
    (
        "Fast-Leiden 150K & Centrality Scale",
        "community_audit + centrality_audit + centrality_approx_scale",
        "centrality_scale",
    ),
    (
        "Benchmark Compilation Smoke",
        "cargo bench --no-run",
        "bench_smoke",
    ),
]


def load_status(path: Path) -> dict[str, dict]:
    out: dict[str, dict] = {}
    if not path.is_file():
        return out
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        gate = obj.get("gate")
        if isinstance(gate, str):
            out[gate] = obj
    return out


def main() -> int:
    status_path = Path(os.environ.get("RGCTL_CI_GATE_STATUS", ".rgctl-ci-gate-status.jsonl"))
    report_path = Path(os.environ.get("RGCTL_CI_GATE_REPORT", ".rgctl-ci-gate-report.json"))
    statuses = load_status(status_path)

    rows = []
    for title, budget, key in GATES:
        st = statuses.get(key, {})
        status = str(st.get("status", "SKIP")).upper()
        detail = st.get("detail") or budget
        rows.append(
            {
                "gate": title,
                "key": key,
                "budget": budget,
                "status": status,
                "detail": detail,
                "elapsed_secs": st.get("elapsed_secs"),
            }
        )

    report = {
        "schema_version": 1,
        "gates": rows,
    }
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    lines = [
        "### rgctl All-Dimension Supremacy CI Gate Summary",
        "",
        "| Gate | Budget / Invariant | Status |",
        "|------|--------------------|--------|",
    ]
    for row in rows:
        lines.append(f"| {row['gate']} | {row['budget']} | {row['status']} |")
    lines.append("")
    text = "\n".join(lines)
    if summary_path:
        with open(summary_path, "a", encoding="utf-8") as fh:
            fh.write(text)
            fh.write("\n")
    else:
        sys.stdout.write(text)
        sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
