#!/usr/bin/env bash
# Pre-flight PR check script — mirrors .github/workflows/ci.yml locally so
# contributors and coding agents catch formatting, clippy, AST-coverage drift,
# commit-trailer policy, and integration-test compilation/runtime failures
# before opening or pushing to a pull request.
#
# Usage:
#   ./scripts/preflight-pr.sh              # Standard pre-PR checks (lint + compile all tests + unit/QE tests)
#   ./scripts/preflight-pr.sh --quick      # Fast gate (commit checks + fmt + all clippy gates + compile all --tests/--benches)
#   ./scripts/preflight-pr.sh --full       # Full CI mirror (adds --release CLI golden paths, scale/perf gates, AST coverage tracking)
#   ./scripts/preflight-pr.sh --fix        # Run `cargo fmt --all` first, then run checks
#   ./scripts/preflight-pr.sh --base main  # Override base git ref for commit-range checks

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

MODE="standard"
FIX_FMT=0
BASE_REF=""
FEATURES="${FEATURES:-}"

usage() {
  cat <<'EOF'
Usage: ./scripts/preflight-pr.sh [OPTIONS]

Options:
  --quick            Fast pre-flight: commit checks, cargo check, cargo fmt, all CI clippy
                     gates, and `cargo test --workspace --lib --bins --tests --no-run`
  --full             Full CI mirror: standard checks + release CLI golden paths, scale/perf
                     gates, benchmark compilation, and AST handler coverage gate
  --fix              Run `cargo fmt --all` before checking formatting
  --base <git-ref>   Base ref for commit-range checks (default: auto-detect upstream/main, origin/main, or main)
  --features <list>  Cargo features to pass (mirrors CI FEATURES input; default: "")
  -h, --help         Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --quick)
      MODE="quick"
      shift
      ;;
    --full)
      MODE="full"
      shift
      ;;
    --fix)
      FIX_FMT=1
      shift
      ;;
    --base)
      BASE_REF="${2:-}"
      shift 2
      ;;
    --features)
      FEATURES="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
# Match .github/workflows/ci.yml: fail on grammar <-> *-ast-coverage.json drift
export RGCTL_AST_COVERAGE_STRICT=1

step() {
  printf '\n\033[1;34m==> %s\033[0m\n' "$1"
}

pass() {
  printf '\033[1;32m✓ %s\033[0m\n' "$1"
}

warn() {
  printf '\033[1;33m⚠ %s\033[0m\n' "$1"
}

# 1. Resolve base ref for commit checks
if [[ -z "$BASE_REF" ]]; then
  for candidate in upstream/main origin/main main; do
    if git rev-parse --verify --quiet "$candidate" >/dev/null; then
      BASE_REF="$candidate"
      break
    fi
  done
fi

if [[ -n "$BASE_REF" ]]; then
  MERGE_BASE="$(git merge-base "$BASE_REF" HEAD 2>/dev/null || true)"
  if [[ -n "$MERGE_BASE" && "$MERGE_BASE" != "$(git rev-parse HEAD)" ]]; then
    step "Checking commits in ${MERGE_BASE:0:8}..HEAD (no AI trailers & DCO sign-off)"
    python3 scripts/check-no-ai-commit-trailers.py --range "${MERGE_BASE}..HEAD"

    MISSING_DCO=0
    while IFS= read -r sha; do
      [[ -z "$sha" ]] && continue
      if ! git log -1 --format=%B "$sha" | grep -qi '^Signed-off-by:'; then
        warn "Commit ${sha:0:8} is missing 'Signed-off-by:' (DCO). Use 'git commit -s'."
        MISSING_DCO=1
      fi
    done < <(git rev-list --no-merges "${MERGE_BASE}..HEAD")

    if [[ "$MISSING_DCO" -eq 0 ]]; then
      pass "Commit range checks passed (no AI trailers, DCO sign-off present)"
    fi
  fi
fi

# 2. Formatting
if [[ "$FIX_FMT" -eq 1 ]]; then
  step "Formatting workspace (cargo fmt --all)"
  cargo fmt --all
fi

step "CI Gate 1/6: cargo fmt --all -- --check"
cargo fmt --all -- --check
pass "Formatting check passed"

# 3. Cold-compile binary check (matches CI `check` job)
step "CI Gate 2/6: cargo check --locked --bin rgctl"
cargo check --locked --bin rgctl
pass "cargo check --locked --bin rgctl passed"

