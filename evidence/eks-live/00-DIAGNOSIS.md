# Phase C — cross-node pod networking diagnosis

**Cluster:** `fleetforge-demo`, us-east-2, created 2026-09-13T22:24Z
**Investigation:** read-only. No security group, route table, NACL, Terraform or
AWS resource was modified.

**This is new Phase-C evidence. `evidence/eks-recovery/` was not touched.**

---

## Result

**Root cause: the EKS node shared security group has no self-referencing
inbound rule covering TCP port 80.**

Its self-referencing rules cover `tcp 1025-65535`, `tcp 53` and `udp 53`.
Port 80 — which the demo `web` workload listens on — falls outside all three.
Cross-node packets to :80 are dropped by the security group. Same-node traffic
never traverses an ENI, so it is unaffected.

## The proof

Same source pod, same destination hosts, same moment; only the port differs:

| Destination | Port | Same node | Elapsed | Outcome |
| --- | --- | --- | --- | --- |
| 10.42.100.124 | 80 | no | 3.8s | **TIMEOUT** — dropped |
| 10.42.100.124 | 8080 | no | 0.8s | **REFUSED** — RST, packet arrived |
| 10.42.101.170 | 80 | no | 3.7s | **TIMEOUT** — dropped |
| 10.42.101.170 | 8080 | no | 0.8s | **REFUSED** — RST, packet arrived |
| 10.42.101.111 | 80 | **yes** | 0.8s | **CONNECTED** |
| 10.42.101.111 | 8080 | yes | 1.0s | REFUSED |

Nothing listens on 8080 anywhere. An instant `connection refused` means the SYN
reached the destination host's TCP stack and was rejected by it — so routing,
NACLs, the CNI data path and pod IP assignment all work. A 3.8s timeout on the
same host at port 80 means the packet was silently discarded in transit.

That is a security group, and it is the only layer that filters by port while
leaving the path otherwise intact.

## Causal table

| Layer | Evidence | Verdict |
| --- | --- | --- |
| CNI initialization | `vpc-cni` ACTIVE before compute (`before_compute = true`); `aws-node` 2/2 Running, 0 restarts on all 3 nodes; addon v1.23.1-eksbuild.1 | **Ruled out** |
| Pod IP assignment | Every pod has a VPC IP in its node's subnet; secondary ENIs `aws-K8S-i-*` present and attached on all 3 instances | **Healthy** |
| Security groups | Self-referencing ingress covers `tcp 1025-65535`, `tcp/udp 53` only. No rule for `tcp 80`. Port 8080 (inside range) reaches the host; port 80 does not | **ROOT CAUSE** |
| Routing | Two nodes (`ip-10-42-101-164`, `ip-10-42-101-82`) share subnet `10.42.101.0/24` and still fail at :80 but succeed at :8080 — the VPC-local route is working | **Healthy** |
| NACL | Intra-subnet traffic does not traverse a NACL, and the same-subnet pair fails identically to the cross-subnet pair. Port 8080 crosses subnets successfully | **Healthy** |
| NetworkPolicy / CNI policy | `kubectl get networkpolicy -A` → none. No `ENIConfig`. `ENABLE_POD_ENI=false`, no custom networking, no SG-for-pods | **Healthy** |

## Why the old hypothesis is refuted

`evidence/eks-recovery/` carries an `UNVERIFIED` claim that the M5 run's 30.2%
traffic result was caused by add-on ordering — the VPC CNI installed after the
nodes had joined. This cluster was built with the correction already in place
(`vpc-cni { before_compute = true }`), the CNI was ACTIVE before any node
registered, and cross-node traffic to :80 fails **identically**.

The ordering hypothesis is refuted for this run. The 1-in-3 *shape* it
predicted is confirmed — a 3-endpoint ClusterIP Service returns roughly 33%
when only the co-located endpoint answers — but the cause is the security
group, not add-on ordering.

**The original replay's conclusions are unchanged.** That run had its own
evidence and its own caveats; this is a different cluster on a different day.
What this establishes is that the Terraform correction described there as
"implemented but not verified on a fresh cluster" **does not fix cross-node pod
networking**, because that was never the fault.

## Where it comes from

