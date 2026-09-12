# ADR-0006 — Approval is bound to an immutable plan hash

**Status:** Accepted · 2026-09-12

## Context
An approval is only meaningful if what executes is exactly what a human reviewed. The dangerous
case is subtle: an operator reviews a plan, the cluster changes underneath, the plan is
recomputed, and the old approval carries forward onto different work. Nobody lied; the guarantee
simply was not there.

## Decision
Plans are deterministic and content-hashed. An approval token is bound to one `plan_hash`, is
single-use, and is time-bounded. A recomputed plan — even one that looks identical — has a
different identity if any input changed, and needs a new approval. Snapshot freshness is
re-verified before each wave; a stale snapshot aborts execution rather than proceeding.

## Consequences
- "Approval applies only to the exact reviewed plan" is enforced, not documented.
- Operators sometimes re-approve after cluster drift. That friction is the feature.
- Plan generation must be genuinely deterministic: stable iteration order, no timestamps or
  randomness in the hashed content. Property-tested.

## Alternatives rejected
- **Approve a maintenance *intent*.** Convenient; the approved thing is then not the executed
  thing. Rejected outright.
- **Approval with a re-confirmation prompt on drift.** Relies on a human reading a dialog at the
  moment they are least likely to.
