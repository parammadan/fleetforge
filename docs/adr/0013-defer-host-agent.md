# ADR-0013 — Defer the host-observability agent

**Status:** Accepted · 2026-09-12

## Context
A Rust DaemonSet collecting cgroup PSI, containerd state, kubelet health, filesystem pressure,
pod termination behavior, reboot and recovery duration, and Bottlerocket update state is a
genuine differentiator — those signals are not in the Kubernetes API.

It is also the largest security surface in the product. A binary on every node, with host mounts
and elevated capabilities, is a lateral-movement target. Its threat model is most of a threat
model on its own.

The decisive question: what in the demonstration needs it? Node inventory, pod placement,
requests, PDBs, Bottlerocket version, Brupop state, evictions, reboots, and recovery are all
observable through the API server and Brupop's own custom resources. The answer is nothing.

## Decision
No `ff-agent` in the vertical slice. Reboot and recovery timing come from node conditions and
Kubernetes events. The agent's privilege posture in `THREAT_MODEL.md` remains a design
commitment, explicitly marked as unverified.

## Consequences
- No privileged workload is deployed, so the slice carries no node-level attack surface.
- Recovery timing is coarser — event granularity rather than host granularity. Acceptable, and
  the resulting imprecision is stated in the prediction-versus-actual comparison rather than
  hidden.
- Prediction accuracy will be limited by API-server visibility, which is exactly the measurement
  that tells us whether the agent is worth building.

## Evidence that would justify building it
1. A recorded prediction miss where the API server provably could not have supplied the missing
   signal — for example a drain that stalled on I/O pressure invisible to the kubelet.
2. Recovery-time predictions failing accuracy targets specifically because event timestamps are
   too coarse.
3. A Bottlerocket update failure whose cause was only visible on the host.
4. A written per-capability justification, reviewed, before a single line is deployed.

Point 1 is the real bar. Until a miss is attributable to missing host data, the agent is a
capability in search of a requirement.

## Alternatives rejected
- **A minimal read-only agent now.** Even read-only host mounts on every node require the full
  privilege review; "minimal" does not reduce the surface, only the payoff.
- **Prometheus node-exporter instead.** Reasonable, and it answers the question — worth doing
  *first* if host signals become necessary, since it defers writing a privileged binary.
