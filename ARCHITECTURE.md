# FleetForge — Architecture

Status: **proposed** (Milestone 0). Nothing below is implemented yet.

Scope note: this describes the architecture of the **vertical slice** — the read, analyze,
recommend, observe, report path. Subsystems deferred by ADRs 0011–0015 are shown in the diagram
as dashed, and are documented here only to the extent that the slice must not preclude them.

## 1. Shape of the system

```
              ┌──────────────────── fleetforge control plane (one Rust binary) ──────────────────┐
              │                                                                                  │
Kubernetes    │  ff-collect          ff-core             ff-preflight                            │
API (RO) ─────┼─▶ watchers ──▶ ClusterSnapshot ──────▶ analyzers ──▶ Findings                    │
Brupop CRs    │   reflectors    (immutable, hashed)          └────▶ RecommendationSummary        │
              │        │                 │                              │                        │
              │        ▼                 ▼                              ▼                        │
              │  ff-record ── append-only JSONL event log ──▶ evidence report · predicted-vs-actual│
              │        │                                                                          │
              │  ff-api (axum): REST + SSE                                                        │
              └────────┬──────────────────────────────────────────────────────────────────────────┘
                       │                              ╎ deferred — ADRs 0011-0015
                  web/ (React + TS + Vite)            ╎ ff-planner · ff-exec · ff-agent · replay UI
                                                      ╎
  the executor:  operator applies Brupop CR ──▶ Brupop cordons, drains, updates, reboots
                                                      │
                                                      └──▶ observed by ff-collect, recorded by ff-record
```

FleetForge reads and recommends. **Brupop executes.** FleetForge then observes what Brupop did and
scores its own prediction against it. No mutating Kubernetes client is constructed anywhere in the
slice, which makes "FleetForge cannot damage the cluster" a structural fact rather than a policy.

One process. No message broker, no cache tier, no sidecars.

## 2. Component boundaries

Enforced by the crate dependency graph, not by convention.

### `ff-core` — domain model · *depends on: serde, chrono, thiserror*
Types only. `Mode`, `Provenance`, `CollectionStatus`, `ClusterSnapshot`, `NodeFact`, `PodFact`,
`WorkloadFact`, `PdbFact`, `EventFact`, `Finding`, `RecommendationSummary`, `FleetForgeError`.
No I/O, no kube-rs, no tokio. This is why analyzers test without a cluster.

### `ff-collect` — cluster collector · *depends on: ff-core*
Owns **every** call to the Kubernetes API (ADR-0008). kube-rs watchers/reflectors for Nodes, Pods,
Deployments, StatefulSets, DaemonSets, ReplicaSets, PDBs, Events, and Brupop custom resources.
Normalizes to `ff-core` facts, stamps provenance, emits immutable content-hashed snapshots, and
reports per-kind collection status:
`Syncing | InSync { at } | Forbidden { verb, resource } | Degraded { since, error } | Stale { age }`.

Also implements `FixtureSource`: loads a recorded snapshot from `fixtures/`, stamping every fact
`FIXTURE` at construction. Same types, same downstream path, different provenance.

**Connection liveness.** A watch is silent both when the cluster is quiet and when the connection
is dead, so `ff-collect` probes the API server independently every 5s (ADR-0023). The probe
outcome is tracked separately from per-kind watch status and composed at the edge:

| Probe result | Effect on an otherwise-current kind | Operator's next step |
| --- | --- | --- |
| `Ok` | stays `InSync`; freshness anchor refreshed | — |
| `Unauthorized` | downgraded to `Degraded` (credential) | reissue the token |
| `Unreachable` | downgraded to `Stale` | check the network |

Observed counts are **retained** through both failures. Whatever was last seen is still the last
thing seen; the status is what marks it unusable. Zeroing them would render as "no
PodDisruptionBudgets", which reads as "no blockers", which reads as safe to drain.

### `ff-preflight` — analysis engine · *depends on: ff-core*
Pure: `(ClusterSnapshot, MaintenanceRequest) -> PreflightResult`. Zero I/O, therefore
deterministic and exhaustively unit-testable. Each analyzer implements one trait and is registered
in a table; adding a check never edits an existing one. Emits `Vec<Finding>` plus the
`RecommendationSummary` (ADR-0016).

### `ff-record` — event log and reports · *depends on: ff-core*
Append-only JSONL (ADR-0018): observations, snapshots, preflight results, recommendation
summaries, Brupop state transitions, workload health. One JSON object per line, each carrying its
provenance and a monotonic sequence number. Generates the evidence report and the
prediction-versus-actual comparison in a single sequential pass, offline, with no live connection.

