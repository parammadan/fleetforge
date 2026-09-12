# FleetForge — Roadmap

**One polished, correct, end-to-end vertical slice.** Not a twelve-month product plan.

The target is a single demonstration that survives a skeptical reviewer with `kubectl` open
beside it: a real EKS cluster with real Bottlerocket nodes under real traffic, an unsafe
maintenance condition detected and explained before anything changes, the condition fixed, the
risk verifiably clearing, a real Brupop update observed end to end, and an evidence report that
compares what FleetForge predicted against what actually happened.

Everything not on the path to that demonstration is deferred with a written rationale. See
ADRs 0011–0015 for what was deferred and the evidence that would justify building it.

Legend: ✅ done · 🔨 in progress · ⬜ not started · 🔒 requires explicit approval

---

## Final deliverables

The slice is finished when all seven exist and are reproducible from a clean checkout:

| # | Deliverable | Produced by |
| --- | --- | --- |
| D1 | Real EKS run with real Bottlerocket worker nodes | M5 |
| D2 | Live Brupop observation — cordon, drain, update, reboot, recovery | M4, M5 |
| D3 | Recorded demonstration video | M5 |
| D4 | Persisted event log as inspectable JSON | M4 |
| D5 | Exported evidence report | M4 |
| D6 | Prediction-versus-actual comparison | M4, M5 |
| D7 | Reproducible Terraform + Helm setup, with documented teardown | M5 |

---

## M0 — Repository foundation ✅

Environment inspection, repository structure, component boundaries, data model, API contract,
documents, ADRs. No dependencies installed, no cluster contacted, no AWS call.

**Delivered.** 4 commits at `~/fleetforge`. Unvalidated until M2 runs against a real cluster.

---

## M1 — Workspace and domain model ⬜ · ~4 days

**Scope.** Install the Rust toolchain, pinned in `rust-toolchain.toml`. Cargo workspace with four
crates: `ff-core`, `ff-collect`, `ff-preflight`, `ff-api`. Real types in `ff-core` —
`Mode`, `Provenance`, `CollectionStatus`, `ClusterSnapshot`, `NodeFact`, `PodFact`, `PdbFact`,
`WorkloadFact`, `Finding`, `FleetForgeError` — with tests. CI green on ARM64 and x86_64.

**Non-goals.** Kubernetes connection. Frontend. Persistence.

**Acceptance.**
- `make check` passes from a clean checkout: `fmt --check`, `clippy -D warnings`, `test`.
- Snapshot content hashing is canonical and stable across runs and across architectures —
  property-tested. This is load-bearing: approval binds to a plan which binds to a snapshot id.
- A test asserts no crate outside `ff-collect` depends on `kube` (ADR-0008).

**Limitations.** Types are unvalidated against real cluster objects until M2.

---

## M2 — Live read-only slice ⬜ 🔒 · ~10 days

Requires explicit approval before starting.

**Scope.** Connect to a kubeconfig context the operator names. Read-only. kube-rs watches on
Nodes, Pods, Deployments, StatefulSets, DaemonSets, ReplicaSets, PDBs, and Events. Normalize,
snapshot, serve through Axum, stream over SSE. React + TypeScript + Vite frontend:

- **Fleet overview** — nodes, capacity, conditions, AZ distribution, Bottlerocket version
- **Node/workload topology** — which pods sit on which nodes, with owner chains
- **Environment and permissions panel** — cluster identity, per-kind collection status, RBAC
  probe results, build info
- **Live event stream** — real Kubernetes events as they arrive

Provenance and observation timestamps are visible on every fact, everywhere.

**Acceptance.**
- `kubectl scale deploy/demo --replicas=5` run externally appears with no page refresh and **no
  frontend polling timer**. Verified by comparing the UI's resourceVersion against `kubectl`.
- Disconnected, forbidden, loading, **stale**, and empty states each render correctly and each is
  reachable in a test. A `403` on PDBs renders as *forbidden*, never as "no PDBs".
- Fixture mode is labelled `FIXTURE` in persistent chrome that cannot be dismissed.
- A test asserts no secret material appears in any response body or log line.

