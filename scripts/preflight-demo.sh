#!/usr/bin/env bash
# The Milestone 3 demonstration.
#
#   1. Observe a real cluster.
#   2. Create a genuinely unsafe condition with kubectl.
#   3. Run preflight — blocked, with the exact object and the arithmetic.
#   4. Fix the condition with kubectl.
#   5. Re-run — the risk clears.
#
# Every mutation here is performed by the operator through kubectl. FleetForge
# holds no mutating client and changes nothing (ADR-0012); it reads, analyzes,
# and explains.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

KUBECONFIG_PATH="infra/local/.kubeconfig-fleetforge-reader"
PORT=${PORT:-8084}
BIN=./target/debug/fleetforge
# Chosen below: the worker holding the fewest app=web pods. With 3 replicas
# across 2 workers, one worker necessarily holds 2 — and draining that one is
# blocked even by the permissive budget, because a drain evicts a node's pods
# together while the budget allows one disruption at a time. That is a real
# finding (FF-PDB-002) and it is shown, but it is not the condition this
# demonstration is about.
NODE=${NODE:-}

cleanup() {
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
  # Always restore the permissive budget, whatever happened.
  kubectl patch pdb web-pdb -n demo --type=merge \
    -p '{"spec":{"minAvailable":2}}' >/dev/null 2>&1
  wait 2>/dev/null
}
trap cleanup EXIT

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }
rule() { printf '%s\n' "------------------------------------------------------------"; }

[ -x "$BIN" ] || { echo "build first: cargo build" >&2; exit 1; }
# Always reissue: tokens are short-lived by design (ADR-0019), and a demo
# that assumes yesterday's credential still works is a demo that fails.
./scripts/make-kubeconfig.sh fleetforge-reader >/dev/null

"$BIN" --kubeconfig "$KUBECONFIG_PATH" --bind "127.0.0.1:${PORT}" >/tmp/ff-preflight.log 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do
  curl -sf "http://127.0.0.1:${PORT}/readyz" >/dev/null 2>&1 && break
  sleep 0.5
done

# Wait until the collector reflects a given disruptionsAllowed value, so the
# demo compares against fresh state rather than racing the watch.
wait_for_pdb() {
  local want=$1
  for _ in $(seq 1 40); do
    got=$(curl -s "http://127.0.0.1:${PORT}/api/v1/pdbs" 2>/dev/null \
      | jq -r '.data[] | select(.name=="web-pdb") | .disruptions_allowed' 2>/dev/null)
    [ "$got" = "$want" ] && return 0
    sleep 0.5
  done
  return 1
}

preflight() {
  curl -s -X POST "http://127.0.0.1:${PORT}/api/v1/preflight" \
    -H 'content-type: application/json' \
    -d "{\"node_names\":[\"${NODE}\"],\"desired_concurrency\":1}" 2>/dev/null
}

summarise() {
  jq -r '
    "  mode             \(.mode_label)",
    "  status           \(.data.summary.status | ascii_upcase)",
    "  max concurrency  \(.data.summary.recommended_max_concurrency)",
    "  reason           \(.data.summary.concurrency_constraint.reason // "no constraint")",
    "  pods evicted     \(.data.summary.predicted_impact.pods_evicted)",
    "  PDB margin       \(.data.summary.predicted_impact.minimum_pdb_margin)",
    "  findings         \([.data.findings[] | "\(.id)(\(.severity))"] | join(" "))"
  '
}

kubectl config use-context kind-fleetforge-dev >/dev/null 2>&1

