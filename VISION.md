# FleetForge — Vision

## One sentence

FleetForge tells you whether a Kubernetes node can be safely taken out of service, proves its
answer with exact cluster evidence, and only then hands the actual work to a real executor.

## The problem

Node maintenance — Bottlerocket updates, Kubernetes upgrades, kernel reboots, instance
replacement, Spot reclamation, AZ maintenance — is routine and routinely dangerous. The
operator-facing question is always the same:

> If I drain these nodes, right now, what breaks?

Today that question is answered by tribal knowledge, a nervous `kubectl drain --dry-run`, or
by finding out in production. `kubectl drain` does not tell you *ahead of time* that a
PodDisruptionBudget will wedge you at node 3 of 12, that the remaining nodes cannot fit the
evicted pods, or that both replicas of your only stateful workload are pinned to the same
availability zone.

## What FleetForge is

An **analysis, explanation, planning, observation, and audit** plane for node maintenance.

1. **Collect** live cluster state through the Kubernetes API, with provenance on every fact.
2. **Analyze** a proposed maintenance action against that state with explicit, auditable checks.
3. **Explain** every finding with the exact objects, the exact arithmetic, and its limitations.
4. **Plan** a wave-based rollout with concurrency, canary, stop, and rollback conditions.
5. **Approve** — a human reviews an immutable plan and a literal diff of proposed mutations.
6. **Execute** — hand off to a real executor (Brupop, cordon/drain, node-group replacement).
7. **Observe** the real rollout and **record** it for replay and audit.

## What FleetForge is not

**FleetForge is not a Brupop replacement or clone.**

| FleetForge | Brupop |
| --- | --- |
| Decides *whether* and *in what order* | Performs the update |
| Analyze, explain, plan, observe, audit | Cordon, drain, update, reboot, uncordon |
| Human approval gate | Coordination of Bottlerocket node updates |

FleetForge is also **not a Kubernetes scheduler simulator**. Version 1 runs explicit
evidence-based analyzers. An aggregate-capacity check is a necessary condition for
reschedulability, never a sufficient one, and FleetForge says so in the finding itself.

## Four differentiating systems

FleetForge is not one idea; it is four that reinforce each other. Each is independently useful,
and each makes the others more credible.

### 1. Maintenance digital twin
A representation of the live cluster plus a calculation of what happens if selected nodes become
unavailable — PDBs, requests, taints/tolerations, selectors, required affinity and anti-affinity,
topology spread, AZs, DaemonSets, StatefulSets, PV restrictions, singletons, unmanaged pods,
priority/preemption, grace periods, autoscaling and Karpenter capacity, and multi-node removal.

FleetForge does not claim scheduler equivalence. Every finding states its confidence,
assumptions, and limitations. Once real drains are observed, predictions are scored against
outcomes and the accuracy is published rather than asserted.

### 2. Rust host-observability agent
A small DaemonSet agent for signals the API server does not carry: cgroup CPU/memory pressure,
containerd state, kubelet health, filesystem pressure, pod termination behavior, reboot and
recovery duration, process/network information, kernel and OS metadata, and Bottlerocket version
and update state. Minimum privilege; every host capability it requests is justified in
`THREAT_MODEL.md`. eBPF and deeper tracing only after the basics are reliable.

### 3. Adaptive maintenance planner
Waves with canary selection, computed safe concurrency, per-node justification, preconditions,
stop conditions, SLO observation, concurrency that grows while health holds and pauses when
latency/errors/capacity/availability degrade, and a rollback recommendation. For Bottlerocket,
approved updates are executed *through Brupop*.

### 4. Recording, chaos experiments, and replay
Normalized Kubernetes, Brupop, FleetForge, and workload-health events, persisted. Controlled
experiments in a test environment — PDB-blocked drain, insufficient capacity, slow termination,
containerd failure, kubelet failure, network interruption, unhealthy node after reboot, AZ
capacity loss — replayable on an interactive timeline. Replay is never displayed as live.

## Non-negotiable honesty rules

Every fact surfaced in the UI or API carries a provenance label:

| Label | Meaning |
| --- | --- |
| `LIVE` | Read from the current Kubernetes/AWS environment, with an observation timestamp |
| `REPLAY` | Recorded historical events, played back on a timeline |
| `FIXTURE` | Local development or test data |
| `WHAT-IF` | A computed outcome that has **not** happened |

A preflight result is by construction `WHAT-IF`. Presenting it as an event that occurred is a
correctness bug, not a cosmetic one. Nothing shown as `LIVE` may be hard-coded, cached beyond
its stated freshness, or fabricated.

## Twelve-month scope

Bottlerocket updates via Brupop → Kubernetes version upgrades → kernel reboots → node
replacement → Spot interruption analysis → cluster scaling → availability-zone maintenance.

## Definition of success

A real EKS cluster with real Bottlerocket nodes and real traffic, where FleetForge blocks an
unsafe drain, explains exactly why, shows the risk clear after a real fix, and then observes a
real Brupop update end to end — with an evidence-backed report at the end that a skeptical
reader can verify against the cluster.
