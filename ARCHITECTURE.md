# FleetForge — Architecture

Status: **proposed** (Milestone 0). Nothing below is implemented yet.

## 1. Shape of the system

```
                 ┌───────────────────── fleetforge control plane (one Rust binary) ────────────────────┐
                 │                                                                                     │
 Kubernetes API  │  ff-collect        ff-core            ff-preflight      ff-planner                  │
   (watch, RO) ──┼─▶ watchers ─▶ ClusterSnapshot ───────▶ analyzers ─▶ Findings ─▶ Plan (waves)         │
                 │  reflectors   (immutable, hashed)          │              │                         │
                 │      ▲              │                      │              │                         │
  ff-agent       │      │              ▼                      ▼              ▼                          │
  (DaemonSet) ───┼──────┘        ff-record (events, snapshots, findings, plans, approvals, outcomes)   │
   host signals  │                     │                                                               │
                 │                ff-store (trait) ── SQLite now, PostgreSQL later                     │
                 │                     │                                                               │
                 │  ff-api (axum): REST + SSE ─────────┐        ff-exec (adapter trait)                │
                 └─────────┬───────────────────────────┴────────────────┬──────────────────────────────┘
                           │                                            │  M5+, approval-gated,
                      web/ (React + TS + Vite)                          │  separate credential
                                                                        ▼
                                             Brupop CRs · cordon/drain · EKS managed node group
```

One control-plane process. One optional node-local agent. No message broker, no cache tier, no
sidecars. The API server is the source of truth; putting Kafka or Redis in front of a watch
stream adds failure modes without adding capability.

## 2. Component boundaries

Boundaries are enforced by the crate dependency graph, not by convention.

### `ff-core` — domain model · *depends on: serde, chrono, thiserror*
Types only. `ClusterSnapshot`, `NodeFact`, `PodFact`, `PdbFact`, `Provenance`, `Finding`,
`Plan`, `Approval`, `FleetForgeError`. No I/O, no kube-rs, no tokio. This is why analyzers can be
property-tested without a cluster.

### `ff-collect` — cluster collector · *depends on: ff-core*
Owns **every** call to the Kubernetes API. kube-rs watchers/reflectors for Nodes, Pods,
Deployments, StatefulSets, DaemonSets, ReplicaSets, PDBs, Events, and (M6+) Brupop CRs.
Normalizes to `ff-core` facts, stamps provenance, emits immutable content-hashed
`ClusterSnapshot` values, and reports **collection status** per resource kind
(`Syncing | InSync { at } | Forbidden { verb, resource } | Degraded { since, error } | Stale { age }`).

**No other crate may construct a Kubernetes client.** That single rule turns "read-only mode
cannot mutate the cluster" from an aspiration into something a test can assert.

Also implements `FixtureSource`: loads a recorded snapshot from `fixtures/`, stamps every fact
`FIXTURE`. Same types, same downstream path, different provenance.

### `ff-preflight` — analysis engine · *depends on: ff-core*
Pure: `(ClusterSnapshot, MaintenanceRequest) -> Vec<Finding>`. Zero I/O, therefore deterministic
and exhaustively unit-testable. Each analyzer implements one trait and is registered in a table;
adding a check never edits an existing one.

### `ff-planner` — maintenance planner · *depends on: ff-core, ff-preflight*
Pure: `(ClusterSnapshot, Vec<Finding>, PlanRequest) -> Plan`. Same snapshot hash + same request
⇒ byte-identical plan. That determinism is what makes an approval token meaningful.

### `ff-agent` — host-observability agent · *separate binary, depends on: ff-core*
Optional DaemonSet. Reads cgroup v2 CPU/memory pressure (PSI), filesystem pressure, kubelet
`/healthz`, containerd state, pod termination timing, reboot/recovery duration, kernel and OS
metadata, and Bottlerocket version/update state. Reports to the control plane over mTLS.

Privilege posture (justified per-capability in `THREAT_MODEL.md`): read-only host mounts, no
`privileged: true`, no host network unless a signal provably requires it, drop all capabilities
by default. eBPF is explicitly out of scope until the base product is reliable, because it
changes the privilege story completely.

### `ff-exec` — execution adapters · *depends on: ff-core* · M5+
Defines `ExecutionAdapter` and the approval/verification protocol. Milestones 0–4 ship exactly
one implementation: `RecommendOnly`, which returns the intended mutation and performs none.
Destructive adapters (cordon/drain, Brupop, node-group replacement) arrive behind explicit cargo
features *and* runtime allow-lists.

### `ff-record` — recorder and replay · *depends on: ff-core, ff-store*
Persists normalized observations, input snapshots, findings, plans, approvals, execution events,
workload-health measurements, and predicted-versus-actual outcomes. Serves replay as a timeline.
Replay sessions are constructed with no adapter handle at all — the type system, not a flag,
prevents replay from mutating anything.

### `ff-store` — persistence · *depends on: ff-core*
A `Store` trait; SQLite (`sqlx`) first, PostgreSQL later as a swap rather than a rewrite. Audit
records are append-only by contract.

### `ff-api` — HTTP surface and main binary · *depends on: all of the above*
Axum. REST for queries and commands, SSE for the live stream. Owns configuration, tracing/OTel,
Prometheus metrics, timeouts, cancellation, graceful shutdown, and the redaction layer that keeps
kubeconfig contents, bearer tokens, certificates, and AWS account identifiers out of every
response and every log line.