if [ -z "$NODE" ]; then
  NODE=$(kubectl get pods -n demo -l app=web -o json \
    | jq -r '[.items[].spec.nodeName] | group_by(.) | map({node: .[0], n: length})
             | sort_by(.n) | .[0].node')
  BUSY=$(kubectl get pods -n demo -l app=web -o json \
    | jq -r '[.items[].spec.nodeName] | group_by(.) | map({node: .[0], n: length})
             | sort_by(-.n) | .[0].node')
fi

say "SETUP — where the web pods actually are"
rule
kubectl get pods -n demo -l app=web -o custom-columns=POD:.metadata.name,NODE:.spec.nodeName \
  --no-headers | sed 's/^/  /'
echo
echo "  analyzing:  ${NODE}  (fewest covered pods)"
echo "  note:       ${BUSY} holds more, and is blocked even by the permissive"
echo "              budget — shown at the end."

say "STEP 1 — observe the real cluster"
rule
curl -s "http://127.0.0.1:${PORT}/api/v1/pdbs" | jq -r '
  "  \(.mode_label)  web-pdb: minAvailable=\(.data[0].min_available) " +
  "currentHealthy=\(.data[0].current_healthy) " +
  "disruptionsAllowed=\(.data[0].disruptions_allowed)  rv=\(.data[0].provenance.resource_version)"'
echo "  verify with: kubectl get pdb web-pdb -n demo -o wide"

say "STEP 2 — preflight on ${NODE}, before any change"
rule
preflight | summarise

say "STEP 3 — create the unsafe condition (kubectl, not FleetForge)"
rule
echo "  kubectl patch pdb web-pdb -n demo --type=merge -p '{\"spec\":{\"minAvailable\":3}}'"
kubectl patch pdb web-pdb -n demo --type=merge -p '{"spec":{"minAvailable":3}}' >/dev/null
kubectl get pdb web-pdb -n demo --no-headers | sed 's/^/  kubectl says: /'
wait_for_pdb 0 || echo "  (collector did not observe the change in time)"

say "STEP 4 — preflight again: BLOCKED"
rule
RESULT=$(preflight)
printf '%s' "$RESULT" | summarise

say "     the evidence behind it"
rule
printf '%s' "$RESULT" | jq -r '
  .data.findings[] | select(.id=="FF-PDB-001") |
  "  \(.title)",
  "",
  "  confidence: \(.confidence)",
  "",
  "  evidence:",
  (.evidence[] | "    \(.resource.kind)/\(.resource.name) \(.field_path) = \(.value)" +
    (if .note then "\n      (\(.note))" else "" end)),
  "",
  "  calculation:",
  "    \(.calculation.formula)",
  (.calculation.inputs[] | "      \(.[0]) = \(.[1])"),
  "    => \(.calculation.result) \(.calculation.unit // "")",
  "",
  "  limitations:",
  (.limitations[] | "    - \(.)")'

say "STEP 5 — fix it (kubectl, not FleetForge)"
rule
echo "  kubectl patch pdb web-pdb -n demo --type=merge -p '{\"spec\":{\"minAvailable\":2}}'"
kubectl patch pdb web-pdb -n demo --type=merge -p '{"spec":{"minAvailable":2}}' >/dev/null
kubectl get pdb web-pdb -n demo --no-headers | sed 's/^/  kubectl says: /'
wait_for_pdb 1 || echo "  (collector did not observe the change in time)"

say "STEP 6 — preflight again: the risk has cleared"
rule
preflight | summarise

say "ASIDE — the busier node, with the permissive budget restored"
rule
curl -s -X POST "http://127.0.0.1:${PORT}/api/v1/preflight" \
  -H 'content-type: application/json' \
  -d "{\"node_names\":[\"${BUSY}\"],\"desired_concurrency\":1}" 2>/dev/null | jq -r '
    "  \(.data.summary.status | ascii_upcase): \(.data.summary.concurrency_constraint.reason // "no constraint")"'
echo "  A different node, the same budget, a different answer. Which node you"
echo "  pick is part of the question, and preflight answers it per selection."

say "Note the snapshot ids: each preflight names the exact state it analyzed."
rule
printf '%s' "$RESULT" | jq -r '"  blocked run analyzed  \(.data.findings[0].snapshot_id[0:16])"'
preflight | jq -r '"  cleared run analyzed  \(.data.findings[0].snapshot_id[0:16])"'
