#!/usr/bin/env bash
# Bump the lockstep workspace version (option 2: cargo set-version).
#
# Usage:
#   ./scripts/bump-version.sh patch          # 0.4.17 -> 0.4.18
#   ./scripts/bump-version.sh minor          # 0.4.17 -> 0.5.0
#   ./scripts/bump-version.sh 0.4.18         # set exact version
#
# Updates:
#   - [workspace.package] version (crates use version.workspace = true)
#   - path crate version= pins under [workspace.dependencies]
#
# Does NOT commit or tag. For bump+commit+tag use:
#   cargo release <LEVEL|VERSION> --workspace --execute
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! cargo set-version -h >/dev/null 2>&1; then
  echo "error: cargo-edit (cargo set-version) is required" >&2
  echo "  cargo install cargo-edit --locked" >&2
  exit 1
fi

ARG="${1:-}"
if [[ -z "$ARG" ]]; then
  echo "usage: $0 <patch|minor|major|X.Y.Z>" >&2
  exit 1
fi

read_workspace_version() {
  python3 - <<'PY'
import re
from pathlib import Path
text = Path("Cargo.toml").read_text()
m = re.search(r'(?ms)^\[workspace\.package\].*?^version = "([^"]+)"', text)
if not m:
    raise SystemExit("could not find [workspace.package] version")
print(m.group(1))
PY
}

prev="$(read_workspace_version)"

if [[ "$ARG" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]]; then
  cargo set-version --workspace "$ARG"
else
  case "$ARG" in
    patch|minor|major)
      cargo set-version --bump "$ARG" --workspace
      ;;
    *)
      echo "error: unknown bump '$ARG' (use patch|minor|major|X.Y.Z)" >&2
      exit 1
      ;;
  esac
fi

new="$(read_workspace_version)"

# Keep [workspace.dependencies] path pins in lockstep (needed for publish metadata).
python3 - "$prev" "$new" <<'PY'
import re, sys
from pathlib import Path
prev, new = sys.argv[1], sys.argv[2]
path = Path("Cargo.toml")
text = path.read_text()

def repl_block(match: re.Match) -> str:
    return match.group(0).replace(f'version = "{prev}"', f'version = "{new}"')

text2, n = re.subn(
    r'(?ms)^\[workspace\.dependencies\]\n.*?(?=^\[|\Z)',
    repl_block,
    text,
    count=1,
)
if n != 1:
    print("warning: could not locate [workspace.dependencies] block to sync", file=sys.stderr)
else:
    path.write_text(text2)
    print(f"synced [workspace.dependencies] path versions {prev} -> {new}")
PY

echo "workspace version: $prev -> $new"

# Keep README latest-release link in sync when present.
if [[ -f README.md ]]; then
  python3 - "$prev" "$new" <<'PY'
import sys
from pathlib import Path
prev, new = sys.argv[1], sys.argv[2]
path = Path("README.md")
text = path.read_text()
updated = text.replace(f"v{prev}", f"v{new}").replace(
    f"docs/releases/v{prev}.md", f"docs/releases/v{new}.md"
)
if updated != text:
    path.write_text(updated)
    print(f"updated README.md release links {prev} -> {new}")
PY
fi

echo "next: review git diff, then either commit manually or:"
echo "  cargo release $new --workspace --execute   # commit + tag v$new + push"