### `web/` — operator interface
React + TypeScript + Vite. TypeScript types generated from the Rust models, so a provenance label
cannot be silently dropped in the frontend.

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
    pub source: Source,                 // KubeWatch{group,version,kind} | Agent{node} | Fixture{file} | Computed{from: SnapshotId}
    pub collection: CollectionStatus,   // Syncing | InSync | Forbidden | Degraded | Stale
}

pub struct ClusterSnapshot {
    pub snapshot_id: SnapshotId,        // content hash — identity IS the content
    pub taken_at: DateTime<Utc>,
    pub mode: Mode,
    pub cluster_id: String,
    pub nodes: Vec<NodeFact>,
    pub pods: Vec<PodFact>,
    pub workloads: Vec<WorkloadFact>,   // Deployment | StatefulSet | DaemonSet | ReplicaSet
    pub pdbs: Vec<PdbFact>,
    pub events: Vec<EventFact>,
    pub coverage: Vec<KindCoverage>,    // per-kind collection status — what we could NOT see
}
```

`NodeFact` carries name, UID, labels, taints, conditions, allocatable/capacity, AZ and instance
type, cordon state, Bottlerocket version where available, and Brupop state where available.
`PodFact` carries owner reference chain, phase, node assignment, per-container requests/limits,
node selector, tolerations, affinity/anti-affinity, topology-spread constraints, volume kinds
(to detect node-bound storage), priority class, and termination grace period.

`coverage` matters as much as the facts. A snapshot that could not list PDBs because RBAC
forbade it must never be analyzed as if there were no PDBs.

A `Finding` must stand alone in an exported report:

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
    pub snapshot_id: SnapshotId,          // the exact state this was computed against
}
```

`limitations` is required, not optional. A capacity analyzer that cannot evaluate scheduler
predicates says so in every finding it emits.

## 4. Initial API contract

Milestone 2 (live read-only slice):

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/v1/environment` | mode, cluster id, RBAC probe results, collection status, build info |
| `GET` | `/api/v1/snapshot` | current `ClusterSnapshot` |
| `GET` | `/api/v1/nodes`, `/pods`, `/workloads`, `/pdbs` | filtered views |
| `GET` | `/api/v1/events` | normalized Kubernetes events |
| `GET` | `/api/v1/stream` | **SSE**: `snapshot.updated`, `node.changed`, `pod.changed`, `event.observed`, `collection.status`, `heartbeat` |
| `GET` | `/healthz`, `/readyz`, `/metrics` | liveness, readiness, Prometheus |

Later: `POST /api/v1/preflight` (M3) · `/plans`, `/approvals` (M4) · `/executions`, `/audit`
(M5) · `/replay`, `/accuracy` (M7).

Every response envelope carries `mode`, `cluster_id`, `observed_at`, and `collection`. There is
no unlabeled data path — an endpoint that cannot state its mode is a bug.

Errors are structured: `{ code, message, retriable, details }`. `403` from Kubernetes surfaces as
a first-class `Forbidden` state naming the verb and resource, never as an empty list.

## 5. Live transport: SSE, not WebSockets

Data flows one way, server → browser. SSE gives automatic reconnection with `Last-Event-ID`,
passes through proxies, and is a plain `GET` for authorization purposes. Commands travel over
ordinary POSTs where they receive normal authorization and audit treatment. See ADR-0007.

The UI renders from watch-driven events. There are no frontend polling timers standing in for
liveness; a stale stream is displayed as **stale**, not as calm.

## 6. Permission tiers

Five separate capabilities, never one blanket role:

| Tier | Grants | Kubernetes identity |
| --- | --- | --- |
| Discovery & analysis | read cluster state, run preflight | read-only ServiceAccount |
| Plan generation | create plans | same read-only identity |
| Plan approval | approve a specific plan hash | no cluster access at all |
| Controlled execution | apply allow-listed mutations | separate, narrowly scoped ServiceAccount |
| Administration | configure clusters, allow-lists, retention | no cluster access |

The execution identity does not exist in the process until an approved plan requires it.

## 7. Execution protocol (designed now, built at M5)

No mutation occurs without all seven, in order:

1. **Plan** — deterministic, immutable, content-hashed.
2. **Diff** — the literal patches, rendered for review.
3. **Risk summary** — blockers, disrupted workloads, capacity headroom.
4. **Approval** — a human approves one `plan_hash`; the token is bound to that hash and is
   invalid for any other plan, including a recomputed identical-looking one.
5. **Allow-list check** — resource kind *and* field path must be explicitly permitted.
6. **Execution** — concurrency clamped at the executor, not in the UI; snapshot freshness
   re-checked before each wave.
7. **Postcondition verification** — re-read from the API server, confirm, record.

Invariants, each property-tested:

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

## 8. Deliberate deferrals

- **High-fidelity scheduling.** Running upstream scheduler predicates — via the scheduler
  framework or a throwaway control plane — is the honest way to answer "will it actually fit."
  It is also a large subsystem. V1 ships approximations labeled as approximations, and M7 scores
  them against reality.
- **eBPF and deep tracing.** Changes the agent's privilege story; not before the basics work.
- **Multi-cluster.** `cluster_id` is in the model from day one; the collector runs one cluster.
- **FleetForge's own authN/authZ.** M2 binds to localhost. Real auth lands *before* the first
  mutating endpoint exists (M5), not after.
