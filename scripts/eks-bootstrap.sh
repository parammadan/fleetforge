#!/usr/bin/env bash
# Install everything the demonstration needs onto the EKS cluster.
#
# Order matters:
#   cert-manager -> Brupop (its webhooks need certificates)
#   demo workload
#   FleetForge (read-only)
#
# Creates nothing outside the cluster. No load balancers.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

PROFILE=${AWS_PROFILE:-fleetforge}
REGION=us-east-2
CLUSTER=fleetforge-demo
BRUPOP_VERSION=v1.8.0

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

say "0/5  point kubectl at the cluster"
aws eks update-kubeconfig --region "$REGION" --name "$CLUSTER" --profile "$PROFILE" >/dev/null
kubectl config current-context
kubectl get nodes -o custom-columns=NODE:.metadata.name,STATUS:.status.conditions[-1].type,OS:.status.nodeInfo.osImage,ARCH:.status.nodeInfo.architecture --no-headers

say "1/5  verify the nodes are Bottlerocket and carry Brupop's required label"
MISSING=$(kubectl get nodes -o json | jq -r '
  .items[] | select(.metadata.labels["bottlerocket.aws/updater-interface-version"] != "2.0.0")
  | .metadata.name')
if [ -n "$MISSING" ]; then
  echo "  nodes WITHOUT bottlerocket.aws/updater-interface-version=2.0.0:" >&2
  echo "$MISSING" | sed 's/^/    /' >&2
  echo "  Brupop's agent will schedule nowhere. Stopping." >&2
  exit 1
fi
echo "  all nodes labelled correctly"

say "2/5  cert-manager (Brupop's webhooks require it)"
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/latest/download/cert-manager.yaml
kubectl -n cert-manager rollout status deploy/cert-manager --timeout=300s
kubectl -n cert-manager rollout status deploy/cert-manager-webhook --timeout=300s

say "3/5  Brupop ${BRUPOP_VERSION}"
kubectl apply -f "https://github.com/bottlerocket-os/bottlerocket-update-operator/releases/download/${BRUPOP_VERSION}/bottlerocket-update-operator-${BRUPOP_VERSION}.yaml"
kubectl -n brupop-bottlerocket-aws rollout status deploy/brupop-controller-deployment --timeout=300s || true
echo "  shadows Brupop has created (one per node):"
sleep 20
kubectl get bottlerocketshadows -A 2>&1 | sed 's/^/    /'

say "4/5  demo workload"
kubectl apply -f infra/eks/demo-workload.yaml
kubectl -n demo rollout status deploy/web --timeout=300s
kubectl -n demo rollout status deploy/api --timeout=300s
echo "  pod placement:"
kubectl get pods -n demo -o custom-columns=POD:.metadata.name,NODE:.spec.nodeName --no-headers | sed 's/^/    /'
kubectl get pdb -n demo --no-headers | sed 's/^/    /'

say "5/5  next"
cat <<'NEXT'
  Run FleetForge against the cluster:

    ./scripts/make-kubeconfig-eks.sh          # read-only ServiceAccount
    ./target/debug/fleetforge \
      --kubeconfig infra/eks/.kubeconfig-fleetforge-reader \
      --record /tmp/fleetforge-eks/events.jsonl \
      --run-description "EKS Bottlerocket demonstration"

  Then ./scripts/preflight-demo.sh, and the Brupop update.

  WHEN FINISHED:  ./scripts/eks-down.sh    (~$5.47/day until you do)
NEXT
