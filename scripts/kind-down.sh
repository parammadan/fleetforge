#!/usr/bin/env bash
# Delete the FleetForge local development cluster.
#
# Requires explicit confirmation. Nothing in this repository ever calls it
# automatically.
set -euo pipefail
CLUSTER=fleetforge-dev

if ! kind get clusters 2>/dev/null | grep -qx "$CLUSTER"; then
  echo "Cluster '$CLUSTER' does not exist. Nothing to do."
  exit 0
fi

echo "This deletes the kind cluster '$CLUSTER' and everything in it."
printf "Type the cluster name to confirm: "
read -r reply
if [ "$reply" != "$CLUSTER" ]; then
  echo "Not confirmed. Nothing deleted."
  exit 1
fi

kind delete cluster --name "$CLUSTER"
rm -f infra/local/.kubeconfig-fleetforge-*
echo "Deleted, and local kubeconfigs removed."
