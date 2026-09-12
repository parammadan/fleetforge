# ADR-0022 — Client/server version skew is reported, not assumed

**Status:** Accepted · 2026-09-12

## Context
The `k8s-openapi` bindings are compiled against one Kubernetes minor version.
At the time of writing, version 0.28 targets at most `v1_36`, while the local
`kind` cluster runs **v1.37.0**. Kubernetes supports a ±1 minor client skew, so
this combination is fine.

"Fine" is doing real work in that sentence, though. Serde silently ignores
unknown fields, so a v1.36 client reading a v1.37 object drops anything new
without a word. For most software that is a non-event. For a tool whose entire
claim is that it reports what it actually saw, a silent omission is exactly the
failure it exists to prevent — and it compounds when fixtures captured under
skew are later analyzed as if complete.

The tempting options were to pin the cluster down to v1.36 so the pair matches,
or to say nothing because skew is supported.

## Decision
Run the skew and **display it**. `GET /api/v1/environment` returns both
`server_version` (read from the API server at startup) and
`client_target_version` (the compiled-in binding version), and the interface
shows them next to each other with a `version skew` marker when they differ.

## Consequences
- The operator can see the gap instead of inheriting an assumption.
- When a field is missing from a finding, there is a documented first thing to
  check.
- Pinning the cluster to the client version would have made the *local*
  environment tidy and taught us nothing, because the eventual target is EKS,
  where the server version is not ours to choose. Skew is the permanent
  condition; the tooling should make it visible rather than pretend it away.
- `client_target_version` is a constant that must be updated when the
  `k8s-openapi` feature changes. A drift between them would be its own quiet
  lie, so it belongs in the dependency-bump checklist.

## Alternatives rejected
- **Pin `kind` to v1.36.4.** Tidy locally, useless against EKS, and it hides the
  problem rather than surfacing it.
- **Say nothing, skew is supported.** True and insufficient. "Supported" means
  the API calls work, not that no field was dropped.
- **Fail on any skew.** Would refuse to run against most real clusters.
