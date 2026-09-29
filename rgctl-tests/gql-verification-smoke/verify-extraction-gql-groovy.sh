#!/usr/bin/env bash
# Groovy extraction-depth GQL + rgctl command verification.
# Fixture: rgctl-tests/ecommerce-groovy
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RGCTL_TESTS="$(cd "${SCRIPT_DIR}/.." && pwd)"
# shellcheck source=extraction-gql-common.sh
source "${SCRIPT_DIR}/extraction-gql-common.sh"
RGCTL_CMD_ID=groovy
# shellcheck source=rgctl-commands-config.sh
source "${SCRIPT_DIR}/rgctl-commands-config.sh"
# shellcheck source=rgctl-commands-common.sh
source "${SCRIPT_DIR}/rgctl-commands-common.sh"

FIXTURE="${RGCTL_TESTS}/ecommerce-groovy"

run_fixture_gql() {
  echo "--- fixture GQL: ${FIXTURE} ---"
  discover_repo "${FIXTURE}" "${RGCTL_CMD_DISCOVER_EXTRA[@]}"
  assert_node_min "classes" Class 1 "${FIXTURE}"
  assert_edge_min "call resolution (CALLS)" CALLS 1 "${FIXTURE}"
  assert_gql_min "method FQN (OrderService.process)" \
    "MATCH (n:Function) WHERE n.qualified_name = 'com.example.ecommerce.OrderService.process' RETURN n LIMIT 5" 1 "${FIXTURE}"
  assert_gql_min "constructor FQN (OrderDTO.<init>)" \
    "MATCH (n:Function) WHERE n.qualified_name = 'com.example.ecommerce.OrderDTO.<init>' RETURN n LIMIT 5" 1 "${FIXTURE}"
}

echo "=== groovy extraction GQL + commands ==="
run_fixture_gql
RGCTL_CMD_SKIP_DISCOVER=1 run_rgctl_commands_suite "${FIXTURE}"
echo "=== groovy extraction GQL + commands: OK ==="
