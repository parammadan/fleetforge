# ADR-0023 — A watch is silent for two different reasons, so probe the connection

**Status:** Accepted · 2026-09-12

## Context
A watch-based collector receives events only when something changes. That is the
point, and it is also a hole: **silence means either "the cluster is quiet" or
"the connection is dead", and nothing in the stream distinguishes them.**

This was not theoretical. The first version tracked freshness as "time since the
last watch event" and looked correct. Pausing the control-plane container —
`docker pause`, which freezes the processes without closing any socket — and
querying FleetForge twenty seconds later returned `authoritative=true`, every
kind `in_sync`, the full inventory intact. The API server was unreachable and
the interface said everything was fine.

Two things caused it. Nothing had changed just before the pause, so there was no
event to miss. And a paused server leaves the TCP connection open rather than
resetting it, so the client raised no error; it simply waited. Kubernetes watch
timeouts run into the minutes, so nothing would have surfaced for a long while.

Follow that through: an unreachable API server showing stale data as current
means a PodDisruptionBudget list frozen at a moment that has passed. Stale PDB
data reads as "no blockers", which reads as **safe to drain**. The most
dangerous state this product can enter was reachable through a plausible
implementation and a silent network.

## Decision
Probe the API server independently of the watches. Every 5 seconds the collector
calls the version endpoint with a timeout. Two consecutive failures mark the
connection unreachable.

Reachability is tracked **separately** from per-kind watch status, and composed
at the edge:

- A successful probe refreshes the freshness anchor on every already-current
  kind. That is what the probe actually proves: the connection works and the
  watches are established, so what they last reported still stands.
- While unreachable, any kind reporting `InSync` is downgraded to `Stale`.
- `authoritative` requires reachability **and** per-kind authority. Perfect
  coverage collected from a cluster we can no longer reach is not authoritative,
  however healthy the individual statuses look.
- Observed counts are **retained**, never zeroed. Whatever was last seen is
  still the last thing seen; the status is what marks it unusable. Zeroing would
  reintroduce the exact "no PDBs, therefore safe" failure.

## Consequences
- Verified with `scripts/resilience-demo.sh`: during a paused control plane,
  `authoritative=false` and PDB status is `stale` with the count preserved;
  after unpause both return to `in_sync` without a restart.
- One extra API call every 5 seconds per FleetForge instance. Negligible, and
  it is a `get` on a non-namespaced version endpoint requiring no extra RBAC.
- The probe timeout must be bounded. A frozen server accepts the connection and
  never answers, so an unbounded call would hang the probe task and stop the
  very detection it exists to perform.
- Two failures before declaring unreachable, so a single blip does not flip the
  whole interface into a failure state.

## Alternatives rejected
- **Shorten the staleness budget.** Treats the symptom. The budget was 30s and
  the outage was 20s; at 10s the same test with a 5s outage would have passed
  just as falsely. No budget fixes an ambiguity in the signal.
- **Rely on watch errors.** They arrive eventually. "Eventually" against a
  frozen socket is minutes, during which the interface is confidently wrong.
- **Periodically re-list every kind.** Would work and costs vastly more, since
  it re-fetches every object to answer a yes/no question about connectivity.
