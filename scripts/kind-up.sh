#!/usr/bin/env bash
# Create the FleetForge local development cluster.
#
# Idempotent-ish: refuses to clobber an existing cluster. Creates nothing
# outside the `fleetforge-dev` kind cluster, and contacts no cloud provider.
set -euo pipefail

CLUSTER=fleetforge-dev
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

say "1/6  Checking the container runtime"
if ! docker info >/dev/null 2>&1; then
  echo "Docker is not reachable. Start Docker Desktop and try again." >&2
  exit 1
fi
RUNTIME_CPUS=$(docker info --format '{{.NCPU}}')
RUNTIME_MEM_MIB=$(( $(docker info --format '{{.MemTotal}}') / 1024 / 1024 ))
echo "  runtime: $(docker info --format '{{.OperatingSystem}}')"
echo "  cpus=${RUNTIME_CPUS} memory=${RUNTIME_MEM_MIB}MiB"
if [ "$RUNTIME_MEM_MIB" -gt 5120 ]; then
  echo "  note: more than ~4 GiB is allocated to the container runtime."
  echo "        On an 8 GB machine that competes with the Rust toolchain."
fi
if [ "$RUNTIME_MEM_MIB" -lt 3584 ]; then
  echo "  warning: under ~3.5 GiB. Three kind nodes may not stay healthy." >&2
fi

say "2/6  Creating the cluster"
if kind get clusters 2>/dev/null | grep -qx "$CLUSTER"; then
  echo "  cluster '$CLUSTER' already exists; leaving it alone."
else
  # Observed 2026-09-12: the first creation after the node image is pulled can
  # fail in kubeadm's wait-control-plane phase. The API server is reachable but
  # answers empty while etcd is still settling on a cold page cache, and
  # kubeadm gives up. A second attempt succeeds. Retry once rather than hand
  # the operator a scary log for a transient condition.
  if ! kind create cluster --config infra/local/kind.yaml --wait 180s; then
    echo "  first attempt failed; retrying once (see comment in this script)."
    kind delete cluster --name "$CLUSTER" >/dev/null 2>&1 || true
    kind create cluster --config infra/local/kind.yaml --wait 180s
  fi
fi
kubectl config use-context "kind-${CLUSTER}"

say "3/6  Recording the images actually pulled"
./scripts/record-images.sh

say "4/6  Applying RBAC (read-only identities, no cluster-admin)"
kubectl apply -f infra/local/rbac/fleetforge-reader.yaml
kubectl apply -f infra/local/rbac/fleetforge-restricted.yaml

say "5/6  Applying the demo workload"
kubectl apply -f infra/local/demo/demo-namespace.yaml
kubectl -n demo rollout status deploy/web --timeout=180s
kubectl -n demo rollout status deploy/api --timeout=180s

say "6/6  Verifying FleetForge's identity cannot mutate anything"
./scripts/mutation-denial-test.sh

say "Done."
echo "  context:      kind-${CLUSTER}"
echo "  kubeconfig:   ./scripts/make-kubeconfig.sh fleetforge-reader"
echo "  tear down:    ./scripts/kind-down.sh   (never runs automatically)"
