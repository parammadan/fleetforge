# ADR-0017 — The EKS environment is ephemeral; the artifacts are the deliverable

**Status:** Accepted · 2026-09-12

## Context
The demonstration requires real Bottlerocket nodes and a real Brupop update. Kind cannot provide
either — Bottlerocket is an OS, not a container image, and Brupop needs actual hosts to reboot.
So EKS is unavoidable.

A persistent EKS environment costs roughly $8/day whether or not anyone is looking at it, and the
development work — analyzers over fixtures, the API, the frontend — does not need it.

## Decision
Terraform stands the environment up for the demonstration and tears it down afterwards. All
development happens against fixtures and Kind. The cluster is disposable; the recording, event
log, evidence report, and prediction-versus-actual comparison are not.

Every resource is tagged `Project=FleetForge`, `Environment=dev`, `ManagedBy=Terraform`,
`Owner=Param`. Teardown is documented and run by hand, never automatically.

## Consequences
- Cost is bounded to the days the demonstration is actually being captured.
- Reproducible stand-up from a clean checkout is forced, because the environment is rebuilt every
  time rather than accumulating hand-applied fixes. This is the main engineering benefit.
- Every artifact must be captured *before* teardown. A missing recording means paying to rebuild.
- Iterating on the real environment has a rebuild cost, which pushes correctness work back onto
  fixtures where it is faster anyway.
- Bottlerocket update testing needs a version genuinely behind current, so the node group must be
  pinned to an older AMI at stand-up. Discovered at plan time, not during the run.

## Evidence that would justify a persistent cluster
1. Debugging cycles against the real environment exceeding roughly a week of continuous work, at
   which point rebuild friction costs more than $8/day.
2. Prediction-versus-actual needing many recorded runs rather than one.
3. A second person needing simultaneous access.

## Alternatives rejected
- **Persistent dev cluster.** Convenient, and burns money continuously during a period when
  nothing is connecting to it.
- **kOps or self-managed Bottlerocket on EC2.** Cheaper control plane, considerably more
  operational surface, and less representative of how anyone actually runs Bottlerocket.
