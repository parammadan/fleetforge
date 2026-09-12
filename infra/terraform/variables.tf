variable "region" {
  description = "AWS region. Bottlerocket ARM64 AMIs must be available here."
  type        = string
  default     = "us-east-2"
}

variable "aws_profile" {
  description = <<-EOT
    Named AWS CLI profile to use.

    Deliberately has no default. FleetForge's threat model forbids secret keys
    in source or environment files, and an unset profile means the operator
    chooses explicitly rather than inheriting whichever credential happens to be
    ambient — which is how the wrong account gets an EKS cluster.
  EOT
  type        = string
}

variable "cluster_name" {
  description = "EKS cluster name."
  type        = string
  default     = "fleetforge-demo"
}

variable "kubernetes_version" {
  description = <<-EOT
    EKS control plane version.

    Verify against `aws eks describe-cluster-versions` before applying: EKS
    supports a moving window, and a version that was current when this was
    written may be unsupported by the time it runs.
  EOT
  type        = string
  default     = "1.33"
}

variable "cost_profile" {
  description = <<-EOT
    How much to spend to make this cluster well-isolated.

    "lean"     — t4g.medium workers, nodes in public subnets, no NAT gateway.
                 Roughly $5.00/day. Nodes get public IPs; inbound is still
                 restricted by security groups.
    "isolated" — m6g.large workers, nodes in private subnets behind one NAT
                 gateway. Roughly $9.20/day. The shape you would actually run.

    This matters more than usual on a free-plan account: credits are finite and
    access ends when they are gone, so the choice is not only about money but
    about how long the environment can safely be left standing.
  EOT
  type        = string
  default     = "lean"

  validation {
    condition     = contains(["lean", "isolated"], var.cost_profile)
    error_message = "cost_profile must be \"lean\" or \"isolated\"."
  }
}

variable "node_instance_type" {
  description = <<-EOT
    Override the instance type chosen by cost_profile.

    Graviton either way: Bottlerocket supports ARM64, and it is cheaper than the
    x86 equivalent for identical capacity.
  EOT
  type        = string
  default     = null
}

variable "node_count" {
  description = <<-EOT
    Number of Bottlerocket workers.

    Three is the minimum that makes the demonstration meaningful: drain one and
    two remain, so rescheduling has somewhere to go and a PodDisruptionBudget
    can be satisfied. Two would make every drain look like a capacity problem.
  EOT
  type        = number
  default     = 3

  validation {
    condition     = var.node_count >= 2 && var.node_count <= 6
    error_message = "Between 2 and 6. This is a demonstration environment, not a fleet."
  }
}

variable "bottlerocket_release_version" {
  description = <<-EOT
    Bottlerocket AMI release to launch nodes with, for example "1.19.5-82ec1d16".

    **This must be a release BEHIND current**, or Brupop has nothing to update
    and the demonstration has no subject. Find available releases with:

      aws ssm get-parameters-by-path \
        --path /aws/service/bottlerocket/aws-k8s-1.33/arm64 --recursive

    Leaving it null launches the latest, which produces a cluster with no
    pending update.
  EOT
  type        = string
  default     = null
}

variable "allowed_public_cidr" {
  description = <<-EOT
    CIDR permitted to reach the EKS public API endpoint.

    Defaults to nothing. Set it to your own address (`curl ifconfig.me`/32) —
    0.0.0.0/0 exposes the control plane endpoint to the internet, and an
    ephemeral demo cluster is not a reason to do that.
  EOT
  type        = string
  default     = null

  validation {
    condition     = var.allowed_public_cidr == null || var.allowed_public_cidr != "0.0.0.0/0"
    error_message = "Refusing 0.0.0.0/0. Use your own address, e.g. 203.0.113.4/32."
  }
}
