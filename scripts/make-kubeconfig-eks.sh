#!/usr/bin/env bash
# Create FleetForge's read-only identity on the EKS cluster and a kubeconfig
# that uses it.
#
# Same discipline as the local cluster: three verbs, no secrets, no mutating
# permission anywhere, short-lived token (ADR-0019). FleetForge never runs with
# the cluster-admin credentials Terraform used.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

SA=${1:-fleetforge-reader}
DURATION=${2:-4h}
NS=fleetforge-system
REGION=us-east-2
CLUSTER=fleetforge-demo
OUT="infra/eks/.kubeconfig-${SA}"

case "$SA" in
  fleetforge-reader|fleetforge-restricted) ;;
  *) echo "Unknown ServiceAccount: $SA" >&2; exit 1 ;;
esac

# The same RBAC as the local cluster, plus Brupop's CRs. Reused verbatim so the
# two environments cannot drift.
kubectl apply -f infra/local/rbac/fleetforge-reader.yaml >/dev/null
kubectl apply -f infra/local/rbac/fleetforge-restricted.yaml >/dev/null

echo "Proving the identity cannot mutate anything:"
./scripts/mutation-denial-test.sh || { echo "mutation-denial test FAILED" >&2; exit 1; }

SERVER=$(aws eks describe-cluster --name "$CLUSTER" --region "$REGION" \
  --query 'cluster.endpoint' --output text)
CA=$(aws eks describe-cluster --name "$CLUSTER" --region "$REGION" \
  --query 'cluster.certificateAuthority.data' --output text)
TOKEN=$(kubectl create token "$SA" -n "$NS" --duration="$DURATION")

mkdir -p infra/eks
umask 077
cat > "$OUT" <<EOF
apiVersion: v1
kind: Config
clusters:
  - name: ${CLUSTER}
    cluster: { server: ${SERVER}, certificate-authority-data: ${CA} }
contexts:
  - name: ${SA}
    context: { cluster: ${CLUSTER}, user: ${SA} }
current-context: ${SA}
users:
  - name: ${SA}
    user: { token: ${TOKEN} }
EOF

echo
echo "Wrote ${OUT}  (gitignored)"
echo "  ServiceAccount: ${SA}"
echo "  Token duration: ${DURATION} — expiry becomes Unauthorized, never an empty cluster"