# 4. Clippy gates (exact commands from .github/workflows/ci.yml)
step "CI Gate 3/6: cargo clippy --features \"${FEATURES}\" --lib --bins -- -D warnings"
cargo clippy --features "${FEATURES}" --lib --bins -- -D warnings
pass "Workspace clippy (--lib --bins) passed"

step "CI Gate 4/6: cargo clippy -p rgctl-analysis --features \"${FEATURES}\" -- -D warnings"
cargo clippy -p rgctl-analysis --features "${FEATURES}" -- -D warnings
pass "rgctl-analysis clippy passed"

step "CI Gate 5/6: cargo clippy -p rgctl-graph -p rgctl-core --features \"${FEATURES}\" -- -D warnings"
cargo clippy -p rgctl-graph -p rgctl-core --features "${FEATURES}" -- -D warnings
pass "rgctl-graph & rgctl-core clippy passed"

# 5. Test compilation & execution
if [[ "$MODE" == "quick" ]]; then
  step "CI Gate 6/6 (quick): Compile all unit & integration tests + run library unit tests"
  cargo test --features "${FEATURES}" --workspace --lib --bins --tests --no-run
  cargo test --features "${FEATURES}" --workspace --lib --no-fail-fast
  pass "All workspace test targets compiled cleanly and workspace library unit tests passed"
else
  step "Scrubbing gitignored .rgctl caches in test fixtures to match clean CI checkout"
  find rgctl-tests tests/fixtures -name ".rgctl" -type d -prune -exec rm -rf {} + 2>/dev/null || true
  pass "Fixture .rgctl caches cleaned"

  step "CI Gate 6/6: cargo test --features \"${FEATURES}\" --workspace --lib --bins --tests --no-fail-fast"
  cargo test --features "${FEATURES}" --workspace --lib --bins --tests --no-fail-fast

  step "Running CI QE & correctness gates"
  cargo test --features "${FEATURES}" --test map_collision_qe
  cargo test --features "${FEATURES}" --test graph_correctness
  cargo test --features "${FEATURES}" --test semantic_search_qe
  cargo test --features "${FEATURES}" --test cross_feature_qe
  cargo test --features "${FEATURES}" --test graph_determinism_qe
  cargo test --features "${FEATURES}" --test stable_key_invariance_qe
  cargo test --features "${FEATURES}" --test columnar_zero_cold_touch_gate
  cargo test --features "${FEATURES}" --test e2e_workflow_equivalence
  cargo test --features "${FEATURES}" --test e2e_feature_contracts
  cargo test --features "${FEATURES}" --test cli_output
  cargo test --features "${FEATURES}" -p rgctl-extraction --test single_pass_complexity_parity
  cargo test --features "${FEATURES}" --test inheritance_edges --test cross_language_mro_and_di
  cargo test --features "${FEATURES}" --test slicing --test taint_analysis
  cargo test --features "${FEATURES}" --test community_audit --test centrality_audit
  cargo test --features "${FEATURES}" --test incremental_performance_integration
  pass "Standard QE & correctness gates passed"

  step "Release CLI golden paths & scale/perf gates (mirrors CI release steps)"
  cargo test --release --features "${FEATURES}" --test subprocess_golden_path
  cargo test --release --features "${FEATURES}" --test all_commands_sanity
  cargo test --release --features "${FEATURES}" --test blast_radius_perf
  cargo test --release --features "${FEATURES}" --test scale_perf_ci_gate -- --nocapture
  cargo test --release --features "${FEATURES}" --test community_audit --test centrality_audit --test centrality_approx_scale -- --nocapture

  step "Benchmark compilation smoke check (cargo bench --no-run)"
  cargo bench --no-run

  step "AST Handler Runtime Coverage Gate (RGCTL_AST_COVERAGE_TRACK=1)"
  RGCTL_AST_COVERAGE_TRACK=1 cargo test --features "${FEATURES}" \
    -p rgctl-ast-coverage \
    -p rgctl-languages \
    -p rgctl-lang-c \
    -p rgctl-lang-cpp \
    -p rgctl-lang-csharp \
    -p rgctl-lang-go \
    -p rgctl-lang-java \
    -p rgctl-lang-javascript \
    -p rgctl-lang-kotlin \
    -p rgctl-lang-php \
    -p rgctl-lang-python \
    -p rgctl-lang-ruby \
    -p rgctl-lang-rust \
    -p rgctl-lang-typescript \
    --tests
  pass "Full release, benchmark, and AST coverage gates passed"
fi

printf '\n\033[1;32m============================================================\033[0m\n'
printf '\033[1;32m  rgctl pre-flight PR check (%s) PASSED\033[0m\n' "$MODE"
printf '\033[1;32m============================================================\033[0m\n'