`terraform-aws-modules/eks` v21 ships `ingress_nodes_ephemeral` (tcp
1025-65535) as a built-in default in `node_groups.tf:112`. It does not open
application ports, by design. `infra/terraform/main.tf` declares no
`node_security_group_additional_rules`, so nothing ever added port 80.

This gap was present during the M5 run as well.

## Minimum proposed correction

**Narrowest fix** — one self-referencing ingress rule for the application port:

```hcl
node_security_group_additional_rules = {
  ingress_self_http = {
    description = "Node to node, demo workload HTTP"
    protocol    = "tcp"
    from_port   = 80
    to_port     = 80
    type        = "ingress"
    self        = true
  }
}
```

**Conventional EKS fix** — allow all node-to-node traffic, which is what most
EKS deployments run and what makes pod-to-pod work for any workload port:

```hcl
node_security_group_additional_rules = {
  ingress_self_all = {
    description = "Node to node, all ports"
    protocol    = "-1"
    from_port   = 0
    to_port     = 0
    type        = "ingress"
    self        = true
  }
}
```

The narrow rule is the minimum that fixes the observed failure. The broad rule
is the minimum that makes the cluster behave the way a reader would assume a
Kubernetes cluster behaves.

**Requires an AWS mutation:** yes — `terraform apply` adding one
`aws_security_group_rule`. No instance replacement, no cluster replacement, no
new AWS service. **Not applied.**

---

# Correction applied and verified — 2026-09-13T22:34:31Z

One `aws_security_group_rule` added: ingress, tcp/80, self-referencing, on
`sg-0a8e0b3e1c2d0ddde`. The plan contained exactly that one change — no
deletions, no replacements, no cluster or node group modification, no other
resource touched. `ingress_self_all` was deliberately **not** used.

## Before and after

| Test | Before | After |
| --- | --- | --- |
| same-node pod→pod, direct IP | 3/3 PASS | 3/3 **PASS** |
| **cross-node pod→pod, direct IP** | **0/6** | **6/6 PASS** |
| ClusterIP from each of 3 sources | not run (would have measured ~33% and that is not availability) | 36/36 **PASS** |
| DNS resolution, each source | not run | 3/3 **PASS** |
| HTTP via DNS name, each source | not run | 3/3 **PASS** |
| Bounded sustained sampler | not run | **120/120 in 61.1s PASS** |

The sampler is the M5 control: that run used `wget` with no timeout and
recorded 31 samples in 34 minutes because each failure blocked for minutes.
This one uses `-T 2 -t 1`, completed 120 of 120 samples in 61.1 seconds, and
its sample count matching its design is what makes the result meaningful.

## Causal conclusions

**Confirmed — the ~1-in-3 pattern was caused by failed cross-node
connectivity.** A three-endpoint ClusterIP Service load-balances across all
three; with only the co-located endpoint reachable, roughly one request in
three succeeds. Before the fix, direct pod-IP tests showed exactly that: 3/3
same-node, 0/6 cross-node. After the fix the same Service returns 120/120.

**Refuted — VPC CNI / add-on ordering was not the cause.** This cluster was
built with `vpc-cni { before_compute = true }`; the CNI was ACTIVE before any
node registered; `aws-node` ran 2/2 with zero restarts throughout. Cross-node
traffic failed anyway, and was fixed by a change that touched no add-on.

**Established in this cluster — a missing node-SG self-ingress rule for TCP/80
caused the cross-node application failure.** Proved before the fix by failure
mode: port 80 cross-node timed out (dropped) while port 8080 — inside the
module's `1025-65535` self-rule, with nothing listening — was refused instantly
(arrived). Proved after the fix by 0/6 → 6/6 on the identical test.

**Correction — narrowly allow TCP/80 between members of the node security
group.** Port 80 only, self-referencing, nothing else broadened.

## Scope of this finding

This is a statement about **this cluster, on 2026-09-13**. The historical
capture in `evidence/eks-recovery/` is a different cluster on a different day
and has not been modified. What this establishes about that run is narrower and
worth stating exactly: the Terraform networking correction recorded there as
"implemented but not verified on a fresh cluster" has now been verified, and it
**does not fix cross-node pod networking** — because add-on ordering was not
the fault. Whether the missing SG rule also explains the 30.2% measured there
is **not proven**: that cluster no longer exists and cannot be re-probed.
