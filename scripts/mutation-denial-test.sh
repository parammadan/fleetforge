#!/usr/bin/env bash
# Prove that FleetForge's identity cannot change the cluster.
#
# This asks the live API server, as the fleetforge-reader ServiceAccount,
# whether it may perform each mutating verb on each watched resource. Every
# answer must be "no". A YAML file expressing intent is not evidence; this is.
#
# Exits non-zero on any permitted mutation, so it works as a test.
set -euo pipefail

SA="system:serviceaccount:fleetforge-system:fleetforge-reader"
RESOURCES=(
  "nodes" "pods" "namespaces" "events"
  "deployments.apps" "statefulsets.apps" "daemonsets.apps" "replicasets.apps"
  "poddisruptionbudgets.policy"
  "pods/eviction"
  "secrets" "configmaps"
)
MUTATING_VERBS=(create update patch delete deletecollection)
READ_VERBS=(get list watch)

fail=0

echo "  Mutating verbs as ${SA} — every answer must be 'no':"
for res in "${RESOURCES[@]}"; do
  for verb in "${MUTATING_VERBS[@]}"; do
    ans=$(kubectl auth can-i "$verb" "$res" --all-namespaces --as="$SA" 2>/dev/null || echo "no")
    if [ "$ans" != "no" ]; then
      printf "    \033[31mFAIL\033[0m %s %s -> %s\n" "$verb" "$res" "$ans"
      fail=1
    fi
  done
done
[ "$fail" -eq 0 ] && echo "    all mutating verbs denied"

echo "  Secrets and configmaps must be unreadable too:"
for res in secrets configmaps; do
  for verb in "${READ_VERBS[@]}"; do
    ans=$(kubectl auth can-i "$verb" "$res" --all-namespaces --as="$SA" 2>/dev/null || echo "no")
    if [ "$ans" != "no" ]; then
      printf "    \033[31mFAIL\033[0m %s %s -> %s\n" "$verb" "$res" "$ans"
      fail=1
    fi
  done
done
[ "$fail" -eq 0 ] && echo "    secrets and configmaps denied"

echo "  Reads FleetForge does need must be allowed:"
for res in nodes pods poddisruptionbudgets.policy deployments.apps; do
  ans=$(kubectl auth can-i list "$res" --all-namespaces --as="$SA" 2>/dev/null || echo "no")
  if [ "$ans" != "yes" ]; then
    printf "    \033[31mFAIL\033[0m list %s -> %s (collector cannot work)\n" "$res" "$ans"
    fail=1
  fi
done
[ "$fail" -eq 0 ] && echo "    required reads allowed"

if [ "$fail" -ne 0 ]; then
  echo "  MUTATION DENIAL TEST FAILED" >&2
  exit 1
fi
echo "  mutation-denial test passed"
