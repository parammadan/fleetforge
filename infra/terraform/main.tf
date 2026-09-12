# FleetForge demonstration environment.
#
# Deliberately small and deliberately temporary (ADR-0017): it exists to capture
# the demonstration and is destroyed afterwards. The artifacts — the recording,
# the event log, the evidence report — are the deliverable; the cluster is not.
#
# Nothing here is applied without explicit approval of a shown plan.

data "aws_availability_zones" "available" {
  state = "available"

  filter {
    name   = "opt-in-status"
    values = ["opt-in-not-required"]
  }
}

locals {
  # The lean profile trades network isolation for credit runway. On a free-plan
  # account that is a defensible trade for a cluster measured in days: a NAT
  # gateway is about $1/day of defence-in-depth, and the same dollar buys a day
  # of not having to hurry the teardown.
  lean = var.cost_profile == "lean"

  instance_type = coalesce(
    var.node_instance_type,
    local.lean ? "t4g.medium" : "m6g.large",
  )

  # Estimated from published us-east-2 on-demand pricing, not a live API.
  hourly_instance_cost = local.lean ? 0.0336 : 0.077

  # Two AZs, not three. Enough for the availability-zone analyzer to have
  # something real to reason about, without a third NAT gateway's worth of cost
  # for a cluster that lives for days.
  azs = slice(data.aws_availability_zones.available.names, 0, 2)

  # A private VPC range unlikely to collide with anything the operator already
  # has, so peering or a VPN never needs thinking about.
  vpc_cidr = "10.42.0.0/16"

  # Full hourly cost. Published us-east-2 on-demand pricing, hand-entered, NOT
  # from a live pricing API — verify against the AWS Pricing Calculator.
  #
  # The first version of this counted only the control plane, the instances, and
  # the NAT gateway, and was 13% low. The items below it missed are exactly the
  # ones that are easy to miss: they are small individually, attached to
  # resources you did not consciously choose, and invisible until the bill.
  cost = {
    # EKS control plane. $0.60/hr instead of $0.10 on a version past standard
    # support — see the kubernetes_version variable.
    control_plane = 0.10

    instances = var.node_count * local.hourly_instance_cost

    # Charged since Feb 2024, per address, whether or not it is used. Only
    # applies to the lean profile, where nodes sit in public subnets.
    public_ipv4 = local.lean ? var.node_count * 0.005 : 0

    nat_gateway = local.lean ? 0 : 0.045

    # Bottlerocket uses two volumes per node: a small OS volume and a data
    # volume. gp3 at $0.08/GiB-month.
    ebs = var.node_count * (4 + 20) * 0.08 / 730

    # One customer-managed key for EKS secrets encryption, $1/month.
    kms = 1.0 / 730

    # Control-plane logs at $0.50/GB ingested. An idle three-node cluster with
    # `audit` disabled produces roughly 100 MB/day; this is the least certain
    # line here and scales with cluster activity.
    cloudwatch_logs = 0.10 * 0.50 / 24

    # Cross-AZ traffic at $0.01/GB each way, nodes spread over two zones.
    # A guess for a demonstration workload, and a small one.
    cross_az = 1.0 * 0.02 / 24
  }

  hourly_cost = sum(values(local.cost))
}

module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "~> 6.0"

  name = "${var.cluster_name}-vpc"
  cidr = local.vpc_cidr
  azs  = local.azs

  private_subnets = [for i, _ in local.azs : cidrsubnet(local.vpc_cidr, 8, i)]
  public_subnets  = [for i, _ in local.azs : cidrsubnet(local.vpc_cidr, 8, i + 100)]

  # Under the isolated profile, nodes sit in private subnets and reach the
  # internet through a single NAT gateway — one rather than one per AZ, because
  # the availability that buys does not matter for a cluster that lives for a
  # long afternoon.
  #
  # Under the lean profile there is no NAT at all and nodes sit in public
  # subnets with public IPs. Inbound is still closed by security groups; what is
  # given up is the second layer, not the first.
  enable_nat_gateway = !local.lean
  single_nat_gateway = !local.lean

  map_public_ip_on_launch = local.lean

  enable_dns_hostnames = true
  enable_dns_support   = true

  # EKS finds subnets by these tags.
  public_subnet_tags = {
    "kubernetes.io/role/elb" = 1
  }
  private_subnet_tags = {
    "kubernetes.io/role/internal-elb" = 1
  }
}

