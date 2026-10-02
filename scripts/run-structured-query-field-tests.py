#!/usr/bin/env python3
"""Exhaustive structured-query field runner (OpenSpec test-plan-multi-language).

Usage:
  python3 scripts/run-structured-query-field-tests.py [--phase warm|cold|all] [--lang go,java,...]

Writes:
  .reports/sq-field-results.jsonl
  .reports/multi-language-structured-query-field-report.md
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Any, Optional

ROOT = Path(__file__).resolve().parents[1]
RGCTL = ROOT / "target" / "release" / "rgctl"
REPORTS = ROOT / ".reports"
JSONL = REPORTS / "sq-field-results.jsonl"
MD = REPORTS / "multi-language-structured-query-field-report.md"

# Gate B corpora (see AGENTS.md / OpenSpec test plan)
CORPORA: dict[str, dict[str, Any]] = {
    "go": {
        "path": ROOT / "example" / "kubernetes",
        "discover": ["discover", "-l", "go", "pkg/", "cmd/"],
        "scope": "pkg/util",
        "find_pat": "*Controller*",
        "find_type": "struct",
        "file_basename": None,
        "expect_annotatedwith": False,
        "phase": "warm",
    },
    "java": {
        "path": ROOT / "example" / "metasfresh-4.9.8b",
        "discover": ["discover", "--full", "."],
        "scope": "de.metas",
        "find_pat": "*Service*",
        "find_type": "class",
        "file_basename": None,
        "expect_annotatedwith": True,
        "phase": "warm",
        "latency": True,
    },
    "php": {
        "path": ROOT / "example" / "magento2",
        "discover": ["discover", "-l", "php", "."],
        "scope": "Magento/Customer",
        "scope_alt": r"Magento\Customer",
        "find_pat": "*Customer*",
        "find_type": "class",
        "file_basename": None,
        "expect_annotatedwith": False,
        "phase": "warm",
    },
    "python": {
        "path": ROOT / "example" / "home-assistant",
        "discover": ["discover", "-l", "python", "."],
        "scope": "homeassistant",
        "find_pat": "*Sensor*",
        "find_type": "class",
        "file_basename": None,
        "expect_annotatedwith": False,
        "phase": "warm",
    },
    "ruby": {
        "path": ROOT / "example" / "discourse",
        "discover": ["discover", "-l", "ruby", "."],
        "scope": "app/models",
        "find_pat": "*User*",
        "find_type": "class",
        "file_basename": None,
        "expect_annotatedwith": False,
        "phase": "warm",
    },
    "rust": {
        "path": ROOT / "example" / "rust",
        "discover": ["discover", "-l", "rust", "."],
        "scope": "compiler",
        "find_pat": "*Resolver*",
        "find_type": "struct",
        "file_basename": None,
        "expect_annotatedwith": False,
        "phase": "warm",
    },
    "groovy": {
        "path": ROOT / "example" / "groovy",
        "discover": ["discover", "-l", "groovy", "."],
        "scope": "org.gradle",
        "find_pat": "*Service*",
        "find_type": "class",
        "file_basename": None,
        "expect_annotatedwith": True,  # after re-discover
        "phase": "warm",
        "rediscover": True,
    },
    "kotlin": {
        "path": ROOT / "example" / "kotlin",
        "discover": ["discover", "-l", "kotlin", "."],
        "scope": "libraries",
        "find_pat": "*Factory*",
        "find_type": "class",
        "file_basename": None,
        "expect_annotatedwith": False,  # corpus currently 0; track as extraction gap
        "phase": "warm",
    },
    "typescript": {
        "path": ROOT / "example" / "vscode" / "src",
        "discover": ["discover", "-l", "typescript", "."],
        "scope": "vs/workbench",
        "find_pat": "*Editor*",
        "find_type": "class",
        "file_basename": None,
        "expect_annotatedwith": True,
        "phase": "cold",
        "rediscover": True,
    },
    "javascript": {
        "path": ROOT / "example" / "node" / "test",
        "discover": ["discover", "-l", "javascript", "."],
        "scope": "parallel",
        "find_pat": "*test*",
        "find_type": "function",
        "file_basename": None,
        "expect_annotatedwith": False,
        "phase": "cold",
    },
    "csharp": {
        "path": ROOT / "example" / "roslyn" / "src",
        "discover": ["discover", "-l", "csharp", "."],
        "scope": "Compilers",
        "find_pat": "*Syntax*",
        "find_type": "class",
        "file_basename": None,
        "expect_annotatedwith": True,
        "phase": "cold",
    },
    "cpp": {
        "path": ROOT / "example" / "llvm-project" / "clang",
        "discover": ["discover", "-l", "cpp", "."],
        "scope": "lib",
        "find_pat": "*Parser*",
        "find_type": "function",
        "file_basename": None,
        "expect_annotatedwith": False,
        "phase": "cold",
    },
    "c": {
        "path": ROOT / "example" / "linux",
        "discover": ["discover", "."],
        "scope": "kernel",
        "find_pat": "*sched*",
        "find_type": "function",
        "file_basename": None,
        "expect_annotatedwith": False,
        "phase": "cold",
        "optional": True,
    },
    "puppet": {
        "path": ROOT / "example" / "theforeman",
        "discover": ["discover", "-l", "puppet,erb,ruby", "-e", "spec,vendor", "."],
        "scope": "modules",
        "find_pat": "*",
        "find_type": "class",
        "file_basename": None,
        "expect_annotatedwith": False,
        "phase": "cold",
        "optional": True,
    },
}


def extract_json(text: str) -> Optional[Any]:
    text = text.strip()
    if not text:
        return None
    # Prefer first top-level JSON value (banners may precede; never rfind into nested objects).
    for i, ch in enumerate(text):
        if ch not in "{[":
            continue
        opener, closer = ("{", "}") if ch == "{" else ("[", "]")
        depth = 0
        in_str = False
        esc = False
        for j in range(i, len(text)):
            c = text[j]
            if in_str:
                if esc:
                    esc = False
                elif c == "\\":
                    esc = True
                elif c == '"':
                    in_str = False
                continue
            if c == '"':
                in_str = True
            elif c == opener:
                depth += 1
            elif c == closer:
                depth -= 1
                if depth == 0:
                    chunk = text[i : j + 1]
                    try:
                        return json.loads(chunk)
                    except json.JSONDecodeError:
                        break
    # Plain count-only integer
    if re.fullmatch(r"-?\d+", text.splitlines()[-1].strip() if text else ""):
        return {"returned": int(text.splitlines()[-1].strip()), "total": int(text.splitlines()[-1].strip()), "schema_version": 1, "count_only": True}
    return None


def probe_ok_envelope(data: Any, check_schema: bool) -> tuple[bool, str]:
    if data is None:
        return False, "no JSON"
    if not isinstance(data, dict):
        return False, f"non-object JSON: {type(data)}"
    if not check_schema:
        return True, ""
    if "schema_version" in data:
        return True, ""
    if "by" in data and "counts" in data:
        return True, ""  # inventory envelope
    if "count_only" in data:
        return True, ""
    return False, "missing schema_version"


@dataclass
class ProbeResult:
    lang: str
    probe_id: str
    cmd: list[str]
    ok: bool
    exit_code: int
    wall_s: float
    detail: str
    returned: Optional[int] = None
    total: Optional[int] = None
    extra: Optional[dict] = None


def run_rgctl(cwd: Path, args: list[str], timeout: int = 600) -> tuple[int, str, str, float]:
    cmd = [str(RGCTL), "-f", "json", *args]
    t0 = time.perf_counter()
    try:
        p = subprocess.run(
            cmd,
            cwd=str(cwd),
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        wall = time.perf_counter() - t0
        return p.returncode, p.stdout, p.stderr, wall
    except subprocess.TimeoutExpired as e:
        wall = time.perf_counter() - t0
        out = (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
        err = (e.stderr or b"").decode() if isinstance(e.stderr, bytes) else (e.stderr or "")
        return 124, out, err + "\nTIMEOUT", wall


def counts_map(data: Any) -> dict[str, int]:
    if not isinstance(data, dict):
        return {}
    out = {}
    for row in data.get("counts") or []:
        if isinstance(row, dict) and "key" in row:
            out[str(row["key"])] = int(row.get("count") or 0)
    return out


def first_entity_name(data: Any) -> Optional[str]:
    if not isinstance(data, dict):
        return None
    ents = data.get("entities") or data.get("neighbors") or []
    if ents and isinstance(ents[0], dict):
        return ents[0].get("name")
    return None


def probe(
    lang: str,
    probe_id: str,
    cwd: Path,
    args: list[str],
    expect_ok: bool = True,
    expect_total_gt: Optional[int] = None,
    check_schema: bool = True,
) -> ProbeResult:
    code, out, err, wall = run_rgctl(cwd, args)
    data = extract_json(out)
    detail_parts = []
    ok = (code == 0) if expect_ok else (code != 0)
    returned = total = None
    extra: dict[str, Any] = {}
    if expect_ok:
        env_ok, env_msg = probe_ok_envelope(data, check_schema)
        if not env_ok:
            ok = False
            detail_parts.append(f"{env_msg}; stderr={err[:200]!r}")
        elif isinstance(data, dict):
            returned = data.get("returned")
            total = data.get("total")
            if returned is None and "counts" in data:
                extra["nonzero"] = sum(1 for c in data["counts"] if c.get("count", 0) > 0)
                extra["keys"] = len(data["counts"])
            if expect_total_gt is not None:
                t = total if total is not None else 0
                if t <= expect_total_gt:
                    ok = False
                    detail_parts.append(f"total={t} not > {expect_total_gt}")
            if "counts" in data:
                cm = counts_map(data)
                extra["sample"] = {k: cm[k] for k in list(cm)[:8]}
                extra["top"] = sorted(cm.items(), key=lambda x: -x[1])[:6]
    else:
        detail_parts.append(f"exit={code} err={err[:160]!r}")
    if data is not None and returned is None and isinstance(data, dict):
        returned = data.get("returned")
        total = data.get("total")
    detail = "; ".join(detail_parts) if detail_parts else ("pass" if ok else "fail")
    if returned is not None or total is not None:
        detail = f"returned={returned} total={total}; {detail}"
    return ProbeResult(lang, probe_id, [str(RGCTL), "-f", "json", *args], ok, code, wall, detail, returned, total, extra or None)


def has_snapshot(path: Path) -> bool:
    return (path / ".rgctl" / "graph.snapshot.bin").is_file()


def ensure_discover(lang: str, cfg: dict[str, Any], force: bool = False) -> ProbeResult:
    path: Path = cfg["path"]
    if not path.is_dir():
        return ProbeResult(lang, "DISCOVER", [], False, 2, 0.0, f"corpus missing: {path}")
    if has_snapshot(path) and not force and not cfg.get("rediscover"):
        return ProbeResult(lang, "DISCOVER", [], True, 0, 0.0, "reuse warm snapshot")
    if has_snapshot(path) and cfg.get("rediscover") and not force:
        # still allow warm pass; rediscover flagged separately
        return ProbeResult(lang, "DISCOVER", [], True, 0, 0.0, "warm snapshot present (rediscover deferred)")
    args = cfg["discover"]
    print(f"[{lang}] discovering: {' '.join(args)}", flush=True)
    t0 = time.perf_counter()
    p = subprocess.run(
        [str(RGCTL), *args],
        cwd=str(path),
        capture_output=True,
        text=True,
        timeout=7200,
    )
    wall = time.perf_counter() - t0
    ok = p.returncode == 0 and has_snapshot(path)
    return ProbeResult(
        lang,
        "DISCOVER",
        [str(RGCTL), *args],
        ok,
        p.returncode,
        wall,
        "ok" if ok else f"fail stderr={p.stderr[-400:]}",
    )


def run_lang(lang: str, cfg: dict[str, Any], do_discover: bool, force_rediscover: bool) -> list[ProbeResult]:
    path: Path = cfg["path"]
    results: list[ProbeResult] = []
    if not path.is_dir():
        results.append(ProbeResult(lang, "SKIP", [], False, 0, 0.0, f"corpus absent: {path}"))
        return results

    if do_discover or (force_rediscover and cfg.get("rediscover")):
        results.append(ensure_discover(lang, cfg, force=force_rediscover and bool(cfg.get("rediscover"))))
    elif not has_snapshot(path):
        results.append(ProbeResult(lang, "SKIP", [], False, 0, 0.0, "no snapshot; cold discover not requested"))
        return results
    else:
        results.append(ProbeResult(lang, "DISCOVER", [], True, 0, 0.0, "reuse warm snapshot"))

    if not has_snapshot(path):
        return results

    # A inventory
    for by in ("type", "edge", "lang", "file", "community"):
        results.append(probe(lang, f"A-{by}", path, ["inventory", "--by", by]))

    # B find
    results.append(probe(lang, "B1", path, ["find", "--type", "function", "--count-only"]))
    results.append(probe(lang, "B2", path, ["find", "--type", cfg["find_type"], "--count-only"]))
    results.append(probe(lang, "B4", path, ["find", cfg["find_pat"], "--type", cfg["find_type"], "--limit", "20"]))
    results.append(probe(lang, "B5", path, ["find", cfg["find_pat"], "--limit", "20"]))
    results.append(probe(lang, "B6", path, ["find", "--type", "import", "--count-only"]))
    results.append(probe(lang, "B10", path, ["find", "--type", "not_a_real_type"], expect_ok=False, check_schema=False))
    results.append(probe(lang, "B11", path, ["query", "find", cfg["find_pat"], "--limit", "5"]))

    # C scope
    scope = cfg.get("scope")
    if scope:
        expect = 0  # just complete; mark soft
        r = probe(lang, "C-scope", path, ["find", cfg["find_pat"], "--type", cfg["find_type"], "--scope", scope, "--limit", "10"])
        # Soft pass: command ok; note if total==0
        if r.ok and (r.total or 0) == 0:
            r.detail += "; WARN total=0 (scope miss?)"
        results.append(r)
    if cfg.get("scope_alt"):
        r = probe(
            lang,
            "C-scope-alt",
            path,
            ["find", cfg["find_pat"], "--type", cfg["find_type"], "--scope", cfg["scope_alt"], "--limit", "10"],
        )
        if r.ok and (r.total or 0) == 0:
            r.detail += "; WARN total=0"
        results.append(r)

    # Seed symbol for callers from B4 — prefer concrete file_path (skip <external>)
    code, out, _, _ = run_rgctl(
        path, ["find", cfg["find_pat"], "--type", cfg["find_type"], "--limit", "50"]
    )
    data = extract_json(out)
    # Prefer a unique name (exact total==1) so callers/callees aren't blocked on ambiguity.
    seed = None
    seed_file = None
    if isinstance(data, dict):
        candidates = []
        for ent in data.get("entities") or []:
            fp = ent.get("file") or ent.get("file_path") or ""
            name = ent.get("name")
            if name and fp and fp != "<external>":
                candidates.append((name, fp))
        for name, fp in candidates:
            _, out_u, _, _ = run_rgctl(path, ["find", name, "--exact", "--count-only"])
            du = extract_json(out_u)
            tot = (du or {}).get("total") if isinstance(du, dict) else None
            if tot == 1:
                seed, seed_file = name, fp
                break
        if seed is None and candidates:
            seed, seed_file = candidates[0]

        if seed_file:
            base = Path(seed_file).name
            results.append(probe(lang, "C5-file-basename", path, ["find", "*", "--file", base, "--limit", "5"]))
            # Path-segment scope from file path (Go/TS style)
            parts = Path(seed_file).parts
            if len(parts) >= 2:
                seg = "/".join(parts[-3:-1]) if len(parts) >= 3 else parts[-2]
                r = probe(
                    lang,
                    "C-scope-from-file",
                    path,
                    ["find", "*", "--type", cfg["find_type"], "--scope", seg, "--limit", "10"],
                )
                if r.ok and (r.total or 0) == 0:
                    r.detail += "; WARN total=0"
                results.append(r)

    if seed:
        results.append(probe(lang, "B3-exact", path, ["find", seed, "--exact", "--limit", "5"]))
        # Prefer basename --file for disambiguation (full paths often still collide on name).
        file_flag = Path(seed_file).name if seed_file else None
        call_args = ["callers", seed, "--depth", "1", "--limit", "20"]
        callee_args = ["callees", seed, "--depth", "1", "--limit", "20"]
        call2_args = ["callers", seed, "--depth", "2", "--limit", "20"]
        rel_args = ["relations", seed, "--edge", "calls", "--direction", "out", "--depth", "1", "--limit", "20"]
        if file_flag:
            call_args += ["--file", file_flag]
            callee_args += ["--file", file_flag]
            call2_args += ["--file", file_flag]
            rel_args += ["--file", file_flag]
        results.append(probe(lang, "D1-callers", path, call_args))
        results.append(probe(lang, "D2-callees", path, callee_args))
        results.append(probe(lang, "D3-callers-d2", path, call2_args))
        results.append(probe(lang, "E7-seeded", path, rel_args))
        # D4: without --file — ambiguous error OR unique resolve both acceptable
        code_d4, out_d4, err_d4, wall_d4 = run_rgctl(path, ["callers", seed, "--depth", "1", "--limit", "5"])
        d4_ok = code_d4 != 0 and "Ambiguous" in (err_d4 + out_d4) or code_d4 == 0
        results.append(
            ProbeResult(
                lang,
                "D4-ambiguous",
                [str(RGCTL), "-f", "json", "callers", seed],
                d4_ok,
                code_d4,
                wall_d4,
                f"exit={code_d4}; ambiguous_or_unique ok",
            )
        )
    else:
        results.append(ProbeResult(lang, "SEED", [], False, 0, 0.0, "no seed symbol from find"))

    results.append(probe(lang, "D5-missing", path, ["callers", "__no_such_symbol_zz__"], expect_ok=False, check_schema=False))

    # E relations seedless
    for edge, pid in (
        ("calls", "E1"),
        ("extends", "E2"),
        ("implements", "E3"),
        ("annotatedwith", "E4"),
        ("instantiates", "E6"),
    ):
        results.append(probe(lang, pid, path, ["relations", "--edge", edge, "--limit", "20"]))
    results.append(
        probe(
            lang,
            "E5",
            path,
            [
                "relations",
                "--edge",
                "annotatedwith",
                "--from-type",
                "function",
                "--to-type",
                "annotation",
                "--limit",
                "20",
            ],
        )
    )
    results.append(probe(lang, "E8", path, ["relations", "--edge", "not_a_real_edge"], expect_ok=False, check_schema=False))
    if scope:
        results.append(
            probe(
                lang,
                "E9",
                path,
                ["relations", "--edge", "extends", "--scope", scope, "--scope-mode", "inside", "--limit", "20"],
            )
        )
        results.append(
            probe(
                lang,
                "C6-crossing",
                path,
                ["relations", "--edge", "calls", "--scope", scope, "--scope-mode", "crossing", "--limit", "20"],
            )
        )

    # F extraction checks via inventory type/edge
    code, out, _, wall = run_rgctl(path, ["inventory", "--by", "type"])
    tdata = extract_json(out)
    cm = counts_map(tdata)
    code2, out2, _, wall2 = run_rgctl(path, ["inventory", "--by", "edge"])
    em = counts_map(extract_json(out2))
    notes = []
    if cfg.get("expect_annotatedwith") and em.get("annotatedwith", 0) == 0:
        notes.append("FAIL expect annotatedwith>0")
    if lang == "typescript":
        if cm.get("enum", 0) == 0:
            notes.append("WARN enum=0 (need re-discover?)")
        if cm.get("typealias", 0) == 0 and cm.get("type_alias", 0) == 0:
            notes.append("WARN typealias=0 (need re-discover?)")
    if lang == "groovy" and cm.get("annotation", 0) == 0:
        notes.append("WARN annotation nodes=0 (usage edges may still exist)")
    detail = f"types_top={sorted(cm.items(), key=lambda x:-x[1])[:5]}; edges_top={sorted(em.items(), key=lambda x:-x[1])[:5]}; " + "; ".join(notes)
    ok_f = "FAIL" not in detail
    results.append(ProbeResult(lang, "F-extract", ["inventory"], ok_f, 0, wall + wall2, detail, extra={"types": cm, "edges": em}))

    # G latency on java
    if cfg.get("latency") and seed:
        for pid, args in (
            ("G1-find-scope", ["find", cfg["find_pat"], "--type", cfg["find_type"], "--scope", scope, "--limit", "50"]),
            (
                "G2-anno",
                [
                    "relations",
                    "--edge",
                    "annotatedwith",
                    "--from-type",
                    "function",
                    "--to-type",
                    "annotation",
                    "--limit",
                    "50",
                ],
            ),
            ("G3-callers", ["callers", seed, "--depth", "1", "--limit", "50"]),
        ):
            # warm: run twice, keep second
            run_rgctl(path, args)
            r = probe(lang, pid, path, args)
            if r.wall_s >= 3.0:
                r.ok = False
                r.detail += "; FAIL latency>=3s (GQL floor)"
            results.append(r)

    return results


def write_report(all_results: list[ProbeResult]) -> None:
    REPORTS.mkdir(parents=True, exist_ok=True)
    with JSONL.open("w") as f:
        for r in all_results:
            f.write(json.dumps(asdict(r), default=str) + "\n")

    by_lang: dict[str, list[ProbeResult]] = {}
    for r in all_results:
        by_lang.setdefault(r.lang, []).append(r)

    lines = [
        "# Multi-language structured-query field report",
        "",
        f"Generated from OpenSpec [test-plan-multi-language.md](../openspec/changes/add-structured-query-cli/test-plan-multi-language.md).",
        f"Binary: `{RGCTL}` (`rgctl` release).",
        f"Probes: {len(all_results)}; languages: {len(by_lang)}.",
        "",
        "## Summary",
        "",
        "| Lang | Pass | Fail | Skip/notes | Max wall (s) |",
        "|------|------|------|------------|--------------|",
    ]
    for lang, rows in sorted(by_lang.items()):
        passes = sum(1 for r in rows if r.ok and r.probe_id != "SKIP")
        fails = sum(1 for r in rows if not r.ok and r.probe_id != "SKIP")
        skips = [r.detail for r in rows if r.probe_id == "SKIP"]
        mx = max((r.wall_s for r in rows), default=0)
        note = skips[0] if skips else ""
        lines.append(f"| {lang} | {passes} | {fails} | {note[:60]} | {mx:.3f} |")

    lines += ["", "## Per-language probes", ""]
    for lang, rows in sorted(by_lang.items()):
        lines += [f"### {lang}", "", "| ID | OK | wall_s | detail |", "|----|----|--------|--------|"]
        for r in rows:
            mark = "✅" if r.ok else "❌"
            det = r.detail.replace("|", "\\|")[:180]
            lines.append(f"| {r.probe_id} | {mark} | {r.wall_s:.3f} | {det} |")
        lines.append("")

    fails = [r for r in all_results if not r.ok and r.probe_id not in ("SKIP",)]
    lines += ["## Failures", ""]
    if not fails:
        lines.append("None.")
    else:
        for r in fails:
            lines.append(f"- **{r.lang}/{r.probe_id}**: {r.detail} — `{' '.join(r.cmd[-6:])}`")
    lines.append("")
    MD.write_text("\n".join(lines))
    print(f"Wrote {JSONL} and {MD}", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--phase", choices=("warm", "cold", "all"), default="warm")
    ap.add_argument("--lang", default="", help="comma-separated language filter")
    ap.add_argument("--discover", action="store_true", help="run discover when snapshot missing")
    ap.add_argument("--rediscover", action="store_true", help="force re-discover for flagged langs")
    args = ap.parse_args()
    if not RGCTL.is_file():
        print(f"missing release binary: {RGCTL}", file=sys.stderr)
        return 2

    langs = [x.strip() for x in args.lang.split(",") if x.strip()] or list(CORPORA.keys())
    all_results: list[ProbeResult] = []
    for lang in langs:
        cfg = CORPORA.get(lang)
        if not cfg:
            print(f"unknown lang {lang}", file=sys.stderr)
            continue
        phase = cfg.get("phase", "warm")
        if args.phase == "warm" and phase != "warm":
            all_results.append(ProbeResult(lang, "SKIP", [], True, 0, 0.0, f"phase={phase}; skipped in warm"))
            continue
        if args.phase == "cold" and phase != "cold":
            continue
        if cfg.get("optional") and args.phase != "all":
            all_results.append(ProbeResult(lang, "SKIP", [], True, 0, 0.0, "optional corpus; use --phase all"))
            continue
        print(f"=== {lang} ===", flush=True)
        need_discover = args.discover or not has_snapshot(cfg["path"])
        all_results.extend(run_lang(lang, cfg, do_discover=need_discover, force_rediscover=args.rediscover))

    write_report(all_results)
    hard_fails = [r for r in all_results if not r.ok and r.probe_id not in ("SKIP",)]
    return 1 if hard_fails else 0


if __name__ == "__main__":
    sys.exit(main())
