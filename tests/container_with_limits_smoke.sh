#!/usr/bin/env bash
# Smoke-test `rgctl discover --with-limits` inside a memory-constrained container.
# Expects the Linux corpus mounted at /corpus (see tests/Containerfile).
set -euo pipefail

CORPUS="${CORPUS:-/corpus}"
# Scoped path under the mounted tree for a fast smoke (override with SMOKE_SCOPE=. for full tree).
SMOKE_SCOPE="${SMOKE_SCOPE:-scripts}"
LIMITS_SPEC="${LIMITS_SPEC:-max-mem-mb=4096,threads=1}"
RGCTL_BIN="${RGCTL_BIN:-rgctl}"

log() { printf '==> %s\n' "$*"; }
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

[[ -x "$(command -v "$RGCTL_BIN")" ]] || die "rgctl binary not found: $RGCTL_BIN"
[[ -d "$CORPUS" ]] || die "corpus mount missing at $CORPUS (pass -v example/linux:/corpus)"

TARGET="$CORPUS"
if [[ "$SMOKE_SCOPE" != "." && -n "$SMOKE_SCOPE" ]]; then
  TARGET="$CORPUS/$SMOKE_SCOPE"
fi
[[ -d "$TARGET" ]] || die "smoke scope not found: $TARGET"

log "rgctl $($RGCTL_BIN --version 2>/dev/null || true)"
log "corpus=$CORPUS scope=$SMOKE_SCOPE limits=$LIMITS_SPEC"

# --- CLI surface ---
HELP="$($RGCTL_BIN discover --help)"
echo "$HELP" | grep -q -- '--with-limits' \
  || die "discover --help missing --with-limits"

# Invalid spec must fail fast (no discover).
if $RGCTL_BIN discover "$TARGET" --with-limits 'not-a-spec' >/tmp/bad-limits.out 2>/tmp/bad-limits.err; then
  die "expected invalid --with-limits to fail"
fi
grep -qi 'with-limits\|invalid\|unknown\|want key' /tmp/bad-limits.err /tmp/bad-limits.out \
  || die "invalid --with-limits error message unclear"

# --- Constrained discover ---
# Artifacts under the scanned tree (or CORPUS if we cd there with positional .).
cd "$TARGET"
rm -rf .rgctl
# Also clear parent corpus .rgctl if a prior full run left one (mount is shared).
rm -rf "$CORPUS/.rgctl"

LOG=/tmp/rgctl-with-limits-smoke.log
set +e
NO_COLOR=1 RUST_LOG="${RUST_LOG:-info}" \
  "$RGCTL_BIN" discover . --with-limits "$LIMITS_SPEC" -l c -v \
  >"$LOG" 2>&1
RC=$?
set -e
# Strip ANSI in case the TTY still colors tracing output.
CLEAN=$(sed 's/\x1b\[[0-9;]*m//g' "$LOG")
printf '%s\n' "$CLEAN"

[[ $RC -eq 0 ]] || die "discover --with-limits failed (exit $RC)"

printf '%s\n' "$CLEAN" | grep -q 'discover --with-limits active' \
  || die "missing 'discover --with-limits active' log line"
printf '%s\n' "$CLEAN" | grep -Eq 'max_mem_mb[= ].*Some\([0-9]+\)|max_mem_mb=Some\(' \
  || die "limits log missing max_mem_mb"
printf '%s\n' "$CLEAN" | grep -Eq 'threads[= ].*Some\([0-9]+\)|threads=Some\(' \
  || die "limits log missing threads"

[[ -f .rgctl/graph.snapshot.bin ]] || die "missing .rgctl/graph.snapshot.bin after discover"

# Env-form limits (no flag) should also activate.
rm -rf .rgctl
LOG2=/tmp/rgctl-with-limits-env.log
set +e
NO_COLOR=1 RUST_LOG=info RGCTL_WITH_LIMITS="$LIMITS_SPEC" \
  "$RGCTL_BIN" discover . -l c -v \
  >"$LOG2" 2>&1
RC2=$?
set -e
CLEAN2=$(sed 's/\x1b\[[0-9;]*m//g' "$LOG2")
[[ $RC2 -eq 0 ]] || die "discover via RGCTL_WITH_LIMITS failed (exit $RC2)"
printf '%s\n' "$CLEAN2" | grep -q 'discover --with-limits active' \
  || die "RGCTL_WITH_LIMITS did not activate limits"

log "PASS: --with-limits smoke on $TARGET"
