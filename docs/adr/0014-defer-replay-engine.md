# ADR-0014 — Defer the interactive replay engine

**Status:** Accepted · 2026-09-12

## Context
Replaying a maintenance operation on an interactive timeline — scrub, pause, inspect cluster
state at any moment — is genuinely valuable for post-incident review, and it is how the
demonstration survives the cluster being destroyed.

But replay splits into two things. **Recording** is the durable artifact and a required
deliverable. **Interactive playback** is a UI feature over that artifact: a timeline component,
state reconstruction at arbitrary timestamps, and a second rendering path through every screen
that must be provably incapable of touching an execution adapter.

## Decision
Build the recording. Defer the playback UI.

`ff-record` writes an append-only JSONL event log (ADR-0018) capturing observations, snapshots,
preflight results, recommendation summaries, Brupop transitions, and workload health. The
evidence report and the prediction-versus-actual comparison are generated from that log, offline,
with no live connection. There is no timeline scrubber.

## Consequences
- The valuable artifact survives cluster teardown. A reviewer can `jq` the log and check every
  claim in the report independently — which is a stronger demonstration of rigour than a scrubber.
- No second rendering path, so the `REPLAY` mode plumbing stays unexercised and the risk of
  showing recorded data as live never materializes in the slice.
- Post-incident review is command-line, not visual. Fine for one operator.
- The event log schema is designed so playback can be added without re-recording anything.

## Evidence that would justify building it
1. The JSONL log proving insufficient in a real post-incident review — someone needing to see
   cluster state at a moment, not a sequence of deltas.
2. More than one person reviewing runs, where a shared visual timeline beats everyone running `jq`.
3. Enough recorded runs to make a comparison view worth navigating.
4. The `REPLAY` provenance path property-tested, including the invariant that a replay session
   holds no adapter handle.

## Alternatives rejected
- **Skip recording too.** Then the demonstration dies with the cluster, and the
  prediction-versus-actual comparison is impossible. Not an option.
- **Record to SQLite with a query API now.** More capable, and the slice needs an inspectable file
  rather than a query surface. See ADR-0018.
