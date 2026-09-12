# Credits on a free-plan account are finite, and access ends when they run out.
# Surfacing the burn rate as an output means it is in front of the operator at
# plan time, not discovered in a billing console two weeks later.
output "credit_runway" {
  description = "How long a fixed credit balance lasts at this configuration's burn rate."
  value = format(
    "$120 of credits lasts ~%.1f days at ~$%.2f/day. Tear down with ./scripts/eks-down.sh.",
    120.0 / (local.hourly_cost * 24),
    local.hourly_cost * 24,
  )
}

output "cluster_name" {
  description = "EKS cluster name."
  value       = module.eks.cluster_name
}

output "region" {
  description = "Region the cluster is in."
  value       = var.region
}

output "configure_kubectl" {
  description = "Command to point kubectl at this cluster."
  value = join(" ", [
    "aws eks update-kubeconfig",
    "--region ${var.region}",
    "--name ${module.eks.cluster_name}",
    "--profile ${var.aws_profile}",
  ])
}

output "cluster_endpoint" {
  description = <<-EOT
    API server endpoint.

    Marked sensitive so it does not land in CI logs or a terminal recording.
    It is not a secret, but it identifies the cluster and there is no reason to
    print it by default (THREAT_MODEL.md R1).
  EOT
  value       = module.eks.cluster_endpoint
  sensitive   = true
}

output "node_group_ami_release" {
  description = <<-EOT
    Bottlerocket release the nodes launched with.

    Check this against the latest release before running the demonstration. If
    they match, there is no update pending and Brupop will correctly do nothing.
  EOT
  value       = var.bottlerocket_release_version
}

output "cost_breakdown_usd_per_day" {
  description = <<-EOT
    Every billed component, per day.

    Hand-entered from published us-east-2 on-demand pricing, not a live pricing
    API. `cloudwatch_logs` and `cross_az` are estimates that scale with activity;
    the rest are fixed for a running cluster.

    NOT included, because this configuration does not create them:
      - Load balancers. A Service of type LoadBalancer would add an NLB at
        ~$0.0225/hr plus LCUs (~$0.55/day). The demonstration must use
        `kubectl port-forward` instead. A load balancer created by Kubernetes is
        also NOT destroyed by `terraform destroy` — the cloud controller made
        it, so Terraform does not know it exists, and it will outlive the
        cluster and keep billing.
      - EBS snapshots, data transfer out to the internet, Route 53.
  EOT
  value       = { for k, v in local.cost : k => format("$%.3f", v * 24) }
}

output "estimated_hourly_cost_usd" {
  description = <<-EOT
    Total hourly and daily cost. See cost_breakdown_usd_per_day for components.
  EOT
  value = format(
    "~$%.3f/hr = ~$%.2f/day  [%s profile: %d x %s%s]",
    local.hourly_cost,
    local.hourly_cost * 24,
    var.cost_profile,
    var.node_count,
    local.instance_type,
    local.lean ? ", public IPs, no NAT" : ", private subnets + NAT",
  )
}
