# ADR-0004 — Evidence-based analyzers, not scheduler simulation, in V1

**Status:** Accepted · 2026-09-12

## Context
"Will these pods reschedule?" is genuinely answered only by the Kubernetes scheduler: predicates,
priorities, preemption, dynamic resource allocation, and every plugin the cluster has enabled.
Reimplementing that is a large project and would be wrong in ways nobody could see.

But an approximation presented as certainty is worse than no answer.

## Decision
V1 ships explicit, independent analyzers. Each states what it proves, what it assumes, and what it
does not prove, in a required `limitations` field on every finding. Aggregate capacity checks are
labelled as necessary-but-not-sufficient conditions, in those words.

A higher-fidelity scheduling adapter — upstream predicates, or an isolated throwaway control
plane — is designed later and must be *validated* before its output is presented differently.

## Consequences
- FleetForge can be trusted, because it never claims more than it knows.
- Some genuine failures are missed in V1 — no analyzer catches everything, and that limitation is
  published rather than hidden.
- M7 scores predictions against real observed drains, so the accuracy claim is measured, not
  asserted.

## Alternatives rejected
- **Reimplement the scheduler.** Large, subtly wrong, and unverifiable.
- **Shell out to `kubectl drain --dry-run`.** Only covers eviction API admission; blind to
  capacity, affinity, topology, and storage. Useful as one signal, not as the answer.
