#!/usr/bin/env bash
# Prove that an invalid or expired credential becomes an explicit unauthorized
# state, not an empty cluster.
#
# FleetForge uses short-lived TokenRequest credentials (ADR-0019), so expiry is
# a routine event rather than an exotic one. The failure to guard against: the
# token lapses, every watch returns 401, the interface renders empty lists, an
# empty PodDisruptionBudget list reads as "no blockers", and a drain is reported
# safe on the strength of data FleetForge could not read.
#
Covers both cases:
#   A. Invalid at startup  — FleetForge must refuse to start, not serve an
#      empty view.
#   B. Invalidated mid-run — the running collector must transition to a
#      non-authoritative state and keep its last observed counts.
#
# Case B needs a credential that stops working while the process runs. Deleting
# a ServiceAccount invalidates the tokens issued to it, so the test creates a
# throwaway SA for exactly this purpose and removes it afterwards. Nothing the
# demo workload depends on is touched.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

CLUSTER=fleetforge-dev
CTX="kind-${CLUSTER}"
BIN=./target/debug/fleetforge
BAD_KUBECONFIG="infra/local/.kubeconfig-invalid-token"
PORT=${PORT:-8082}

cleanup() {
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
  rm -f "$BAD_KUBECONFIG"
  wait 2>/dev/null
}
trap cleanup EXIT

SERVER=$(kubectl config view --raw -o jsonpath="{.clusters[?(@.name==\"${CTX}\")].cluster.server}")
CA=$(kubectl config view --raw -o jsonpath="{.clusters[?(@.name==\"${CTX}\")].cluster.certificate-authority-data}")

# A token the API server will reject. Deliberately NOT shaped like a real JWT:
# an expired credential and a malformed one both come back 401, which is all
# this test needs — and a JWT-shaped literal makes every secret scanner that
# ever reads this repository report a finding it then has to be told to ignore.
INVALID_TOKEN="not-a-real-token.$(date +%s).rejected-by-design"

umask 077
cat > "$BAD_KUBECONFIG" <<EOF
apiVersion: v1
kind: Config
clusters:
  - name: ${CLUSTER}
    cluster: { server: ${SERVER}, certificate-authority-data: ${CA} }
contexts:
  - name: invalid
    context: { cluster: ${CLUSTER}, user: invalid }
current-context: invalid
users:
  - name: invalid
    user: { token: ${INVALID_TOKEN} }
EOF

echo "Starting FleetForge with a credential the API server will reject."
echo
"$BIN" --kubeconfig "$BAD_KUBECONFIG" --bind "127.0.0.1:${PORT}" >/tmp/ff-expired.log 2>&1 &
SERVER_PID=$!
sleep 12

echo "GET /api/v1/environment:"
curl -s --max-time 5 "http://127.0.0.1:${PORT}/api/v1/environment" 2>/dev/null | jq '{
  authoritative,
  api_server_reachable,
  coverage: [.coverage[] | {kind, observed_count, state: .status.state, error: .status.error}]
}' || echo "  (no snapshot yet — the process never got a usable view, which is itself correct)"

echo
echo "GET /api/v1/pdbs:"
curl -s --max-time 5 "http://127.0.0.1:${PORT}/api/v1/pdbs" 2>/dev/null \
  | jq '{code, message, authoritative, pdb_count: (.data | length?)}' \
  || echo "  (no response)"

echo
echo "Collector log:"
grep -E "watch failed|unreachable" /tmp/ff-expired.log | head -4 | sed 's/^/  /'

echo
echo "The required outcome: NOT authoritative, and never an empty list presented"
echo "as a complete one. An empty PDB list would read as 'no blockers'."

# --- Case B: the credential stops working while FleetForge is running --------

echo
echo "=============================================================="
echo "Case B: credential invalidated while the collector is running"
echo "=============================================================="

kill "$SERVER_PID" 2>/dev/null; wait 2>/dev/null; SERVER_PID=""

THROWAWAY=fleetforge-expiry-probe
NS=fleetforge-system
TMP_KUBECONFIG="infra/local/.kubeconfig-${THROWAWAY}"

cleanup_b() {
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null
  kubectl delete sa "$THROWAWAY" -n "$NS" --ignore-not-found >/dev/null 2>&1
  kubectl delete clusterrolebinding "$THROWAWAY" --ignore-not-found >/dev/null 2>&1
  rm -f "$TMP_KUBECONFIG" "$BAD_KUBECONFIG"
}
trap cleanup_b EXIT

echo "Creating throwaway ServiceAccount ${NS}/${THROWAWAY} (read-only, same role)"
kubectl create sa "$THROWAWAY" -n "$NS" >/dev/null
kubectl create clusterrolebinding "$THROWAWAY" \
  --clusterrole=fleetforge-reader \
  --serviceaccount="${NS}:${THROWAWAY}" >/dev/null

TOKEN=$(kubectl create token "$THROWAWAY" -n "$NS" --duration=1h)
umask 077
cat > "$TMP_KUBECONFIG" <<EOF
apiVersion: v1
kind: Config
clusters:
  - name: ${CLUSTER}
    cluster: { server: ${SERVER}, certificate-authority-data: ${CA} }
contexts:
  - name: probe
    context: { cluster: ${CLUSTER}, user: probe }
current-context: probe
users:
  - name: probe
    user: { token: ${TOKEN} }
EOF

"$BIN" --kubeconfig "$TMP_KUBECONFIG" --bind "127.0.0.1:${PORT}" >/tmp/ff-expiry-b.log 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 60); do
  curl -sf "http://127.0.0.1:${PORT}/readyz" >/dev/null 2>&1 && break
  sleep 0.5
done

show() {
  printf '  %-22s %s\n' "$1" "$(curl -s --max-time 5 "http://127.0.0.1:${PORT}/api/v1/environment" 2>/dev/null | jq -r '
    "authoritative=\(.authoritative) reachable=\(.api_server_reachable)  " +
    ([.coverage[] | select(.kind=="PodDisruptionBudget")][0] |
      "pdbs=\(.observed_count) status=\(.status.state)")' 2>/dev/null)"
}

show "with valid token"

echo "  >>> kubectl delete sa ${THROWAWAY}   (invalidates the issued token)"
kubectl delete sa "$THROWAWAY" -n "$NS" >/dev/null
sleep 25
show "after invalidation"

echo
echo "  collector log:"
grep -E "watch failed|unreachable|unauthorized" /tmp/ff-expiry-b.log | tail -5 | sed 's/^/    /'
