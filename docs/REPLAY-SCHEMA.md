# Replay evidence schema

`REPLAY_SCHEMA_VERSION = 1`. Served on `/api/v1/replay/context` so a consumer can
tell whether it understands the payload before it renders one.

Every endpoint returns an envelope:

```json
{
  "mode": "replay",
  "mode_label": "REPLAY",
  "cluster_id": "1c2cdb4c-…",
  "observed_at": "2026-09-13T15:19:00.342810Z",
  "authoritative": true,
  "data": { … }
}
```

`mode` is always `replay` from a process started with `--replay`. Nothing else
is representable.

## Endpoints

| Endpoint | Returns |
| --- | --- |
| `GET /api/v1/replay/context` | schema version, `CaptureContext`, event counts, `DataCaveat[]` |
| `GET /api/v1/replay/timeline` | significant `ReplayEvent[]` (509). `?all=true` for all 5,068 |
| `GET /api/v1/replay/state?position=N` | `ReplayState` — the fold at `N`, clamped |
| `GET /api/v1/replay/chapters` | `Chapter[]` — derived marks |
| `GET /api/v1/replay/chain` | `InvestigationChain` — facts, arrows, attribution |
| `GET /api/v1/replay/claims` | `Claim[]` — the nine classified statements |
| `GET /api/v1/replay/finding` | `PdbArithmetic` — FF-PDB-001 |
| `GET /api/v1/replay/predictions` | `PredictionRow[]` — predicted vs actual |
| `GET /api/v1/replay/traffic` | `TrafficValidation` — the post-recovery rerun |
| `GET /api/v1/replay/artifacts` | `ArtifactRef[]` — the manifest |
| `GET /api/v1/replay/artifacts/{name}` | one artifact, redacted, with its hash |

## Core types

### `ClaimBasis`

The type the whole interface hangs off.

```rust
pub enum ClaimBasis {
    ObservedByFleetForge,   // "OBSERVED"    — in the event log
    MathematicallyDerived,  // "DERIVED"     — a formula you can check
    HumanRca,               // "HUMAN RCA"   — a person worked it out
    UnverifiedHypothesis,   // "UNVERIFIED"  — never tested
    Unavailable,            // "NO EVIDENCE" — the data does not exist
}
impl ClaimBasis {
    pub const fn is_evidence(self) -> bool {
        matches!(self, Self::ObservedByFleetForge | Self::MathematicallyDerived)
    }
}
```

Serialized in `snake_case`: `observed_by_fleet_forge`, `mathematically_derived`,
`human_rca`, `unverified_hypothesis`, `unavailable`.

### `CaptureContext`

```jsonc
{
  "cluster_id": "1c2cdb4c-c1b2-4cd5-bb54-1132876ab118",
  "cluster_kind": "Amazon EKS with Bottlerocket managed node group (destroyed)",
  "kubernetes_version": "v1.36.4-eks-4cc7921",   // null if unrecoverable
  "client_target_version": "v1.36",
  "bottlerocket_versions": ["1.62.1", "1.64.0"],  // excludes the bogus 2.0.0
  "captured_from": "2026-09-13T14:15:47.607196Z",
  "captured_to":   "2026-09-13T15:19:00.342810Z",
  "nodes": ["ip-10-42-100-186…", "ip-10-42-101-194…", "ip-10-42-101-90…"],
  "brupop_first_seen_at": "2026-09-13T14:08:15Z"  // null if the bundle cannot date it
}
```

`brupop_first_seen_at` is the load-bearing field. It is the oldest
`first_seen_at` among Kubernetes events in the Brupop namespace, read from
`01-snapshot-before.json` — events already in the API server's history when
FleetForge connected, which is why they can testify about a period FleetForge did
not watch. It predates `captured_from`, and that ordering is what makes
"FleetForge did not predict this" a fact rather than modesty. `null` renders as
UNKNOWN; it is never filled in with a plausible time.

### `ReplayEvent`

```jsonc
{
  "index": 731,            // 0-based, dense — the scrubber's coordinate
  "seq": 732,              // from the log, never regenerated
  "at": "2026-09-13T14:15:50.437453Z",
  "kind": "preflight_run", // run_started | snapshot_observed | node_changed |
                           // pod_changed | pod_removed | brupop_state_changed |
                           // preflight_run | kubernetes_event | run_ended
  "summary": "preflight on ip-10-42-100-186 → BLOCKED (16 pods predicted evicted)",
  "significant": true,     // worth stopping on; 509 of 5,068 are
  "raw": { … }             // the captured line, verbatim
}
```

`raw` is the original JSON. Anything the interface shows can be checked against
it without leaving the page.

### `ReplayState`

The fold at a position. Pure, deterministic, clamped at both ends.