**Non-goals.** Any mutation. Any AWS resource. Analysis.

**Limitations.** Single cluster. No FleetForge authentication — binds to `127.0.0.1`.

---

## M3 — Preflight engine and recommendation summary ⬜ · ~12 days

**Scope.** `(ClusterSnapshot, MaintenanceRequest) -> PreflightResult`. Pure, deterministic, no I/O.

Analyzers, in build order:

| Order | Analyzer | Confidence |
| --- | --- | --- |
| 1 | PDB blocks voluntary disruption | Certain |
| 2 | Singleton workload | Certain |
| 3 | Unmanaged pod (no owner) | Certain |
| 4 | Insufficient aggregate CPU after removal | Heuristic — necessary, not sufficient |
| 5 | Insufficient aggregate memory after removal | Heuristic — necessary, not sufficient |
| 6 | Node selector prevents placement | Likely |
| 7 | Missing toleration | Likely |
| 8 | Required node affinity unsatisfiable | Likely |
| 9 | Local or node-bound storage | Certain |
| 10 | Multi-node removal / AZ concentration | Heuristic |

Every finding carries a stable id, severity, exact affected resources with UID and
resourceVersion, evidence, calculation, explanation, remediation, confidence, **limitations**, and
its source snapshot id.

**Recommendation summary** — the lightweight rollout guidance, emitted by the preflight engine as
part of `PreflightResult`. It is a calculation, not a controller, and owns no state (ADR-0016):

```
status                    Safe | Blocked
recommended_max_concurrency   integer, with the constraint that produced it
affected_workloads        exact workloads, with predicted disruption per workload
evidence                  the findings behind the status
predicted_impact          pods evicted, capacity headroom after, PDB margin
```

**UI.** Preflight workspace — select nodes on the topology, adjust concurrency, watch the risk
recompute immediately. Evidence drawer behind "Show evidence" on every finding. Current-versus-
proposed comparison. Full keyboard navigation, accessible colour, responsive layout.

**Acceptance.**
- Every analyzer has at least two fixtures: one it flags, one it clears.
- A capacity finding states, in its own `limitations`, that aggregate capacity does not prove
  schedulability.
- Results are labelled `WHAT-IF` with their snapshot id, never as something that occurred.
- Adjusting concurrency recomputes from the same snapshot and is deterministic.

**Non-goals.** Wave sequencing (ADR-0011). Executing anything (ADR-0012).

**Limitations.** No scheduler predicate evaluation. Published in every affected finding.

---

## M4 — Brupop observation, event log, and evidence report ⬜ · ~8 days

**Scope.** Collect Brupop custom resources and status alongside cluster state — the same
normalization and the same provenance discipline as every other fact.

- **Event log** — append-only JSONL: normalized observations, snapshots, preflight results,
  recommendation summaries, Brupop state transitions, and workload health. Inspectable with
  `jq` by a reviewer who does not trust us (ADR-0018). *(D4)*
- **Evidence report** — exported Markdown + JSON: what was proposed, what FleetForge predicted,
  the evidence behind each finding, what Brupop actually did, and the timeline. *(D5)*
- **Prediction-versus-actual** — score predicted disrupted workloads and predicted timing against
  observed eviction and recovery from the event log. Publish the accuracy, including the misses.
  *(D6)*
- **UI** — live execution timeline driven by real Brupop and Kubernetes events, and a
  prediction-versus-actual view.

**Acceptance.**
- The event log reconstructs the full timeline offline, with no live connection.
- The report's every claim traces to an event-log entry with a timestamp and a resourceVersion.
- The comparison names what FleetForge got wrong, not only what it got right.

**Non-goals.** Interactive replay scrubbing (ADR-0014). FleetForge submitting the update itself —
at this milestone a human applies the Brupop CR after FleetForge reports safe (ADR-0012).

---

## M5 — Ephemeral EKS run and recorded demonstration ⬜ 🔒 · ~10 days

Every step requires explicit approval in the session. Nothing here runs unattended.

