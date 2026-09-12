# ADR-0016 — The recommendation summary is preflight output, not a rollout controller

**Status:** Accepted · 2026-09-12

## Context
The demonstration needs rollout guidance: is this maintenance safe, how many nodes at once, which
workloads are affected, what evidence supports that, and what impact is predicted.

There are two ways to produce it. A rollout-controller subsystem that owns the maintenance
operation over time — or a calculation over a single snapshot. The first is a stateful service;
the second is a function. They look similar in the UI and are nothing alike underneath.

## Decision
`RecommendationSummary` is a field of `PreflightResult`, produced by `ff-preflight`, a pure
function over one `ClusterSnapshot`.

```rust
pub struct RecommendationSummary {
    pub status: MaintenanceStatus,              // Safe | Blocked
    pub recommended_max_concurrency: u32,
    pub concurrency_constraint: ConstraintRef,  // WHICH finding produced that number
    pub affected_workloads: Vec<WorkloadImpact>,
    pub evidence: Vec<FindingId>,
    pub predicted_impact: PredictedImpact,      // pods evicted, capacity headroom, PDB margin
    pub snapshot_id: SnapshotId,
    pub mode: Mode,                             // always WhatIf
}
```

It holds no state, observes nothing over time, makes no decision between waves, and is always
labelled `WHAT-IF`.

`concurrency_constraint` is the part that matters. A bare number is a guess with a UI. A number
that names the finding that produced it — "2, because PDB `web/pdb-web` allows one disruption and
node `ip-10-0-2-17` holds two of its pods" — is a claim a reviewer can check.

## Consequences
- Determinism is free: same snapshot plus same request gives the same summary, byte for byte.
- Fully unit-testable against fixtures, with no cluster.
- Recomputing on every concurrency change is cheap, which is what makes the UI interactive.
- The summary cannot adapt mid-rollout. That is the wave planner's job, and it is deferred
  (ADR-0011). The UI must say **recommended**, and the report must never imply FleetForge
  controlled the rollout.

## Alternatives rejected
- **A rollout controller emitting the summary as a side effect.** Stateful, needs interruption
  and drift handling, and is the deferred planner wearing a smaller name.
- **Concurrency computed in the frontend.** Puts a safety-relevant calculation in the least
  testable layer, and it would drift from the backend's analyzers immediately.
