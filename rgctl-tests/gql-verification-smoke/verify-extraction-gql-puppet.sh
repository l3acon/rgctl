#!/usr/bin/env bash
# Puppet extraction GQL + rgctl command verification.
# Fixture: rgctl-tests/ecommerce-puppet
# Gate B corpus: deferred (RGCTL_PUPPET_REPO) — see docs/puppet-extract-honesty.md
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RGCTL_TESTS="$(cd "${SCRIPT_DIR}/.." && pwd)"
# shellcheck source=extraction-gql-common.sh
source "${SCRIPT_DIR}/extraction-gql-common.sh"
RGCTL_CMD_ID=puppet
# shellcheck source=rgctl-commands-config.sh
source "${SCRIPT_DIR}/rgctl-commands-config.sh"
# shellcheck source=rgctl-commands-common.sh
source "${SCRIPT_DIR}/rgctl-commands-common.sh"

FIXTURE="${RGCTL_TESTS}/ecommerce-puppet"

run_fixture_gql() {
  echo "--- fixture GQL: ${FIXTURE} ---"
  discover_repo "${FIXTURE}" "${RGCTL_CMD_DISCOVER_EXTRA[@]}"
  assert_node_min "Puppet classes" PuppetClass 1 "${FIXTURE}"
  assert_node_min "Puppet modules (metadata)" PuppetModule 1 "${FIXTURE}"
  assert_edge_min "include graph (INCLUDESCLASS)" INCLUDESCLASS 1 "${FIXTURE}"
  assert_edge_min "function calls (CALLS)" CALLS 1 "${FIXTURE}"
}

echo "=== puppet extraction GQL + commands ==="
run_fixture_gql
RGCTL_CMD_SKIP_DISCOVER=1 run_rgctl_commands_suite "${FIXTURE}"
echo "=== puppet extraction GQL + commands: OK ==="
