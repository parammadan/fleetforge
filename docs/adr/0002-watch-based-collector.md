# ADR-0002 — Watch-based collector with immutable content-hashed snapshots

**Status:** Accepted · 2026-09-12

## Context
Analysis must run against a *consistent* view of the cluster. Polling produces torn reads and
lags reality. Analyzing mutable shared state produces findings that cannot be reproduced, which
destroys the value of an evidence-backed report.

## Decision
Run kube-rs watchers/reflectors per resource kind. Materialize immutable `ClusterSnapshot` values
identified by a content hash. Analysis, planning, approval, and audit all reference a
`snapshot_id`.

## Consequences
- A finding can be re-derived exactly, months later, from its snapshot id.
- Approval binds to a plan which binds to a snapshot — staleness becomes checkable.
- Snapshots cost memory proportional to cluster size; large clusters will need field pruning at
  normalization time and possibly a retention policy. Acceptable at current scale, revisited when
  a real cluster says otherwise.
- Hashing must be canonical and stable across architectures — property-tested in M1.

## Alternatives rejected
- **Poll on demand.** Simpler, but no live UI and no reproducibility.
- **Analyze the live reflector store directly.** Zero copy, but no stable identity, so no
  reproducible findings and no meaningful approval.
