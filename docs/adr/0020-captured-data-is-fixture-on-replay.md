# ADR-0020 — Data replayed from disk is FIXTURE, whatever its origin

**Status:** Accepted · 2026-09-12

## Context
The best test fixtures are real. Hand-written Kubernetes JSON reflects what the author imagined
the API server returns; captured JSON reflects what it actually returns, including the fields
nobody remembers exist.

That creates a temptation. This data *came from a real cluster*, so it is real — which slides
easily into presenting it as live, especially in a demonstration where real data is the whole
point.

## Decision
Provenance describes **where a fact is being read from now**, not where it was born. A file on
disk is a file on disk. Anything loaded by `FixtureSource` is `Mode::Fixture`, permanently and
without exception, regardless of having been captured from a live cluster an hour earlier.

This is already structural: `Source::Fixture` permits only `Mode::Fixture` (ADR-0003), the check
runs at construction *and* at deserialization, and a property test covers it. This ADR records
the reasoning so nobody later argues the captured case is special.

## Scrubbing

Captured objects are scrubbed before being committed. `scripts/capture-fixtures.sh` removes:

| Removed | Why |
| --- | --- |
| `managedFields` | Enormous, and records who touched what |
| `kubectl.kubernetes.io/last-applied-configuration` | Embeds whole prior manifests |
| Node `addresses` | Cluster-identifying network detail |
| `machineID`, `systemUUID`, `bootID` | Host-identifying |

UIDs are pseudonymised deterministically rather than deleted: the model needs their shape and
uniqueness, not their real values. The script asserts the scrub afterwards and exits non-zero if
any pattern survives, so an unscrubbed fixture cannot be committed by accident.

## Consequences
- Analyzers are tested against the API server's real output shape, which is where the surprises
  live.
- A fixture can never be demonstrated as a live cluster, even by an author who wants it to be.
- Fixtures drift from reality as Kubernetes versions change. Re-capture is a routine task, and
  the pinned node image (ADR-0021) makes "which version was this?" answerable.
- Scrubbing is lossy. A finding reproduced from a fixture cannot be checked against the original
  cluster by UID. Acceptable: fixtures exist to test analyzers, not to audit history — that is
  the event log's job.

## Alternatives rejected
- **Hand-written fixtures.** No scrubbing needed, and they test the API we imagined.
- **Capturing without scrubbing.** Commits host identifiers and enormous `managedFields` blocks
  into a public repository.
- **A `CapturedLive` mode.** A fifth mode meaning "fixture, but trustworthy" is precisely the
  ambiguity this ADR exists to prevent.