```jsonc
{
  "position": 2932,
  "at": "2026-09-13T14:35:11Z",
  "nodes": [ { "name": "…", "ready": true, "unschedulable": false,
               "bottlerocket_version": "1.64.0", "brupop_state": "Idle",
               "brupop_version": "1.64.0", "pods": ["ns/name", …],
               "last_change": "…" } ],
  "pods":  [ { "namespace": "demo", "name": "web-…", "node": "…",
               "phase": "Running", "workload": "web" } ],
  "last_preflight": { "at": "…", "node_names": ["…"], "status": "blocked",
                      "pods_evicted": 5, "findings": ["FF-PDB-001"],
                      "snapshot_id": "5de2639a…" },
  "snapshot_id": "96d5f543…",
  "events_applied": 2933,
  "pending_pods": 1,
  "cordoned_nodes": 1
}
```

`bottlerocket_version` may be `"2.0.0"` early in the capture. That is not a
release — it is the `bottlerocket.aws/updater-interface-version` label, which
FleetForge was reading into the wrong field until the bug was fixed partway
through. It is served as recorded and flagged in the interface. Correcting
captured evidence to look better is the one thing this system must not do.

### `Chapter`

Derived marks, not authored ones: each is the first index at which a checkable
condition became true.

```jsonc
{ "id": "blocker-detected", "title": "FleetForge reports BLOCKED",
  "position": 731, "at": "2026-09-13T14:15:50.437453Z",
  "narration": "…", "basis": "observed_by_fleet_forge" }
```

Chapters that cannot be justified from the log are simply absent. Eight are
derivable from this bundle.

### `InvestigationChain`

```jsonc
{
  "links": [ { "id": "cordons", "label": "Nodes unschedulable",
               "value": "2 of 3 cordoned", "detail": "…",
               "basis": "observed_by_fleet_forge",
               "artifact": "06-nodes-before.json",
               "field_path": ".items[].spec.unschedulable",
               "at": "2026-09-13T14:15:48Z" }, … ],
  "edges": [ { "from": "cordons", "to": "pending",
               "because": "…", "basis": "human_rca" }, … ],
  "attribution": "…FleetForge reported the blocker at the end of the chain; it did not produce the chain."
}
```

Five links, four edges. Every link is `is_evidence()`; every edge is `HumanRca`.
Asserted in `crates/ff-replay/tests/bundle.rs`.

### `PredictionRow`

```jsonc
{ "at": "2026-09-13T14:34:50Z", "node_names": ["ip-10-42-101-194…"],
  "predicted": 5, "observed": 8, "delta": 3,
  "verdict": "under-predicted — more pods were evicted than predicted",
  "class": "under_predicted",
  "missed_workloads": [],
  "window_had_activity": true }
```

`class` is one of `exact`, `conservative`, `under_predicted`, `missed`,
`untested`. **`under_predicted` is never folded into `conservative`**:
over-prediction wastes an operator's caution, under-prediction spends it
somewhere it was needed.

`window_had_activity` is the field that separates two rows reading identically
— predicted 16, observed 0 — into `untested` (nothing was drained at all, so the
prediction was never put to the test) and `conservative` (pods moved, but not the
predicted ones).

### `TrafficValidation`

```jsonc
{ "requests": 139, "successes": 42, "failures": 97,
  "success_pct": "30.2",
  "window": "14:53:24 → 14:58:58",
  "passed": false,
  "interpretation": "Post-recovery networking validation. This run FAILED. …" }
```

Field names say what this is, because a chart labelled "uptime" is read as
uptime no matter what the caption says. The percentage is computed in integer
tenths — `clippy::float_arithmetic` is denied workspace-wide, because a rate that
renders differently on two machines is a rate nobody can audit.

### `ArtifactRef`

```jsonc
{ "name": "04-pdb-before.json", "description": "PodDisruptionBudget state before recovery",
  "kind": "json", "bytes": 3228,
  "sha256": "12b7aa9eb79fb3cdb0c48ac6ba973a19d5f5989117d5d18ca033a5952b9060a2" }
```

`name` is a manifest key and is never joined to a path. `sha256` is the hash the
file had when captured, so a reader can prove the artifact they are looking at is
the one that was recorded.

## Validation on load

`ReplayBundle::load()` rejects rather than degrades:

| Condition | Result |
| --- | --- |
| directory missing | `BundleNotFound` |
| a required artifact absent | `MissingArtifact { artifact }` |
| event log not JSONL | `MalformedArtifact`, naming the file |
| event log empty | rejected — an empty timeline renders as an empty cluster |
| `FF-PDB-001` absent from the preflight | rejected — this bundle is not the incident |
| `web-pdb` has no `status` | rejected — a missing `currentHealthy` must not default to 0 |
| traffic log has no samples | rejected — zero of zero is not 0% |

Each has a test that damages exactly one thing in a copy of the real bundle, so
a failure names what broke.
