# FleetForge — Roadmap

Twelve months, vertical milestones. Each milestone has scope, non-goals, acceptance criteria,
tests, a demonstration procedure, and known limitations. No milestone begins before the previous
one demonstrably works.

Legend: ✅ done · 🔨 in progress · ⬜ not started · 🔒 blocked on explicit approval

---

## M0 — Repository foundation 🔨
**Scope.** Environment inspection, repository structure, component boundaries, data model, API
contract, and the document set. No dependencies installed, no cloud resources, no cluster access.

**Non-goals.** Any Rust or TypeScript implementation. Any Kubernetes connection.

**Acceptance.** Repository exists with `README`, `VISION`, `ARCHITECTURE`, `THREAT_MODEL`,
`ROADMAP`, `STATUS`, `CONTRIBUTING`, `DEMO`, ADRs 0001–0010, Apache-2.0 licence, `.gitignore`,
CI skeleton. Open decisions are listed for the maintainer.

**Limitations.** Every design here is unvalidated until M2 runs against a real cluster.

---

## M1 — Toolchain and workspace skeleton ⬜
**Scope.** Install the Rust toolchain. Create the cargo workspace with `ff-core`, `ff-collect`,
`ff-preflight`, `ff-store`, `ff-api`. `ff-core` gets real types: `Provenance`, `Mode`,
`ClusterSnapshot`, `Finding`, and their tests. `cargo fmt`, `clippy -D warnings`, and `cargo test`
run green in CI on ARM64 and x86_64.

**Non-goals.** Kubernetes connection. Frontend.

**Acceptance.** `make check` passes from a clean checkout. Snapshot content hashing is stable
across runs and across architectures — property-tested.

---

## M2 — Live read-only vertical slice 🔒
Requires explicit approval before starting.

**Scope.** Connect to a kubeconfig context the operator names. Read-only. Watch Nodes, Pods,
Deployments, and PDBs. Normalize in Rust. Serve through Axum. Stream over SSE. Render a live
cluster overview and node/workload topology in React with provenance and observation timestamps.

**Acceptance.**
- `kubectl scale deploy/x --replicas=5` run externally appears in the UI with no page refresh and
  no frontend polling timer.
- Disconnected, forbidden, loading, stale, and empty states each render correctly and are
  reachable in a test.
- Fixture mode is visibly labelled `FIXTURE` in persistent chrome.
- A test asserts no secret material appears in any response or log line.

**Non-goals.** Any mutation. Any AWS resource. Preflight analysis.

**Limitations.** Single cluster, no authentication, localhost bind only.

---

## M3 — Evidence-based preflight ⬜
**Scope.** First analyzers: PDB blocks voluntary disruption · singleton workload · insufficient
aggregate CPU · insufficient aggregate memory · node selector prevents placement · missing
toleration · required node affinity · required pod anti-affinity. Findings carry evidence,
calculation, confidence, and limitations. Preflight workspace and evidence drawer in the UI.

**Demonstration.** Observe a real cluster → create a restrictive PDB externally → run preflight →
show the exact blocker with evidence → relax the PDB externally → re-run → blocked becomes safe.
Every step verifiable with `kubectl` alongside.

**Limitations.** Aggregate capacity is a necessary, not sufficient, condition. Stated in every
finding it produces.

---

## M4 — Maintenance planner ⬜
Waves, canary recommendation, computed safe concurrency, per-node justification, preconditions,
stop conditions, rollback criteria. Deterministic for a given snapshot — property-tested.
Planner UI with a concurrency control that recomputes risk live. Still zero mutations.

---

## M5 — Approval and controlled execution ⬜ 🔒
FleetForge's own authN/authZ and permission tiers. Plan → diff → risk summary → approval bound to
plan hash → allow-list check → execution → postcondition verification → audit. First real adapter:
Kubernetes cordon/drain, behind a cargo feature and a runtime allow-list. All ten invariants
property-tested before the adapter is enabled anywhere.

---

## M6 — AWS environment and Brupop ⬜ 🔒
Terraform for a deliberately small EKS cluster with a Bottlerocket managed node group. Helm charts
for FleetForge and Brupop. Brupop CR collection and the Brupop execution adapter. Read-only AWS
identity inspection, shown plan, and explicit approval precede any apply. Documented, manually-run
cleanup.

---

## M7 — Recording, replay, and prediction accuracy ⬜
Persist observations, snapshots, findings, plans, approvals, execution events, and workload
health. Interactive replay timeline, never rendered as live. Score predicted disruption against
observed disruption and publish the accuracy rather than asserting correctness.

---

## M8 — Host-observability agent ⬜
`ff-agent` DaemonSet: cgroup PSI, filesystem pressure, kubelet health, containerd state, pod
termination behavior, reboot and recovery duration, kernel/OS metadata, Bottlerocket version and
update state. Minimum privilege, every capability justified in the threat model. No eBPF.

---

## M9 — Adaptive planning, SLOs, and chaos experiments ⬜
Live SLO observation, concurrency that grows while health holds and pauses on degradation,
rollback recommendation. Controlled fault injection in a test environment: PDB-blocked drain,
insufficient capacity, slow termination, containerd failure, kubelet failure, network
interruption, unhealthy node after reboot, AZ capacity loss.

---

## M10 — Leadership demonstration and public release ⬜
The full ten-step EKS + Bottlerocket + Brupop demonstration with continuous traffic and an
evidence-backed predicted-versus-actual report. Performance and reliability testing. Public
open-source release: security policy, issue templates, signed releases, published container
images for both architectures.

---

## Testing strategy

| Layer | Approach |
| --- | --- |
| `ff-core` | Unit tests; property tests for snapshot hashing and provenance invariants |
| `ff-preflight` | Table-driven tests over fixture snapshots — one fixture per analyzer, plus adversarial cases; every analyzer must have a fixture that *fails* it and one that *passes* it |
| `ff-planner` | Property tests: determinism, concurrency clamping, wave ordering, blocked ⇒ unapprovable |
| `ff-collect` | Integration tests against Kind in CI; fixture replay locally |
| `ff-api` | Contract tests over the JSON schema; secret-leak assertions; forbidden/stale/empty state coverage |
| `web/` | `tsc --noEmit`, Vitest, and a Playwright run of each UI state against fixture mode |
| Invariants | Property-based tests (proptest) for all ten execution invariants, gating the executor |
| Security | `cargo deny`, `cargo audit`, `npm audit`, secret scanning, container scanning |

Heavy integration — Kind, EKS, Bottlerocket, Brupop — runs in CI or a temporary AWS environment,
never as a prerequisite for local development on an 8 GB machine.

## Local development strategy (Apple M1, 8 GB)

The constraint is real and shapes the design rather than being worked around.

- **Fixture-first.** `ff-preflight` and `ff-planner` are pure functions over recorded snapshots.
  The majority of development needs no cluster at all — no Docker, no Kind, no memory pressure.
- **Container VM capped at ~2 CPU / 4 GB**, started only when a Kind cluster is genuinely needed.
- **Kind kept minimal** — control plane plus one or two workers, torn down after use. Fixtures are
  captured from it once and reused offline.
- **Rust build discipline.** Workspace-shared `target/`, `sccache` if it earns its keep, thin LTO
  off in dev profile, `cargo check` in the inner loop. Note: 35 GB free disk at M0 is a real
  constraint — `target/` is the first thing to prune.
- **No local observability stack.** `tracing` to stdout; Prometheus and OTel collectors exist in
  the cluster environments, not on the laptop.
- **Anything heavy runs in CI** — cross-architecture builds, Kind integration, container scanning.
