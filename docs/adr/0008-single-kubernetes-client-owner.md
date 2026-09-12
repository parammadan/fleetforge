# ADR-0008 — Only `ff-collect` may construct a Kubernetes client

**Status:** Accepted · 2026-09-12

## Context
"Read-only mode cannot mutate the cluster" is a load-bearing safety claim. If any crate can build
a client, the claim rests on every developer remembering it forever — which is not a guarantee.

## Decision
`ff-collect` is the only crate that depends on `kube` and the only place a client is constructed.
It exposes read-only, watch-derived snapshots. `ff-exec` receives a narrowly scoped mutating
client, constructed only after an approval token validates, carrying the separate execution
identity. Everything else takes a `ClusterSnapshot` value.

## Consequences
- The claim becomes a one-line check: does anything outside `ff-collect` and `ff-exec` depend on
  `kube`? A CI test asserts it.
- Analyzers and the planner are pure and testable without a cluster — which is also what makes
  8 GB development pleasant.
- Any new data need is satisfied by extending the snapshot, not by an inline fetch. Slightly more
  work, and it keeps the boundary intact.

## Alternatives rejected
- **A shared read-only client handle.** Convenient, and the boundary erodes the first time someone
  needs one extra field.
- **Runtime read-only flag.** A flag is checked by code that can forget to check it.
