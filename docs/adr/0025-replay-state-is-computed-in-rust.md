# ADR-0025: Replay state is computed in Rust, never in the browser

**Status:** accepted
**Date:** 2026-09-13

## Context

The captured EKS incident is a 5,068-event JSONL log plus 36 supporting artifacts. The
control room has to answer "what did the cluster look like at 14:35:11?" for any position a
viewer scrubs to, and it has to answer it the same way twice — a leadership demonstration is
rehearsed once and performed once, and a timeline that drifts between the two is a timeline
nobody can point at.

Two obvious implementations:

1. Ship the evidence bundle to the browser and fold it there. One process, no API surface,
   instant scrubbing.
2. Fold it in Rust and serve `state_at(position)` over HTTP.

Option 1 is what a demo would normally do. It is also how the interface stops being
accountable: once the browser owns the fold, the browser owns the truth, and every claim on
screen is a claim about JavaScript rather than about the recording.

## Decision

The replay engine lives in `ff-replay`, a crate that cannot construct a Kubernetes client and
does not depend on `ff-collect`. `ReplayTimeline::state_at` is a pure fold — no clock, no
randomness, no interpolation. The browser fetches state by position and renders the answer.

`ff-replay` is a third data source alongside live and fixture, not a flag on fixture. The
three make different claims about reality: *this is happening*, *this happened*, *this never
happened*. Sharing a code path between them is how one gets rendered as another.

Artifacts are addressed by manifest name, never by path fragment, and pass through a redactor
before leaving the process. Traversal attempts return the same 404 as a missing file, so the
endpoint cannot be used to probe the filesystem.

## Consequences

**Good.** Determinism is testable and tested: the same bundle and position produce identical
bytes, asserted in `crates/ff-api/tests/replay_api.rs`. The mode label cannot be forged —
every replay response carries `Mode::Replay` and a test pins it. Scrubbing costs one request
and the full 5,068-event fold measures under a millisecond, so nothing was bought with this
except correctness.

**Good.** Editorial content is derived rather than authored. Chapter marks are the positions
at which a checkable condition first became true; the opening narration composes Brupop's
start time from Kubernetes events in the captured snapshot. A future bundle where FleetForge
started recording *before* Brupop would invert that sentence rather than leave it wrong.

**Bad.** Scrubbing is a network round-trip, so a dropped backend freezes the timeline. The
interface reports that state explicitly rather than continuing to render the last position as
if it were current.

**Bad.** Two implementations of the event model now exist — Rust structs and TypeScript
interfaces. They are kept in step by the E2E suite, which exercises real responses through a
real browser, not by a code generator.

## Alternatives rejected

**Fold in the browser.** Rejected above.

**Pre-compute every position.** 5,068 snapshots of full cluster state, most of them identical
to their predecessor. Larger than the evidence bundle it describes, and it would still need
the fold to build.

**Interpolate between events to smooth playback.** Rejected outright. Seventeen quiet minutes
replay as seventeen quiet minutes. Inventing intermediate cluster states to make a demo look
lively is fabricating Kubernetes activity, which is the one thing this project must not do.
