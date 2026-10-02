#!/usr/bin/env bash
# Build the with-limits smoke image and run it with example/linux mounted.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="${IMAGE:-rgctl-with-limits-smoke}"
LINUX="${RGCTL_LINUX_REPO:-$ROOT/example/linux}"
ENGINE="${CONTAINER_ENGINE:-}"
SMOKE_SCOPE="${SMOKE_SCOPE:-scripts}"
MEMORY="${CONTAINER_MEMORY:-4g}"

if [[ -z "$ENGINE" ]]; then
  if command -v podman >/dev/null 2>&1; then
    ENGINE=podman
  elif command -v docker >/dev/null 2>&1; then
    ENGINE=docker
  else
    echo "ERROR: need podman or docker" >&2
    exit 1
  fi
fi

if [[ ! -d "$LINUX" ]]; then
  echo "ERROR: linux corpus missing at $LINUX" >&2
  echo "  Run: ./scripts/fetch-profile-repos.sh   # or set RGCTL_LINUX_REPO" >&2
  exit 1
fi

if [[ "$ENGINE" == podman ]]; then
  if ! podman info >/dev/null 2>&1; then
    echo "==> starting podman machine"
    podman machine start
    # Wait until the API is actually reachable (start can race ahead of the socket).
    for _ in $(seq 1 60); do
      if podman info >/dev/null 2>&1; then
        break
      fi
      sleep 1
    done
    podman info >/dev/null || {
      echo "ERROR: podman machine started but API is unreachable" >&2
      exit 1
    }
  fi
fi

echo "==> building $IMAGE (engine=$ENGINE)"
"$ENGINE" build -f "$ROOT/tests/Containerfile" -t "$IMAGE" "$ROOT"

echo "==> running smoke (mount $LINUX -> /corpus, memory=$MEMORY, scope=$SMOKE_SCOPE)"
"$ENGINE" run --rm \
  --memory="$MEMORY" \
  -e "SMOKE_SCOPE=$SMOKE_SCOPE" \
  -e "LIMITS_SPEC=${LIMITS_SPEC:-max-mem-mb=4096,threads=1}" \
  -v "$LINUX:/corpus:Z" \
  "$IMAGE"

echo "==> container with-limits smoke OK"
