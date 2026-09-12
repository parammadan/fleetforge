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

  hourly_cost = 0.10 + (var.node_count * local.hourly_instance_cost) + (local.lean ? 0 : 0.045)
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
      # Null launches the latest, which produces a cluster with nothing to do.
      ami_release_version = var.bottlerocket_release_version

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
