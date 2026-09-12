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
#    last-applied-configuration annotation, machineID, systemUUID, bootID, and
#    every IP address (node addresses, podIP/podIPs, hostIP/hostIPs). UIDs are
#    pseudonymised deterministically — the model needs their shape and
#    uniqueness, not their real values.
#
#    The local Kind cluster's RFC1918 addresses are not themselves sensitive.
#    They are stripped because this same script will be pointed at EKS, where
#    node addresses reveal VPC layout, and a scrub that only runs properly on
#    the harmless cluster is not a scrub.
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
        | if has("podIP") then .podIP = "10.0.0.0" else . end
        | if has("hostIP") then .hostIP = "10.0.0.0" else . end
        | if has("podIPs") then .podIPs = [] else . end
        | if has("hostIPs") then .hostIPs = [] else . end
        | if has("uid") then .uid |= pseudo_uid else . end
        | map_values(walk_scrub)
      elif type == "array" then map(walk_scrub)
      elif type == "string" then
        # Control-plane static pods carry advertise addresses in annotations and
        # container arguments, so field-by-field stripping is not enough. Redact
        # any IPv4 literal wherever it appears. Three-component version strings
        # like "v1.37.0" do not match; four-component addresses do.
        gsub("\\b(?:[0-9]{1,3}\\.){3}[0-9]{1,3}\\b"; "0.0.0.0")
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
for pattern in managedFields machineID systemUUID bootID last-applied-configuration '172\.1[6-9]\.' '172\.2[0-9]\.' '192\.168\.'; do
  # grep exits 1 when it finds nothing, which is the success case here.
  n=$(grep -ro "$pattern" "$OUT" 2>/dev/null | wc -l | tr -d ' ' || true)
  n=${n:-0}
  printf "  %-32s %s\n" "$pattern" "$n"
  if [ "$n" != "0" ]; then echo "  SCRUB FAILED for $pattern" >&2; exit 1; fi
done
echo
echo "Captured fixtures are FIXTURE data. FixtureSource stamps them at"
echo "construction; they can never be served as LIVE (ADR-0003, ADR-0020)."
