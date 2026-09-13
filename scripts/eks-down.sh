#!/usr/bin/env bash
# Destroy the FleetForge EKS demonstration environment.
#
# Requires explicit confirmation. Nothing in this repository calls it
# automatically, and nothing ever should.
#
# On a free-plan AWS account this script is not a tidiness measure. Credits are
# finite and account access ends when they are gone, so a cluster left standing
# is not an expensive mistake — it is a terminal one. The estimated runway is
# printed below so the cost of *not* running this is visible.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Overridable so the regression test can point this at a stub. Nothing else
# should set it.
TF_DIR="${FLEETFORGE_TF_DIR:-$REPO_ROOT/infra/terraform}"
cd "$TF_DIR"

say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

if [ ! -f terraform.tfstate ] && [ ! -d .terraform ]; then
  echo "No Terraform state here. Nothing to destroy."
  exit 0
fi

say "Artifacts — confirm these are captured BEFORE destroying"
printf '%s\n' "------------------------------------------------------------"
cat <<'CHECK'
  [ ] Screen recording of the demonstration
  [ ] Event log        (kubectl cp the pod's events.jsonl, or ./out/events.jsonl)
  [ ] Evidence report  (curl /api/v1/report?format=markdown > report.md)
  [ ] Prediction-versus-actual comparison (inside the report)
  [ ] infra/local/IMAGES.md equivalent for EKS — node AMI release actually used

The cluster is disposable. These are not. Once this runs they are unrecoverable
without paying to rebuild the environment (ADR-0017).
CHECK

say "What will be destroyed"
printf '%s\n' "------------------------------------------------------------"
terraform state list 2>/dev/null | sed 's/^/  /' | head -40
COUNT=$(terraform state list 2>/dev/null | wc -l | tr -d ' ')
echo "  ... ${COUNT} resources in total"

say "Confirm"
printf '%s\n' "------------------------------------------------------------"
printf 'Type "destroy fleetforge-demo" to proceed: '
read -r reply
if [ "$reply" != "destroy fleetforge-demo" ]; then
  echo "Not confirmed. Nothing destroyed."
  exit 1
fi

# Everything below exists because of one observed failure. On 2026-09-13 this
# script printed its banner, ran, and exited 0 — having destroyed nothing. The
# piped confirmation answered the script's own prompt, Terraform asked for its
# own approval, got EOF, and aborted. A teardown script that can silently no-op
# while reporting success is worse than no script: it converts "I tore it down"
# into a belief rather than a fact, and the cluster keeps billing.
#
# So: -auto-approve (the gate above is the confirmation), and three checks.

BEFORE=$(terraform state list 2>/dev/null | grep -c . || true)
echo "  resources in state before destroy: ${BEFORE}"

set +e
terraform destroy -auto-approve
TF_RC=$?
set -e

# Check 1 — Terraform's own exit code. EOF on a prompt lands here.
if [ "$TF_RC" -ne 0 ]; then
  echo >&2
  echo "TEARDOWN FAILED: terraform destroy exited ${TF_RC}." >&2
  echo "Resources may still exist and may still be billing. Investigate before" >&2
  echo "assuming anything was removed." >&2
  exit "$TF_RC"
fi

# grep -c, not wc -l: wc counts newlines, so a final line without one is
# invisible and a surviving resource reads as zero. Caught by the
# regression test in scripts/tests/.
AFTER=$(terraform state list 2>/dev/null | grep -c . || true)
echo "  resources in state after destroy:  ${AFTER}"

# Check 2 — state must be empty afterwards.
if [ "$AFTER" -ne 0 ]; then
  echo >&2
  echo "TEARDOWN FAILED: ${AFTER} resource(s) remain in Terraform state." >&2
  terraform state list >&2
  exit 1
fi

# Check 3 — if there was something to destroy, something must have been
# destroyed. Catches a silent no-op that still exits 0.
if [ "$BEFORE" -gt 0 ] && [ "$BEFORE" -eq "$AFTER" ]; then
  echo >&2
  echo "TEARDOWN FAILED: state had ${BEFORE} resources before and after." >&2
  echo "Nothing was destroyed despite a successful exit code." >&2
  exit 1
fi

echo "  verified: ${BEFORE} resource(s) destroyed, state is empty"

say "Verify nothing is left billing"
printf '%s\n' "------------------------------------------------------------"
cat <<'VERIFY'
  Terraform destroys what it created. Check for anything it did not own:

    aws eks list-clusters --profile fleetforge
    aws ec2 describe-instances --profile fleetforge \
      --filters Name=tag:Project,Values=FleetForge \
      --query 'Reservations[].Instances[?State.Name!=`terminated`].InstanceId'
    aws ec2 describe-nat-gateways --profile fleetforge \
      --filter Name=tag:Project,Values=FleetForge \
      --query 'NatGateways[?State!=`deleted`].NatGatewayId'
    aws ec2 describe-volumes --profile fleetforge \
      --filters Name=tag:Project,Values=FleetForge --query 'Volumes[].VolumeId'

  Load balancers created by Kubernetes Services are the classic survivor: they
  are made by the cloud controller, not by Terraform, so Terraform does not know
  to remove them and the VPC delete can hang on them.
VERIFY

# --- Expected duration -------------------------------------------------------
#
# Observed AWS behaviour, not a guess at the total:
#   node group delete   ~4-6 min   (drains and terminates instances)
#   EKS control plane   ~8-11 min  (the long pole)
#   VPC and subnets     ~1-2 min   (blocks until ENIs are released)
#   TOTAL               ~15-20 min
#
# The VPC delete is what hangs when something outside Terraform is still using
# it — most often a load balancer created by a Kubernetes Service. Delete those
# Services BEFORE running this, or `terraform destroy` will stall for ~20
# minutes on DependencyViolation and then fail with the cluster half gone.
