#!/usr/bin/env bash
# Capture live cluster objects as test fixtures, scrubbed.
#
# Two rules, both non-negotiable:
#
# 1. Anything replayed from disk is FIXTURE, regardless of where it came from
#    originally (ADR-0020). These files were captured from a real cluster; that
#    does not make them live, and FixtureSource stamps every fact accordingly.
#
# 2. Scrub before committing. Removed below: managedFields, the
#    last-applied-configuration annotation, node addresses, machineID,
#    systemUUID, bootID, and kubelet/container-runtime endpoint detail. UIDs are
#    pseudonymised deterministically — the model needs their shape and
#    uniqueness, not their real values.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$REPO_ROOT/fixtures/captured"
mkdir -p "$OUT"

scrub() {
  jq '
    def pseudo_uid:
      if type == "string" and (test("^[0-9a-f-]{36}$")) then
        "fixture-uid-" + (. | @base64 | .[0:12] | ascii_downcase | gsub("[^a-z0-9]"; ""))
      else . end;

    def walk_scrub:
      if type == "object" then
        with_entries(
          select(.key
            | IN("managedFields", "machineID", "systemUUID", "bootID") | not)
        )
        | if has("annotations") then
            .annotations |= (with_entries(
              select(.key | IN("kubectl.kubernetes.io/last-applied-configuration") | not)))
          else . end
        | if has("addresses") then .addresses = [] else . end
        | if has("uid") then .uid |= pseudo_uid else . end
        | map_values(walk_scrub)
      elif type == "array" then map(walk_scrub)
      else . end;

    walk_scrub
    | .items |= (. // [] | map(.metadata.annotations |= (. // {})))
  '
}

capture() {
  local kind=$1 file=$2
  shift 2
  echo "  ${kind} -> fixtures/captured/${file}"
  kubectl get "$kind" "$@" -o json | scrub > "${OUT}/${file}"
}

echo "Capturing from context: $(kubectl config current-context)"
capture nodes                  nodes.json
capture pods                   pods.json -A
capture deployments            deployments.json -A
capture statefulsets           statefulsets.json -A
capture daemonsets             daemonsets.json -A
capture replicasets            replicasets.json -A
capture poddisruptionbudgets   pdbs.json -A
capture events                 events.json -A

echo
echo "Scrub check — these must all report 0:"
for pattern in managedFields machineID systemUUID bootID last-applied-configuration; do
  n=$(grep -ro "$pattern" "$OUT" 2>/dev/null | wc -l | tr -d ' ')
  printf "  %-32s %s\n" "$pattern" "$n"
  [ "$n" != "0" ] && { echo "  SCRUB FAILED for $pattern" >&2; exit 1; }
done
echo
echo "Captured fixtures are FIXTURE data. FixtureSource stamps them at"
echo "construction; they can never be served as LIVE (ADR-0003, ADR-0020)."