Retains the `Store` trait from ADR-0005 so SQLite is a swap when execution arrives.

### `ff-api` — HTTP surface and main binary · *depends on: all of the above*
Axum. REST for queries, SSE for the live stream. Owns configuration, tracing/OTel, Prometheus
metrics, timeouts, cancellation, graceful shutdown, and the redaction layer that keeps kubeconfig
contents, bearer tokens, certificates, and AWS account identifiers out of every response and every
log line.

### `web/` — operator interface
React + TypeScript + Vite. TypeScript types generated from the Rust models, so a provenance label
cannot be silently dropped in the frontend.

### Deferred crates
`ff-planner` (ADR-0011), `ff-exec` (ADR-0012), `ff-agent` (ADR-0013). Each has a designed boundary
above and no implementation. `ff-exec` will ship exactly one implementation when it exists:
`RecommendOnly`, which returns the intended mutation and performs none.

## 3. Normalized cluster data model

Every fact carries provenance. This is the spine of the honesty guarantee.

```rust
pub enum Mode { Live, WhatIf, Replay, Fixture }

pub struct Provenance {
    pub mode: Mode,
    pub cluster_id: String,             // stable ID; never the raw API server URL
    pub namespace: Option<String>,
    pub uid: Option<String>,            // Kubernetes object UID
    pub resource_version: Option<String>,
    pub observed_at: DateTime<Utc>,     // when FleetForge saw it
    pub source: Source,                 // KubeWatch{group,version,kind} | Fixture{file} | Computed{from: SnapshotId}
    pub collection: CollectionStatus,
}

pub struct ClusterSnapshot {
    pub snapshot_id: SnapshotId,        // content hash — identity IS the content
    pub taken_at: DateTime<Utc>,
    pub mode: Mode,
    pub cluster_id: String,
    pub nodes: Vec<NodeFact>,
    pub pods: Vec<PodFact>,
    pub workloads: Vec<WorkloadFact>,
    pub pdbs: Vec<PdbFact>,
    pub events: Vec<EventFact>,
    pub brupop: Vec<BrupopFact>,        // Brupop CR state, when the CRDs are present
    pub coverage: Vec<KindCoverage>,    // per-kind collection status — what we could NOT see
}
```

`NodeFact`: name, UID, labels, taints, conditions, allocatable and capacity, AZ, instance type,
cordon state, Bottlerocket version, Brupop state. `PodFact`: owner reference chain, phase, node
assignment, per-container requests and limits, node selector, tolerations, affinity and
anti-affinity, topology-spread constraints, volume kinds (to detect node-bound storage), priority
class, termination grace period.

`coverage` matters as much as the facts themselves. A snapshot that received `403` on PDBs must
never be analyzed as though there were no PDBs.

### Finding

Must stand alone in an exported report:

```rust
pub struct Finding {
    pub id: FindingId,                    // stable, e.g. FF-PDB-001 — linkable and suppressible
    pub severity: Severity,               // Blocker | High | Medium | Low | Info
    pub title: String,
    pub affected: Vec<ResourceRef>,       // exact objects, with UID + resourceVersion
    pub evidence: Vec<Evidence>,          // the raw field values the conclusion rests on
    pub calculation: Option<Calculation>, // inputs, formula, result — reproducible by hand
    pub explanation: String,              // plain language, no hedging
    pub remediation: Vec<Remediation>,    // suggested, never auto-applied
    pub confidence: Confidence,           // Certain | Likely | Heuristic
    pub limitations: Vec<String>,         // what this check does NOT prove
    pub snapshot_id: SnapshotId,
}
```

`limitations` is required, not optional. A capacity analyzer that cannot evaluate scheduler
predicates says so in every finding it emits.

### Recommendation summary

The lightweight rollout guidance — a calculation, not a controller (ADR-0016):

```rust
pub struct RecommendationSummary {
    pub status: MaintenanceStatus,              // Safe | Blocked
    pub recommended_max_concurrency: u32,
    pub concurrency_constraint: ConstraintRef,  // WHICH finding produced that number
    pub affected_workloads: Vec<WorkloadImpact>,
    pub evidence: Vec<FindingId>,
    pub predicted_impact: PredictedImpact,      // pods evicted, capacity headroom after, PDB margin
    pub snapshot_id: SnapshotId,
    pub mode: Mode,                             // always WhatIf
}
```

`concurrency_constraint` is the point. A bare number is a guess with a UI. A number that names the
finding behind it is a claim a reviewer can check.

## 4. API contract

