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
cd "$REPO_ROOT/infra/terraform"

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

terraform destroy

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
