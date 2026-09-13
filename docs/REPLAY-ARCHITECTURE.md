# Replay architecture and data flow

How a directory of captured evidence becomes an interactive control room, and
why each boundary is where it is.

## The shape of it

```
evidence/eks-recovery/          37 files, 6.3 MB, committed
  ├── 35-…-events-complete.jsonl   5,068 append-only events (the timeline)
  ├── 03-preflight-before.json     FF-PDB-001 with its calculation
  ├── 04-pdb-before.json           the cluster's own words about the PDB
  ├── 01/13/29-snapshot-*.json     full ClusterSnapshots at three points
  └── 31 more artifacts            node/pod/Brupop state, controller logs, traffic
          │
          │  ff-replay::ReplayBundle::load()
          ▼
  ┌──────────────────────────────────────────────────────────────┐
  │ ff-replay                                                    │
  │   schema.rs     versioned types, ClaimBasis                  │
  │   bundle.rs     parse + validate; rejects incomplete bundles │
  │   state.rs      ReplayTimeline::state_at() — a pure fold     │
  │   chapters.rs   derived timeline marks                       │
  │   chain.rs      the investigation chain, facts vs arrows     │
  │   artifacts.rs  manifest addressing + redaction              │
  └──────────────────────────────────────────────────────────────┘
          │  DataSource::Replay(Arc<ReplayBundle>)
          ▼
  ┌──────────────────────────────────────────────────────────────┐
  │ ff-api            10 endpoints under /api/v1/replay/         │
  │                   every response in an Envelope{ mode }      │
  └──────────────────────────────────────────────────────────────┘
          │  HTTP, loopback only
          ▼
  ┌──────────────────────────────────────────────────────────────┐
  │ web               useReplayBundle() — the fixed parts, once  │
  │                   usePlayback()     — position + clock       │
  │                   renders what the API says. Computes nothing│
  └──────────────────────────────────────────────────────────────┘
```

For the demonstration there is no second process: `ff-api` also serves the built
interface from `web/dist` at `/` (`--ui`, see `crates/ff-api/src/ui.rs`), so one
binary on one loopback port answers both `/` and `/api/v1/`. `scripts/demo.sh`
wires that up, checks readiness, refuses a non-loopback bind, and takes the
whole process group down on Ctrl-C.

Unknown paths under `/api/` return a JSON 404 rather than falling through to the
single-page fallback — otherwise a client asking for a mistyped endpoint gets
`200 text/html`, which looks like success and parses as neither JSON nor an
error.

## Why replay is its own data source

`DataSource` has three variants and replay is one of them, not a flag on
fixture:

```rust
pub enum DataSource {
    Live(Arc<Collector>),
    Fixture(Arc<ClusterSnapshot>),
    Replay(Arc<ReplayBundle>),
}
```

They make three different claims about reality — *this is happening*, *this
happened*, *this never happened* — and a shared code path is how one gets
rendered as another. `ff-replay` does not depend on `ff-collect` and cannot
construct a Kubernetes client, so there is no path by which a replay process
reaches a cluster.

The consequence a reader can check: `cargo test -p ff-api` asserts that a
fixture process returns `404 not_replaying` for every replay endpoint, and that
a replay process has no snapshot store, no coverage, and reports
`connection_state: "not applicable"`.

## Why the fold lives in Rust

`ReplayTimeline::state_at(position)` replays events `0..=position` into a
`ReplayState`. It is pure: no clock, no randomness, no interpolation, no
network. Same bundle, same position, same bytes.

The alternative — ship the bundle to the browser and fold it there — is what a
demo would normally do, and it is how the interface stops being accountable.
Once the browser owns the fold, every claim on screen is a claim about
JavaScript rather than about the recording. See
[ADR-0025](adr/0025-replay-state-is-computed-in-rust.md).

