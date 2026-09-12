# ADR-0009 — Docker Desktop with Kind for local development

**Status:** Accepted · 2026-09-12

## Context
The development machine is an Apple M1 with 8 GB RAM and roughly 35 GB free disk. Docker Desktop
29.6.1 is already installed; Colima is not. Kubernetes development normally wants a local cluster,
which on this machine competes directly with the Rust toolchain for memory.

## Decision
Use the already-installed Docker Desktop, capped at approximately 2 CPU and 4 GB, with Kind for a
minimal cluster. Start it only when cluster integration genuinely requires it. Develop
fixture-first the rest of the time: `ff-preflight` and `ff-planner` are pure functions over
recorded snapshots and need no cluster, no Docker, and no memory.

Heavy integration — multi-node Kind, cross-architecture builds, container scanning, EKS and
Bottlerocket — runs in CI or a temporary AWS environment.

## Consequences
- No second VM runtime to install, configure, and keep working.
- The default development loop is `cargo test` against fixtures, which is fast and light.
- Disk is the binding constraint, not memory: `target/` plus container images against 35 GB free
  needs watching, and `target/` is the first thing to prune.
- If Docker Desktop licensing ever becomes an issue for this project, Colima is a drop-in
  replacement and this ADR gets superseded.

## Alternatives rejected
- **Colima.** Lighter, and a second runtime to maintain for no current benefit.
- **k3d / minikube.** Kind is the closest to upstream conformance and the standard for testing
  Kubernetes tooling.
- **Remote development cluster always-on.** Costs money continuously and makes offline work
  impossible.