Slice endpoints (M2 unless noted):

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/v1/environment` | mode, cluster id, collection status, connection state, client/server version skew, build info |
| `GET` | `/api/v1/snapshot` | current `ClusterSnapshot` |
| `GET` | `/api/v1/nodes`, `/pods`, `/workloads`, `/pdbs` | filtered views |
| `GET` | `/api/v1/events` | normalized Kubernetes events |
| `GET` | `/api/v1/brupop` | Brupop CR state *(M4)* |
| `GET` | `/api/v1/stream` | **SSE**: `snapshot.updated`, `node.changed`, `pod.changed`, `event.observed`, `brupop.changed`, `collection.status`, `heartbeat` |
| `POST` | `/api/v1/preflight` | snapshot id + candidate nodes + concurrency → `PreflightResult` *(M3)* |
| `GET` | `/api/v1/report/{run_id}` | evidence report, Markdown or JSON *(M4)* |
| `GET` | `/healthz`, `/readyz`, `/metrics` | liveness, readiness, Prometheus |

Every response envelope carries `mode`, `cluster_id`, `observed_at`, and `collection`. There is no
unlabeled data path — an endpoint that cannot state its mode is a bug.

`POST /preflight` is the one non-`GET` in the slice. It mutates nothing: it computes against a
snapshot and returns. It is a POST because the request body carries node selection and concurrency.

Errors are structured: `{ code, message, retriable, details }`. A `403` from Kubernetes surfaces
as a first-class `Forbidden` state naming the verb and the resource, never as an empty list.

## 5. Live transport: SSE, not WebSockets

Data flows one way, server → browser. SSE gives automatic reconnection with `Last-Event-ID`,
passes through proxies, and is a plain `GET` for authorization purposes (ADR-0007).

The UI renders from watch-driven events. There are no frontend polling timers standing in for
liveness; a stale stream displays as **stale**, not as calm.

## 6. Interface surface

Highly interactive, within a read-only product:

| Screen | Interactions |
| --- | --- |
| Fleet overview | Filter and group nodes by AZ, instance type, Bottlerocket version, condition |
| Node/workload topology | Select nodes for maintenance; inspect any pod's owner chain and constraints |
| Preflight workspace | Adjust concurrency and watch risk recompute; current-versus-proposed comparison |
| Evidence drawer | "Show evidence" on every finding — raw field values, the arithmetic, confidence, limitations |
| Recommendation summary | Safe/blocked, recommended concurrency **with the constraint that produced it**, affected workloads, predicted impact |
| Live timeline | Real Kubernetes and Brupop events as they arrive |
| Prediction vs actual | What was predicted, what happened, where FleetForge was wrong |
| Environment & permissions | Cluster identity, per-kind collection status, RBAC probes, data mode |

Accessible colour, full keyboard navigation, responsive layout, and explicit loading,
disconnected, forbidden, stale, error, and empty states. The data mode is persistent chrome that
cannot be dismissed.

## 7. Execution invariants

No execution adapter exists in the slice (ADR-0012). These invariants are nonetheless written and
property-tested now — they are the correctness harness that gates ADR-0012 being revisited:

- A blocked plan cannot be approved.
- Execution cannot begin without an approval.
- An approval applies only to the exact plan hash reviewed.
- Concurrency never exceeds the approved value.
- A failed stop condition prevents all subsequent waves.
- Read-only mode cannot construct a mutating client.
- Replay cannot invoke an execution adapter.
- Fixture data cannot be emitted with `mode: Live`.
- A stale snapshot cannot be executed against.
- Every mutation produces an audit event.

Four of these are testable in the slice today: fixture-never-live, read-only-no-mutating-client,
preflight-always-what-if, and stale-snapshot-never-presented-as-current.

## 8. Deliberate deferrals

Each has an ADR naming the evidence that would justify building it.

| Deferred | ADR | Consequence accepted |
| --- | --- | --- |
| Wave planner | 0011 | Concurrency is recommended, not sequenced or enforced |
| Execution controller | 0012 | A human applies the Brupop CR; FleetForge never mutates |
| Host-observability agent | 0013 | Recovery timing is event-granular, not host-granular |
| Interactive replay | 0014 | Post-incident review is `jq` over JSONL, not a scrubber |
| Chaos framework | 0015 | Unsafe conditions are created with plain `kubectl` manifests |
| High-fidelity scheduling | 0004 | Capacity checks are necessary, not sufficient — published per finding |
| Multi-cluster | — | `cluster_id` is in the model; the collector runs one cluster |
| FleetForge authN/authZ | 0012 | Binds to `127.0.0.1`, single user; a hard prerequisite for any mutation |
