#!/usr/bin/env bash
# Show what FleetForge does when the API server goes away and comes back.
#
# The failure this guards against: a watch dies, the collector reports nothing,
# and the interface renders an empty cluster. An empty PodDisruptionBudget list
# means no blockers, which means "safe to drain" — so a tool that empties out
# when its connection breaks is actively dangerous.
#
# Pauses the control-plane container (SIGSTOP on every process in it), which is
# a clean way to make the API server unreachable without destroying anything.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

CONTROL_PLANE=fleetforge-dev-control-plane
KUBECONFIG_PATH="infra/local/.kubeconfig-fleetforge-reader"
PORT=${PORT:-8081}
BIN=./target/debug/fleetforge
SERVER_LOG=$(mktemp)

cleanup() {
  docker unpause "$CONTROL_PLANE" >/dev/null 2>&1
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
  wait 2>/dev/null
}
trap cleanup EXIT

stamp() { date -u +%H:%M:%S; }
say() { printf '\033[1m[%s] %s\033[0m\n' "$(stamp)" "$*"; }

probe() {
  local label=$1
  local json
  json=$(curl -s --max-time 5 "http://127.0.0.1:${PORT}/api/v1/environment" 2>/dev/null)
  if [ -z "$json" ]; then
    printf '  %-28s %s\n' "$label" "API did not respond"
    return
  fi
  printf '  %-28s %s\n' "$label" "$(printf '%s' "$json" | jq -r '
    "authoritative=\(.authoritative)  " +
    ([.coverage[] | select(.kind=="PodDisruptionBudget")][0] |
      "pdbs=\(.observed_count) status=\(.status.state)")')"
}

[ -x "$BIN" ] || { echo "build first: cargo build" >&2; exit 1; }
[ -f "$KUBECONFIG_PATH" ] || ./scripts/make-kubeconfig.sh fleetforge-reader >/dev/null

say "starting fleetforge"
"$BIN" --kubeconfig "$KUBECONFIG_PATH" --bind "127.0.0.1:${PORT}" >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do
  curl -sf "http://127.0.0.1:${PORT}/readyz" >/dev/null 2>&1 && break
  sleep 0.5
done
say "healthy baseline"
probe "before"

say ">>> docker pause ${CONTROL_PLANE}  (API server becomes unreachable)"
docker pause "$CONTROL_PLANE" >/dev/null
sleep 20
probe "during outage"
say "    note: the PDB count is retained and the status changes."
say "    The alternative — zeroing the count — would read as 'no blockers'."

say ">>> docker unpause ${CONTROL_PLANE}"
docker unpause "$CONTROL_PLANE" >/dev/null
for _ in $(seq 1 40); do
  state=$(curl -s --max-time 5 "http://127.0.0.1:${PORT}/api/v1/environment" 2>/dev/null \
    | jq -r '.authoritative' 2>/dev/null)
  [ "$state" = "true" ] && break
  sleep 2
done
probe "after recovery"

echo
say "collector log (redacted lines only — no URLs or tokens):"
grep -E "watch failed|connected read-only" "$SERVER_LOG" | tail -6 | sed 's/^/    /'
echo
say "done"
