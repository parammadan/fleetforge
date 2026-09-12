# ADR-0011 — Defer the wave planner

**Status:** Accepted · 2026-09-12

## Context
The original scope included a full maintenance planner: ordered waves, canary selection,
preconditions, stop conditions, SLO-driven concurrency that grows while health holds, and
rollback criteria. It is the most visually impressive part of the product and the most obvious
"systems engineering" artifact.

It is also a stateful subsystem — it observes health over time, makes decisions between waves,
and must behave correctly when it is interrupted, when the cluster drifts underneath it, and when
a wave partially fails. That is a distributed-systems problem, not a UI feature.

The demonstration does not need it. A human selects nodes; FleetForge reports whether that
selection is safe and what concurrency it can support. Brupop sequences the actual updates.

## Decision
Do not build a wave planner for the vertical slice. `ff-preflight` emits a **recommendation
summary** instead: safe or blocked, a recommended maximum concurrency with the constraint that
produced it, affected workloads, evidence, and predicted impact. It is a pure calculation over
one snapshot and holds no state (ADR-0016).

## Consequences
- The engineering effort stays on analysis correctness, which is where the product's credibility
  actually lives.
- Concurrency is recommended, not enforced. The UI must say "recommended", and the report must
  not imply FleetForge controlled the rollout.
- The ten execution invariants are still written and property-tested, so the planner has a
  correctness harness waiting when it is built.
- A half-built planner would be worse evidence of engineering judgement than a written decision
  not to build one yet.

## Evidence that would justify building it
1. A maintenance operation spanning more nodes than an operator will reasonably sequence by hand —
   in practice, more than about ten.
2. A second real user asking for sequencing rather than assessment.
3. Recommendation summaries that are consistently correct, so wave logic has a trustworthy input.
4. At least one recorded run where manual sequencing caused a disruption FleetForge predicted.

Point 4 matters most: it converts the planner from a feature into a fix for an observed failure.

## Alternatives rejected
- **Build a simple planner now.** "Simple" planners acquire stop conditions and retry logic within
  weeks, at which point they are stateful and half-tested.
- **Drop concurrency guidance entirely.** The operator genuinely needs the number; it is cheap to
  compute and expensive to guess.
