# ADR-0001 — Rust for the control plane

**Status:** Accepted · 2026-09-12

## Context
FleetForge continuously watches cluster state, holds a normalized in-memory model, runs analysis
on every change, and will eventually run a privileged node-local agent. It must not add
unpredictable latency or memory pressure to a cluster it is supposed to protect, and on an 8 GB
development machine it must not be heavy to run.

Go is the ecosystem default and has the most mature Kubernetes client. Rust has kube-rs — less
mature, but capable — plus no GC pauses, a much smaller memory footprint, and a type system
strong enough to make invariants like "read-only cannot mutate" structural rather than
procedural.

## Decision
Rust for the control plane, collector, preflight engine, planner, and host agent. Axum, Tokio,
kube-rs, Serde, tracing.

## Consequences
- Invariants can be encoded in types: a replay session that holds no adapter handle cannot
  execute, regardless of what any caller does.
- Small binaries and low memory — the agent is a DaemonSet on every node, where this matters.
- kube-rs trails client-go. CRD handling and some API surfaces need more work. Accepted.
- Compile times are a real cost on the development machine; mitigated by workspace structure,
  `cargo check` in the inner loop, and pure crates that test without a cluster.

## Alternatives rejected
- **Go.** Best client library, and the ecosystem norm. Rejected because the invariant-enforcement
  story is weaker and the agent footprint is worse — and the brief prefers Rust.
- **Rust control plane + Go agent.** Two toolchains, two CI pipelines, for no benefit.
