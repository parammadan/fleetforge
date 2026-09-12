#!/usr/bin/env bash
# Build a kubeconfig for a FleetForge ServiceAccount using a short-lived token.
#
# Usage: ./scripts/make-kubeconfig.sh [fleetforge-reader|fleetforge-restricted] [duration]
#
# Uses the TokenRequest API, not a long-lived Secret token. The credential
# expires, and expiry is a first-class state in FleetForge: an expired token
# produces Unauthorized / non-authoritative, never an empty cluster and never a
# "safe" result (ADR-0019).
#
# The generated file is gitignored. Nothing here is ever committed.
set -euo pipefail

SA="${1:-fleetforge-reader}"
DURATION="${2:-2h}"
NS=fleetforge-system
CLUSTER=fleetforge-dev
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$REPO_ROOT/infra/local/.kubeconfig-${SA}"

case "$SA" in
  fleetforge-reader|fleetforge-restricted) ;;
  *) echo "Unknown ServiceAccount: $SA" >&2; exit 1 ;;
esac

CTX="kind-${CLUSTER}"
SERVER=$(kubectl config view --raw -o jsonpath="{.clusters[?(@.name==\"${CTX}\")].cluster.server}")
CA=$(kubectl config view --raw -o jsonpath="{.clusters[?(@.name==\"${CTX}\")].cluster.certificate-authority-data}")
if [ -z "$SERVER" ] || [ -z "$CA" ]; then
  echo "Could not read cluster '${CTX}' from your kubeconfig. Is the cluster up?" >&2
  exit 1
fi

TOKEN=$(kubectl create token "$SA" -n "$NS" --duration="$DURATION")

umask 077
cat > "$OUT" <<EOF
apiVersion: v1
kind: Config
clusters:
  - name: ${CLUSTER}
    cluster:
      server: ${SERVER}
      certificate-authority-data: ${CA}
contexts:
  - name: ${SA}
    context:
      cluster: ${CLUSTER}
      user: ${SA}
current-context: ${SA}
users:
  - name: ${SA}
    user:
      token: ${TOKEN}
EOF

EXPIRES=$(date -u -v "+${DURATION}" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo "in ${DURATION}")
echo "Wrote ${OUT}"
echo "  ServiceAccount: ${SA}"
echo "  Expires:        ~${EXPIRES}"
echo
echo "The token is short-lived on purpose. When it expires FleetForge must show"
echo "Unauthorized and mark its data non-authoritative — not an empty cluster."
echo
echo "  export KUBECONFIG=${OUT}"
