# ADR-0012 — Defer the execution controller

**Status:** Accepted · 2026-09-12

## Context
The design calls for approval-gated execution adapters: cordon/drain, Brupop update submission,
managed node-group replacement. That requires FleetForge's own authentication and authorization,
a second Kubernetes identity, an allow-list of resource kinds and field paths, single-use
hash-bound approval tokens, audit hash-chaining, and postcondition verification.

Every one of those is a prerequisite for the *first* mutating endpoint, not a follow-up to it.
Together they are larger than the entire analysis path.

And for the Bottlerocket demonstration, the executor already exists: Brupop. A human applying
the Brupop custom resource after FleetForge reports safe is a real approval gate — arguably a
more honest one, because the human's hand is literally on the trigger.

## Decision
No execution adapter in the vertical slice. `ff-exec` ships one implementation, `RecommendOnly`,
which returns the intended mutation and performs none. FleetForge reads and recommends;
Brupop and the operator act. FleetForge then observes what they did.

## Consequences
- FleetForge cannot damage a cluster during the slice, because no mutating client is ever
  constructed (ADR-0008). The strongest safety property in the product is free.
- No FleetForge authN/authZ is needed yet; M2 binds to `127.0.0.1`.
- The demonstration has a manual step. This is a feature: the reviewer watches a human approve.
- The ten execution invariants are written and property-tested regardless, so this ADR can be
  revisited against a working harness rather than a blank page.

## Evidence that would justify building it
1. A maintenance type Brupop does not cover — Kubernetes version upgrades, kernel reboots on
   non-Bottlerocket nodes, node-group replacement.
2. A second real user, making the manual step a genuine bottleneck rather than a demo beat.
3. All ten invariants passing property tests, including `read-only ⇒ no mutating client` and
   `approval binds to exact plan hash`.
4. FleetForge's authN/authZ and permission tiers built and reviewed — a hard prerequisite, not a
   parallel workstream.

## Alternatives rejected
- **Cordon/drain adapter behind a feature flag.** A flag is checked by code that can forget to
  check it. Unreachable-by-construction is a stronger guarantee than disabled-by-default.
- **FleetForge submits the Brupop CR.** One `kubectl apply` of saved effort, in exchange for
  needing the entire authorization and audit stack.
