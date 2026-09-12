# ADR-0021 — The local environment is pinned by digest and verified

**Status:** Accepted · 2026-09-12

## Context
"Works on my machine" is the normal failure of local Kubernetes development. A `kind` cluster
created from a floating tag today and next month is two different clusters, and a fixture captured
from one may not match the other.

There is also a subtler failure available here: assuming which node image `kind` uses. Stating
"this is the default image" because it looked like one, and being wrong, would be exactly the kind
of unverified claim this project exists to avoid making.

## Decision
Every image in the local environment is pinned by tag **and** sha256 digest — the `kind` node
image and both demo workload images.

The node image pin was verified two independent ways before the cluster was created:

1. The tag and digest are embedded in the installed `kind` v0.33.0 binary.
2. The published v0.33.0 release notes list that exact tag and digest and state it is the default
   node image for that release.

Both checks are recorded in `infra/local/kind.yaml`. After the cluster comes up,
`scripts/record-images.sh` writes `infra/local/IMAGES.md` with the node architecture and the
`imageID` each node actually resolved — what was really pulled, not what we expected.

## Consequences
- A cluster created from this repository is the same cluster in six months.
- Fixtures can state which Kubernetes version produced them.
- Manifest digests are multi-architecture *index* digests. Each node resolves the manifest for
  its own architecture, so the per-architecture digest differs between an M1 laptop and x86_64
  CI. `IMAGES.md` records what was resolved, which is why observing beats assuming.
- Pins go stale and must be bumped deliberately. That is the point; a floating tag bumps itself
  at the worst moment.
- Verifying a pin costs a few minutes before first use. Cheap against debugging a fixture
  mismatch later.

## Alternatives rejected
- **Floating tags.** Convenient until the day a silent image change breaks a fixture.
- **Tag without digest.** Tags are mutable; a tag pin is a naming convention, not a pin.
- **Trusting the binary alone.** One source. The release notes are the published contract, and
  two sources that agree is a different quality of claim.
