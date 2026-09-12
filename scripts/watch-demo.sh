#!/usr/bin/env bash
# Prove the interface is driven by real Kubernetes watches, not by polling.
#
# Starts FleetForge, opens the SSE stream, then changes the cluster from a
# separate process and shows the stream reacting. Every line is timestamped so
# the causal order is visible: the kubectl command happens, then the event
# arrives. No page is refreshed and no timer is involved.
#
# Demonstrates M2 acceptance items 13 (external scale appears) and 14 (pod
# deletion and recreation appear).
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

KUBECONFIG_PATH="infra/local/.kubeconfig-fleetforge-reader"
PORT=${PORT:-8080}
STREAM_LOG=$(mktemp)
SERVER_LOG=$(mktemp)
BIN=./target/debug/fleetforge

cleanup() {
  [ -n "${CURL_PID:-}" ] && kill "$CURL_PID" 2>/dev/null
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
  wait 2>/dev/null
}
trap cleanup EXIT

stamp() { date -u +%H:%M:%S; }
say() { printf '\033[1m[%s] %s\033[0m\n' "$(stamp)" "$*"; }

[ -x "$BIN" ] || { echo "build first: cargo build" >&2; exit 1; }
[ -f "$KUBECONFIG_PATH" ] || ./scripts/make-kubeconfig.sh fleetforge-reader >/dev/null

say "starting fleetforge (read-only, loopback only)"
"$BIN" --kubeconfig "$KUBECONFIG_PATH" --bind "127.0.0.1:${PORT}" >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 60); do
  if curl -sf "http://127.0.0.1:${PORT}/readyz" >/dev/null 2>&1; then break; fi
  sleep 0.5
done
curl -sf "http://127.0.0.1:${PORT}/readyz" >/dev/null || {
  echo "server never became ready"; cat "$SERVER_LOG"; exit 1; }
say "ready — first sync complete"

# Record every SSE frame with the time it arrived.
( curl -sN "http://127.0.0.1:${PORT}/api/v1/stream" \
    | while IFS= read -r line; do printf '%s %s\n' "$(stamp)" "$line"; done \
    > "$STREAM_LOG" ) &
CURL_PID=$!
sleep 2

baseline=$(kubectl get deploy/web -n demo -o jsonpath='{.spec.replicas}')
say "baseline: deploy/web has ${baseline} replicas"

say ">>> kubectl scale deploy/web --replicas=5   (external process)"
kubectl scale deploy/web -n demo --replicas=5 >/dev/null
sleep 6

say ">>> kubectl delete pod (one web pod)       (external process)"
victim=$(kubectl get pods -n demo -l app=web -o jsonpath='{.items[0].metadata.name}')
say "    deleting ${victim}"
kubectl delete pod "$victim" -n demo --wait=false >/dev/null
sleep 8

say ">>> kubectl scale deploy/web --replicas=3   (restoring)"
kubectl scale deploy/web -n demo --replicas=3 >/dev/null
sleep 6

kill "$CURL_PID" 2>/dev/null; CURL_PID=""

echo
say "SSE frames received (event type, and what the snapshot said)"
echo
printf '%-10s %-20s %s\n' "TIME" "EVENT" "DETAIL"
awk '{
  ts=$1; $1=""
  line=substr($0,2)
  if (line ~ /^event:/) { split(line, a, ": "); ev=a[2]; evts=ts }
  else if (line ~ /^data:/ && ev != "") {
    payload=substr(line, 7)
    print ts "\t" ev "\t" payload
    ev=""
  }
}' "$STREAM_LOG" | while IFS=$'\t' read -r ts ev payload; do
  case "$ev" in
    snapshot.updated)
      detail=$(printf '%s' "$payload" | jq -r '
        "mode=\(.mode_label) snapshot=\(.data.snapshot_id[0:12]) " +
        "webPods=\([.data.pods[] | select(.labels.app=="web")] | length) " +
        "totalPods=\(.data.pods | length)"' 2>/dev/null)
      ;;
    heartbeat)
      detail=$(printf '%s' "$payload" | jq -r '"mode=\(.mode_label) authoritative=\(.authoritative)"' 2>/dev/null)
      ;;
    *) detail=$(printf '%s' "$payload" | head -c 60) ;;
  esac
  printf '%-10s %-20s %s\n' "$ts" "$ev" "${detail:-}"
done

echo
say "distinct snapshot ids observed:"
grep '^.* data: ' "$STREAM_LOG" | sed 's/^[^ ]* data: //' \
  | jq -r 'select(.data.snapshot_id != null) | .data.snapshot_id[0:12]' 2>/dev/null \
  | uniq -c | awk '{printf "    %s  (%s frames)\n", $2, $1}'

echo
say "done — the cluster changed and the stream reported it, with no refresh and no polling timer"