Cost of the decision: scrubbing is a round-trip. Measured at **under 1 ms** for
the full 5,068-event fold on an M1 Air, so nothing was bought with this except
correctness. When a request is outstanding the interface says so rather than
showing the previous position under the new timestamp.

## What is derived versus what is written

Nothing on the screen is a number somebody typed into a template.

| Shown | Derived from |
| --- | --- |
| Capture window | first and last event timestamps |
| Brupop's start (14:08:15) | oldest `first_seen_at` in the Brupop namespace, from `01-snapshot-before.json` |
| "7m 32s before recording" | that timestamp minus the first event's |
| Chapter positions | the first index at which a checkable condition became true |
| FF-PDB-001 arithmetic | `/data/findings` in `03-preflight-before.json` |
| Chain fact values | the fold at the end of the first snapshot, plus `04-pdb-before.json` |
| Prediction verdicts | `ff_record::score()` re-run over the event log |
| 30.2% | counted from `31-traffic-post-recovery.txt` |
| Bottlerocket versions in the banner | versions seen in events, excluding the known-bad `2.0.0` |

A bundle in which Brupop started *after* recording would render the opposite
sentence rather than keep this one. A bundle with one cordoned node would say
"1 of 3". That is the test: change the evidence, and the prose changes with it.

## Claim classification

Every statement carries one of five bases, defined in `schema.rs`:

| Basis | Label | Means |
| --- | --- | --- |
| `ObservedByFleetForge` | `OBSERVED` | FleetForge watched it; it is in the event log |
| `MathematicallyDerived` | `DERIVED` | computed by a formula the interface shows you |
| `HumanRca` | `HUMAN RCA` | a person worked it out afterwards |
| `UnverifiedHypothesis` | `UNVERIFIED` | plausible, never tested |
| `Unavailable` | `NO EVIDENCE` | the data needed does not exist |

`ClaimBasis::is_evidence()` returns true for exactly the first two. The
interface styles those as solid and the other three as filled or dashed, but the
word is what carries it — colour is never the only signal.

The investigation chain is the sharpest case: its five **facts** are observed or
derived, and all four **arrows between them** are `HumanRca`. Every
incident-review diagram ever drawn uses one uniform arrow, which silently grants
a causal claim the same standing as a measurement. Here they are drawn
differently and tested to stay that way.

## Artifact access

Artifacts are addressed by **manifest name**, never by path fragment:

```rust
GET /api/v1/replay/artifacts/04-pdb-before.json   → 200
GET /api/v1/replay/artifacts/../../etc/passwd     → 404
GET /api/v1/replay/artifacts/./04-pdb-before.json → 404
```

A name not in the manifest returns the same 404 as a missing file, deliberately,
so the endpoint cannot be used to probe the filesystem. The store canonicalizes
on index, excludes dotfiles, truncates oversized reads and says so, and runs
every response through `redact()` over a fixed key list (tokens, certificate and
key data, AWS credentials, passwords). Redaction replaces the *value* and leaves
the key — deleting the line would hide that the artifact contained one.

Two tests keep this honest: a planted credential in a copied bundle must come
back `[REDACTED]` through the HTTP path, and every one of the 37 real artifacts
is swept for `AKIA`, `ASIA`, private-key headers and `client-key-data`.

## Mode integrity

Three independent guards, because this is the claim everything else rests on:

1. **Rust.** Every replay response is wrapped in `Envelope::new(Mode::Replay, …)`.
   There is no code path that constructs one with another mode.
2. **Test.** `no_replay_endpoint_can_report_live` walks every endpoint and
   asserts `mode == "replay"` and `mode_label == "REPLAY"`.
3. **Browser.** `getEnvelope()` throws if the mode is anything but `replay`, and
   the screen renders the error instead of the data. An E2E test rewrites a
   response to `mode: "live"` and asserts the interface refuses it.

The banner is sticky, has no dismiss control, and stays in the viewport at the
bottom of the page — a screenshot of the fleet panel has to carry the label too.
