# The FleetForge demonstration environment

Terraform for a deliberately small, deliberately temporary EKS cluster with
Bottlerocket workers. It exists to capture the Milestone 5 demonstration and is
destroyed afterwards (ADR-0017).

**Nothing here has been applied.** As of 2026-09-12 this configuration has been
written, formatted, initialised, and validated. `terraform plan` reached 30
resources before stopping at the first AWS API call — see *Status* below.

## What it creates

| | |
| --- | --- |
| VPC | `10.42.0.0/16`, two AZs, public + private subnets, **one** NAT gateway |
| EKS | Control plane, public endpoint locked to a single CIDR |
| Node group | 3 × `m6g.large`, `BOTTLEROCKET_ARM_64`, fixed size (no autoscaling) |
| Node label | `bottlerocket.aws/updater-interface-version=2.0.0` — see below |
| Tags | `Project=FleetForge`, `Environment=dev`, `ManagedBy=Terraform`, `Owner=Param` on everything, set at the provider |

### Three decisions worth knowing about

**The node label is load-bearing.** Brupop's agent DaemonSet has a required node
affinity on `bottlerocket.aws/updater-interface-version In [2.0.0]`. Without that
label the agent schedules nowhere, Brupop does nothing, and the failure is
completely silent — no error, just an operator watching an update that never
starts. Verified against the v1.8.0 manifest and set in the node group labels.

**The node group does not autoscale.** `min = max = desired`. A cluster that
quietly adds capacity mid-drain would invalidate the capacity analysis the
demonstration exists to show.

**The AMI release should be pinned behind current.** `bottlerocket_release_version`
defaults to null, which launches the latest — and produces a cluster with no
pending update, so Brupop correctly does nothing and there is nothing to
demonstrate. Find releases with:

```bash
aws ssm get-parameters-by-path \
  --path /aws/service/bottlerocket/aws-k8s-1.33/arm64 --recursive \
  --query 'Parameters[].Name' --profile <profile>
```

## Cost profiles

Set `cost_profile`. Both were computed from the configuration itself
(`terraform console`), not estimated by hand:

| Profile | Workers | Network | Hourly | Daily | $120 of credits lasts |
| --- | --- | --- | --- | --- | --- |
| `lean` (default) | 3 × `t4g.medium` | Public subnets, **no NAT** | $0.2008 | **$4.82** | **24.9 days** |
| `isolated` | 3 × `m6g.large` | Private subnets, 1 NAT gateway | $0.3760 | **$9.02** | **13.3 days** |

`lean` gives nodes public IPs. Inbound is still closed by security groups; what
is given up is the second layer, not the first. For a cluster that lives for two
days that is a defensible trade — and the dollar a day it saves buys a day of
not having to hurry the teardown.

`isolated` is the shape you would actually run in production, and is the right
choice if the demonstration is meant to look like production.

### Why this matters more than usual here

On an AWS free-plan account, credits are finite and **account access ends when
they are depleted**. A forgotten cluster is not an expensive mistake; it is a
terminal one. That is why the burn rate is a Terraform *output* — visible at plan
time — rather than a figure in a document nobody re-reads.

## Cost

Hand-calculated from published `us-east-2` on-demand pricing, **not** from a live
pricing API. Verify against the AWS Pricing Calculator before relying on it.

| Item | Hourly | Daily |
| --- | --- | --- |
| EKS control plane | $0.10 | $2.40 |
| 3 × `m6g.large` on-demand | $0.231 | $5.54 |
| 1 NAT gateway (excl. data) | $0.045 | $1.08 |
| EBS, 3 × 24 GiB gp3 | ~$0.007 | ~$0.17 |
| **Total** | **~$0.38** | **~$9.20** |

Two to three days of demonstration capture is roughly **$20–28**. Leaving it
running for a month is roughly **$280**, which is the entire argument for
ADR-0017.

Excludes data transfer and EBS snapshots. The NAT gateway also bills per GB
processed; image pulls for a three-node cluster are on the order of a few GB.

### Where the cost could be cut, and why it is not

- **No NAT gateway**, putting nodes in public subnets, saves ~$1/day. Rejected:
  it gives every node a public IP to save the price of a coffee.
- **Two nodes instead of three** saves ~$1.85/day. Rejected: draining one of two
  leaves a single node, so every drain looks like a capacity problem and the
  analyzers have nothing interesting to say.
- **Spot instances** would save roughly 70%. Rejected for the demonstration
  itself: a spot interruption mid-drain would be indistinguishable from the
  maintenance in the recording. Worth revisiting when Spot interruption analysis
  is the thing being demonstrated.

## Status

Verified so far, on 2026-09-12:

```
terraform fmt -check     clean
terraform init           providers and modules resolved
terraform validate       Success! The configuration is valid.
terraform plan           Plan: 30 to add, 0 to change, 0 to destroy  (PARTIAL)
                         then: UnauthorizedOperation on ec2:DescribeAvailabilityZones
```

The plan is **partial and the resource count is a floor**: it stopped at the
first AWS call, so everything downstream of the availability-zone lookup is
unresolved.

### The blocker

The only credential configured on this machine is an IAM user,
`arn:aws:iam::771965334314:user/pennydata-sink`, which is scoped to S3 and
nothing else. A read-only probe:

```
eks:ListClusters                DENIED
ec2:DescribeVpcs                DENIED
ec2:DescribeAvailabilityZones   DENIED
iam:ListAttachedUserPolicies    DENIED
s3:ListBuckets                  allowed
sts:GetCallerIdentity           allowed
```

That credential belongs to a different project and should not be used here even
if it were permitted to work. What is needed is a separate profile — SSO for
preference — with authority to create VPC, EKS, EC2, and IAM resources.

## Running it, when there is a credential

```bash
cp terraform.tfvars.example terraform.tfvars   # gitignored
$EDITOR terraform.tfvars                       # profile, your CIDR, AMI release

terraform init
terraform plan -out=demo.tfplan                # review, in full
terraform apply demo.tfplan                    # only after explicit approval

$(terraform output -raw configure_kubectl)
```

Teardown is documented in `DEMO.md` and run by hand. Nothing in this repository
calls `terraform destroy`.

## OpenTofu

This configuration uses no Terraform-specific syntax and runs unchanged under
OpenTofu (`tofu init`, `tofu plan`). Terraform moved to the Business Source
License in 2023 and is no longer open source; FleetForge is Apache-2.0
(ADR-0010). The brief specified Terraform, so that is what is installed and
tested — but the door is deliberately left open, and it costs nothing to keep it
that way.