**Scope.** Terraform for a deliberately small EKS cluster with a Bottlerocket managed node group.
Helm charts for FleetForge and Brupop. A demo application under continuous traffic. The recorded
end-to-end run. *(D1, D2, D3, D7)*

**Sequence, each gate explicit:** read-only AWS identity inspection → confirm account, profile,
and region → shown `terraform plan` with a cost summary → your approval → apply → Helm install →
the run → **documented, manually-run teardown**.

Tags on every resource: `Project=FleetForge`, `Environment=dev`, `ManagedBy=Terraform`,
`Owner=Param`.

**The demonstration.** Ten steps, in `DEMO.md`, with `kubectl` visible throughout.

**Acceptance.**
- Nothing presented as `LIVE` is mocked, cached beyond its stated freshness, or hard-coded.
- The environment stands up and tears down reproducibly from a clean checkout.
- The recording, event log, evidence report, and comparison are all captured before teardown —
  the cluster is disposable, the artifacts are not.

**Cost.** Approximate, to be verified against live pricing at plan time: EKS control plane
~$0.10/hr, three `m6g.large` Bottlerocket nodes ~$0.077/hr each — roughly **$8/day**. The
environment exists for the demo and is destroyed afterwards.

---

## Deferred, deliberately

Not "later, maybe" — each has an ADR stating the trigger that would justify building it.

| Subsystem | ADR | Evidence that would justify it |
| --- | --- | --- |
| Wave planner | [0011](docs/adr/0011-defer-wave-planner.md) | Maintenance across more nodes than a human will sequence by hand |
| Execution controller | [0012](docs/adr/0012-defer-execution-controller.md) | An executor Brupop does not cover, and a real second user |
| Host-observability agent | [0013](docs/adr/0013-defer-host-agent.md) | A prediction miss the API server could not have explained |
| Replay engine | [0014](docs/adr/0014-defer-replay-engine.md) | The JSONL log proving insufficient for post-incident review |
| Chaos framework | [0015](docs/adr/0015-defer-chaos-framework.md) | Analyzers passing tests but missing real-world failures |

Deferring these is a judgement, not an omission. An unfinished wave planner is worse evidence of
engineering ability than a written decision not to build one yet.

---

## Testing strategy

| Layer | Approach |
| --- | --- |
| `ff-core` | Unit tests; property tests for snapshot hash canonicality and provenance invariants |
| `ff-preflight` | Table-driven over fixtures — every analyzer needs one fixture it flags and one it clears, plus adversarial cases |
| `ff-collect` | Kind integration in CI; fixture replay locally |
| `ff-api` | JSON schema contract tests; secret-leak assertions; forbidden / stale / empty coverage |
| `web/` | `tsc --noEmit`, Vitest, Playwright over every UI state in fixture mode |
| Invariants | proptest: fixture data can never be emitted as `Live`; preflight results are always `WhatIf`; preflight is deterministic for a given snapshot; a stale snapshot cannot be presented as current |
| Security | `cargo deny`, `cargo audit`, `npm audit`, gitleaks, container scanning |

The ten execution invariants in `ARCHITECTURE.md` §7 are written and property-tested **before**
any execution adapter exists, not after — they are the gate on ADR-0012 being revisited.

Heavy integration — Kind, EKS, Bottlerocket, Brupop — runs in CI or the ephemeral AWS
environment, never as a prerequisite for local development.

## Local development strategy (Apple M1, 8 GB, 35 GB free)

- **Fixture-first.** `ff-preflight` is a pure function over recorded snapshots. Most development
  needs no cluster, no Docker, no memory pressure.
- **Kind only when required** — control plane plus two workers, capped at ~2 CPU / 4 GB, torn
  down after use. Fixtures captured once and reused offline.
- **Disk is the binding constraint, not RAM.** Shared workspace `target/`, `cargo check` in the
  inner loop, and `target/` pruned first when space runs short.
- **No local observability stack.** `tracing` to stdout. Prometheus and OTel live in the cluster
  environments, not on the laptop.
- **Anything heavy runs in CI** — cross-architecture builds, Kind integration, image scanning.
