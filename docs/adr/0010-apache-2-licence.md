# ADR-0010 — Apache-2.0 licence

**Status:** Accepted · 2026-09-12

## Context
FleetForge is intended as public open-source infrastructure tooling that integrates with
Bottlerocket and Brupop, both Apache-2.0, and sits in an ecosystem — Kubernetes, containerd,
Prometheus — that is overwhelmingly Apache-2.0.

## Decision
Apache-2.0 for FleetForge.

## Consequences
- Compatible with the projects it integrates with and the dependencies it will pull in.
- The express patent grant is meaningful for infrastructure software adopted by companies.
- Permissive: a vendor may build a commercial product on it. Accepted as the cost of adoption.
- Every dependency's licence must be checked in CI (`cargo deny`) — a GPL dependency would force
  this ADR to be revisited.

## Alternatives rejected
- **MIT.** Shorter, no patent grant. The grant is the reason to prefer Apache-2.0 here.
- **AGPL.** Would prevent exactly the adoption this project wants.
