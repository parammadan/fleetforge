#!/usr/bin/env bash
# The Milestone 4 demonstration.
#
#   1. Record a session to an append-only JSONL log.
#   2. Make a prediction with preflight.
#   3. Cause real disruption, and a real Brupop state transition.
#   4. Generate an evidence report from the log alone, offline.
#   5. Score the prediction against what actually happened — including the misses.
#
# FleetForge observes. Every change below is made by the operator through
# kubectl (ADR-0012).
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

KUBECONFIG_PATH="infra/local/.kubeconfig-fleetforge-reader"
PORT=${PORT:-8088}
BIN=./target/debug/fleetforge
LOG_DIR="${LOG_DIR:-/tmp/fleetforge-run}"
LOG="${LOG_DIR}/events.jsonl"
BRUPOP_NS=brupop-bottlerocket-aws

cleanup() {
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
  wait 2>/dev/null
}
trap cleanup EXIT

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }
rule() { printf '%s\n' "------------------------------------------------------------"; }

[ -x "$BIN" ] || { echo "build first: cargo build" >&2; exit 1; }
./scripts/make-kubeconfig.sh fleetforge-reader >/dev/null
kubectl config use-context kind-fleetforge-dev >/dev/null 2>&1

rm -rf "$LOG_DIR"; mkdir -p "$LOG_DIR"

say "STEP 1 — start FleetForge with recording enabled"
rule
"$BIN" --kubeconfig "$KUBECONFIG_PATH" --bind "127.0.0.1:${PORT}" \
  --record "$LOG" --run-description "M4 demonstration" >/tmp/ff-report.log 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do
  curl -sf "http://127.0.0.1:${PORT}/readyz" >/dev/null 2>&1 && break
  sleep 0.5
done
echo "  recording to ${LOG}"
sleep 2

NODE=$(kubectl get pods -n demo -l app=web -o json \
  | jq -r '[.items[].spec.nodeName] | group_by(.) | map({node:.[0], n:length}) | sort_by(.n) | .[0].node')
echo "  analyzing node: ${NODE}"

say "STEP 2 — make a prediction"
rule
curl -s -X POST "http://127.0.0.1:${PORT}/api/v1/preflight" \
  -H 'content-type: application/json' \
  -d "{\"node_names\":[\"${NODE}\"],\"desired_concurrency\":1}" \
  | jq -r '"  status=\(.data.summary.status)  predicted evictions=\(.data.summary.predicted_impact.pods_evicted)  workloads=\([.data.summary.affected_workloads[].workload | "\(.namespace)/\(.name)"] | join(", "))"'
sleep 2

say "STEP 3 — cause real disruption (kubectl, not FleetForge)"
rule
VICTIM=$(kubectl get pods -n demo --field-selector "spec.nodeName=${NODE}" -l app=web \
  -o jsonpath='{.items[0].metadata.name}')
echo "  kubectl delete pod ${VICTIM} -n demo"
kubectl delete pod "$VICTIM" -n demo --wait=false >/dev/null
sleep 8

say "STEP 4 — a Brupop state transition (kubectl, not FleetForge)"
rule
echo "  patching BottlerocketShadow status: RebootedIntoUpdate → MonitoringUpdate"
kubectl patch bottlerocketshadow fleetforge-dev-worker2 -n "$BRUPOP_NS" \
  --subresource=status --type=merge \
  -p '{"status":{"current_state":"MonitoringUpdate","current_version":"1.21.0","target_version":"1.21.0","crash_count":0}}' >/dev/null 2>&1 \
  && echo "  patched" || echo "  (Brupop CRD not installed — skipping)"
sleep 6

say "STEP 5 — the raw event log, which is the deliverable"
rule
echo "  $(wc -l < "$LOG" | tr -d ' ') lines. A reviewer checks the report against this:"
echo
echo "  \$ jq -r 'select(.event==\"pod_removed\") | \"\\(.seq) \\(.namespace)/\\(.name) left \\(.node)\"' events.jsonl"
jq -r 'select(.event=="pod_removed") | "    \(.seq)  \(.namespace)/\(.name) left \(.node)"' "$LOG" 2>/dev/null | head -5
echo
echo "  \$ jq -r 'select(.event==\"brupop_state_changed\")' events.jsonl"
jq -r 'select(.event=="brupop_state_changed") | "    \(.seq)  \(.node): \(.state) (current v\(.current_version // "-"))"' "$LOG" 2>/dev/null | head -5
echo
echo "  event types recorded:"
jq -r '.event' "$LOG" | sort | uniq -c | sort -rn | awk '{printf "    %-26s %s\n", $2, $1}'

say "STEP 6 — the evidence report, generated from the log alone"
rule
curl -s "http://127.0.0.1:${PORT}/api/v1/report?format=markdown" > "${LOG_DIR}/report.md"
sed -n '/## Prediction versus actual/,/## Timeline/p' "${LOG_DIR}/report.md" | sed 's/^/  /'

say "     caveats the report states about itself"
rule
sed -n '/## Caveats/,$p' "${LOG_DIR}/report.md" | sed 's/^/  /'

echo
say "full report: ${LOG_DIR}/report.md    raw log: ${LOG}"