module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 21.0"

  # Control-plane logging. The module defaults to api + audit + authenticator
  # with 90-day retention, which is wrong for this environment in both
  # directions: `audit` is by far the highest-volume stream and FleetForge does
  # not read it, and 90 days of retention on a cluster that lives for three is
  # paying to store logs for an environment that no longer exists.
  #
  # `api` and `authenticator` are kept because they are what you actually want
  # when an operator says "FleetForge could not see the PodDisruptionBudgets".
  enabled_log_types                      = ["api", "authenticator"]
  cloudwatch_log_group_retention_in_days = 3

  # EKS module v21 dropped the `cluster_` prefix from these arguments.
  name               = var.cluster_name
  kubernetes_version = var.kubernetes_version

  vpc_id     = module.vpc.vpc_id
  subnet_ids = local.lean ? module.vpc.public_subnets : module.vpc.private_subnets

  # The public endpoint is what the operator's laptop talks to. It is locked to
  # a single address: see the `allowed_public_cidr` variable, which refuses
  # 0.0.0.0/0 outright.
  endpoint_public_access       = true
  endpoint_public_access_cidrs = compact([var.allowed_public_cidr])
  endpoint_private_access      = true

  # The identity running `terraform apply` gets cluster-admin, so the operator
  # can install Brupop and run the demonstration. FleetForge itself does NOT use
  # this: it authenticates as the read-only ServiceAccount created by
  # deploy/helm (ADR-0008, ADR-0012).
  enable_cluster_creator_admin_permissions = true

  eks_managed_node_groups = {
    bottlerocket = {
      # ARM64 Bottlerocket. This is the whole point of the environment: Brupop
      # manages Bottlerocket hosts and nothing else.
      ami_type       = "BOTTLEROCKET_ARM_64"
      instance_types = [local.instance_type]
      capacity_type  = "ON_DEMAND"

      # Fixed size. No autoscaling, because a cluster that quietly adds capacity
      # during a drain would invalidate the capacity analysis the demonstration
      # is meant to show.
      min_size     = var.node_count
      max_size     = var.node_count
      desired_size = var.node_count

      # Pin to a release behind current so Brupop has a real update to perform.
      #
      # Both arguments are required. The module resolves the release version as
      #   use_latest_ami_release_version ? <latest from SSM> : ami_release_version
      # and it defaults to true, so setting `ami_release_version` alone is
      # silently discarded and nodes launch on the newest AMI. That failure is
      # invisible until the demonstration: Brupop correctly finds nothing to
      # update, and the run has no subject. Caught by reading the plan JSON
      # rather than trusting the configuration (eks module v21, main.tf:480).
      use_latest_ami_release_version = var.bottlerocket_release_version == null
      ami_release_version            = var.bottlerocket_release_version

      labels = {
        # Brupop's agent DaemonSet requires this exact label. Its node affinity
        # is `bottlerocket.aws/updater-interface-version In [2.0.0]`, so without
        # it the agent schedules nowhere, Brupop does nothing, and the failure is
        # silent — no error, just an operator watching an update that never
        # starts. Verified against the v1.8.0 manifest.
        "bottlerocket.aws/updater-interface-version" = "2.0.0"
      }

      # Bottlerocket splits storage: a small read-only OS volume and a separate
      # data volume for containers.
      block_device_mappings = {
        os = {
          device_name = "/dev/xvda"
          ebs = {
            volume_size = 4
            volume_type = "gp3"
          }
        }
        data = {
          device_name = "/dev/xvdb"
          ebs = {
            volume_size = 20
            volume_type = "gp3"
          }
        }
      }

      # Brupop's agent needs to read and update its own BottlerocketShadow, and
      # the API server component needs to talk to the control plane. Those come
      # from the Brupop install, not from the node IAM role; the role here is
      # the standard EKS worker set added by the module.
      iam_role_additional_policies = {
        # Required so Brupop's agent can reach the SSM-published Bottlerocket
        # update metadata, and so the operator can shell into a node if the
        # demonstration goes sideways — Bottlerocket has no SSH.
        AmazonSSMManagedInstanceCore = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
      }

      tags = {
        Purpose = "fleetforge-demonstration"
      }
    }
  }
}
