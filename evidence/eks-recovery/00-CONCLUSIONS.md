# EKS demonstration — what actually happened

Account 771965334314, `us-east-2`, cluster `fleetforge-demo`, 2026-09-13.
Every claim below is backed by a numbered file in this directory.

## Accurate conclusions

**FleetForge detected the already-existing PodDisruptionBudget blocker.** Running
preflight against the node Brupop was stuck on returned a single blocker,
`FF-PDB-001`, with the arithmetic: `disruptionsAllowed = currentHealthy(2) -
desiredHealthy(2) = 0`. See `03-preflight-before.json`.

**FleetForge did NOT predict the deadlock.** Timestamps settle it:

| | |
| --- | --- |
| Brupop installed | 14:08:15Z |
| FleetForge recording started | 14:15:47Z |
| first cordon observed | 14:15:48Z — already cordoned in the first snapshot |
| first preflight run | 14:15:50Z |

Recording began 7.5 minutes after Brupop started. No preflight finding exists
from before the update began, so no prediction was made about it.

**Human investigation identified the feedback loop.** FleetForge reported the
blocker; it did not explain the cycle. The diagnosis — Brupop cordons nodes →
the third `web` replica cannot schedule → `currentHealthy` falls to
`desiredHealthy` → `disruptionsAllowed` reaches 0 → Brupop cannot evict → the
cordons never lift — was worked out by reading node, PDB, and pod state
together. Nothing in FleetForge draws that causal chain.

**Original traffic availability during the incident is unknown.** The sampler
was defective: `wget` with no timeout blocked for minutes on each failure, so it
recorded 31 samples in 34 minutes instead of ~2000, and stalled precisely when
things were going wrong. Its 17/14 split measures `wget`'s patience, not the
service's availability. Nothing can be concluded from it.

**The post-recovery rerun does not demonstrate health either.** With the sampler
bounded (`-T 2 -t 1`), 139 requests over 334s returned **30.2% success, 69.8%
failure** — from the traffic pod and, independently, from a freshly created pod.
All three `web` pods were `Running 1/1`, all three Service endpoints `Ready`,
DNS resolving, kube-proxy and aws-node healthy on every node with zero restarts.

A ~1-in-3 success rate against a 3-endpoint Service is the signature of only the
*same-node* endpoint being reachable — cross-node pod networking failing. The
most likely cause is this cluster's addon ordering: the VPC CNI was installed
**after** the nodes had already joined, because the Terraform did not request
any addons. That is fixed in `infra/terraform/main.tf` with
`vpc-cni = { before_compute = true }`, but it was not re-verified on a fresh
cluster before teardown.

So: post-recovery validation **failed**, and the failure is attributable to
cluster networking, not to the maintenance recovery.

## The recovery itself did work

| Step | Evidence |
| --- | --- |
| Uncordon `101-90` only | `12-recovery-timeline.txt` |
| Deadlock broke | `healthy=3 allowed=1`, 3 Running, at 14:35:14 |
| Brupop resumed and finished | `brupop-194 = Idle` at 14:36:44 |
| Brupop left `101-194` cordoned | same stale-cordon behaviour as `101-90` |
| Uncordon `101-194` only | `24-uncordon-194-recovery.txt` |
| All gates met | `ready=3/3 sched=3/3`, 3 pods `1/1 Running`, `healthy=3 allowed=1`, all shadows `Idle` |

All three nodes ended on **Bottlerocket 1.64.0** — a real Brupop update,
observed by FleetForge as `LIVE`.

## Bugs this environment found, all fixed and tested

1. **Bottlerocket version read from the wrong field.** Every node reported
   `2.0.0`, which is `bottlerocket.aws/updater-interface-version` — Brupop's
   interface version, not the OS release. Now parsed from the kubelet's OS image
   string; verified against the cluster as `1.64.0 / 1.62.1 / 1.64.0`.
2. **Under-prediction mislabelled as conservative.** A run predicting 5 evictions
   and observing 8 was reported "conservative". Over- and under-prediction are
   different mistakes and now have different verdicts.
3. **34 findings where 14 were useful.** With every remaining node cordoned,
   each per-pod placement analyzer fired for each pod. `FF-NODES-001` now states
   it once and the per-pod checks stand down.
4. **No EKS addons.** Cost 30 minutes of a node group stuck in `CREATING`.
5. **Unbounded traffic sampler**, above.

## Snapshot provenance

| | |
| --- | --- |
| before | `5de2639ac583535c443bae827ccb47cb`, 14:32:26Z, 3 nodes / 26 pods / 3 shadows |
| final | `632cbde631dad88af678ecec06e7d1c3`, 14:53:25Z, 3 nodes / 28 pods / 3 shadows |

Every fact carries `mode=live`, a resourceVersion, and an observation timestamp.
