#!/usr/bin/env bash
# Prove that FleetForge's identity cannot change the cluster.
#
# Asks the live API server, as the fleetforge-reader ServiceAccount, whether it
# may perform each mutating verb on each watched resource. Every answer must be
# "no". A YAML file expressing intent is not evidence; this is.
#
# Uses `kubectl auth can-i --quiet`, which prints nothing and communicates
# solely through its exit status: 0 = allowed, non-zero = denied. The non-quiet
# form writes "no" to stdout AND exits non-zero, which makes it very easy to
# write a check that reports failure for every correctly-denied verb.
#
# Exits non-zero on any violation, so it works as a test.
set -uo pipefail

SA="system:serviceaccount:fleetforge-system:fleetforge-reader"
MUTATING_VERBS=(create update patch delete deletecollection)
MUTABLE_RESOURCES=(
  nodes pods namespaces events
  deployments.apps statefulsets.apps daemonsets.apps replicasets.apps
  poddisruptionbudgets.policy
  pods/eviction
  secrets configmaps
)
REQUIRED_READS=(
  nodes pods namespaces events
  deployments.apps statefulsets.apps daemonsets.apps replicasets.apps
  poddisruptionbudgets.policy
)

fail=0

# allowed <verb> <resource> -> 0 if the API server permits it
allowed() {
  kubectl auth can-i "$1" "$2" --all-namespaces --as="$SA" --quiet >/dev/null 2>&1
}

echo "  Mutating verbs must all be denied:"
denied_count=0
for res in "${MUTABLE_RESOURCES[@]}"; do
  for verb in "${MUTATING_VERBS[@]}"; do
    if allowed "$verb" "$res"; then
      printf "    \033[31mFAIL\033[0m %s %s is PERMITTED\n" "$verb" "$res"
      fail=1
    else
      denied_count=$((denied_count + 1))
    fi
  done
done
[ "$fail" -eq 0 ] && echo "    ${denied_count} mutating verb/resource pairs denied"

echo "  Secrets and configmaps must be unreadable:"
secret_denied=0
for res in secrets configmaps; do
  for verb in get list watch; do
    if allowed "$verb" "$res"; then
      printf "    \033[31mFAIL\033[0m %s %s is PERMITTED\n" "$verb" "$res"
      fail=1
    else
      secret_denied=$((secret_denied + 1))
    fi
  done
done
[ "$fail" -eq 0 ] && echo "    ${secret_denied} secret/configmap reads denied"

echo "  Reads the collector needs must be allowed:"
read_ok=0
for res in "${REQUIRED_READS[@]}"; do
  for verb in get list watch; do
    if allowed "$verb" "$res"; then
      read_ok=$((read_ok + 1))
    else
      printf "    \033[31mFAIL\033[0m %s %s is DENIED (collector cannot work)\n" "$verb" "$res"
      fail=1
    fi
  done
done
[ "$fail" -eq 0 ] && echo "    ${read_ok} required reads allowed"

if [ "$fail" -ne 0 ]; then
  echo "  MUTATION DENIAL TEST FAILED" >&2
  exit 1
fi
echo "  mutation-denial test passed"
