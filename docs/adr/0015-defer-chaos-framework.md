# ADR-0015 — Defer the chaos-experiment framework

**Status:** Accepted · 2026-09-12

## Context
Controlled fault injection — PDB-blocked drain, insufficient capacity, slow termination,
containerd failure, kubelet failure, network interruption, unhealthy node after reboot, AZ
capacity loss — would validate the analyzers against real failure rather than fixtures.

A framework to do that is a product in itself: a fault catalogue, injection mechanisms per fault
type, blast-radius containment, cleanup that works when an experiment fails halfway, and safety
interlocks preventing a fault ever reaching a real cluster.

It is also worth noticing which faults matter now. The two the demonstration depends on —
a restrictive PDB and insufficient capacity — are produced with `kubectl apply` and
`kubectl scale`. No framework required. The remaining faults are failures of the *host agent's*
domain, and the host agent is deferred (ADR-0013).

## Decision
No chaos framework. The demonstration creates its unsafe conditions with plain `kubectl`,
committed as manifests in `fixtures/scenarios/` so they are reproducible and reviewable.

## Consequences
- The unsafe conditions are transparent: a reviewer reads a six-line PDB manifest rather than
  trusting an injection harness.
- No fault-injection code can ever reach a real cluster, because none exists.
- Analyzer validation rests on fixtures and one real run. That is a genuine limitation and is
  stated in the prediction-versus-actual comparison.
- Runtime failures — kubelet death, containerd failure — go untested. Accepted: the slice's
  analyzers reason about scheduling constraints, not runtime health.

## Evidence that would justify building it
1. Analyzers passing every fixture test but missing a failure in a real drain — fixtures
   modelling the cluster we imagined rather than the one we have.
2. Enough analyzers that hand-built scenarios stop covering the interaction space.
3. Runtime-health analyzers existing at all, which requires the host agent first.
4. A test environment where blast radius is provably contained.

## Alternatives rejected
- **Chaos Mesh or Litmus.** Mature and avoids writing a framework — but adds a cluster-wide
  privileged component for faults nothing yet analyzes.
- **Scripted `kubectl` fault scenarios.** Effectively the decision above, without calling it a
  framework. This is what `fixtures/scenarios/` is.
