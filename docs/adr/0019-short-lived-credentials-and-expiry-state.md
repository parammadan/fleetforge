# ADR-0019 — Short-lived credentials, and expiry as an explicit state

**Status:** Accepted · 2026-09-12

## Context
FleetForge needs a Kubernetes identity. The traditional approach — a ServiceAccount Secret
holding a token that never expires — puts a permanent cluster credential on disk and in any
backup that touches it.

Kubernetes offers the TokenRequest API instead: bound, audience-scoped, short-lived tokens. The
cost is that the credential expires while the process is running, which raises a question most
tools answer badly. A client whose token has expired starts receiving `401 Unauthorized` on its
watches. If that is handled as "the watch ended", the natural rendering is an empty list.

An empty list is indistinguishable from a healthy cluster with nothing in it. For FleetForge that
is not a cosmetic bug: zero PodDisruptionBudgets means no blockers, which means **safe**. A
tool that reports "safe to drain" because its credential expired is worse than a tool that
reports nothing at all.

## Decision
Credentials are short-lived, issued by `kubectl create token` with an explicit duration, written
to a gitignored path. No long-lived Secret tokens.

Expiry is a first-class state. A `401` from any watch transitions that kind to
`CollectionStatus::Degraded` with an `Unauthorized` cause, which is **not authoritative**. The
consequences follow from the type:

- `require_authoritative()` fails, so an analyzer refuses to conclude anything about that kind.
- The interface shows the credential state, not an empty result.
- No preflight result can be reported `Safe` on the strength of data FleetForge could not read.

The same reasoning applies to `403 Forbidden` — a different cause, the same non-authoritative
outcome — and to a stale stream.

## Consequences
- No permanent cluster credential exists on disk.
- Every collection path must carry a cause for its failure, because "no data" and "no permission"
  and "no credential" must render differently.
- Operators will see the state change when a token lapses. That is the intended behaviour, and
  the remedy is to re-issue.
- Token renewal is manual in Milestone 2. In-cluster deployment uses projected ServiceAccount
  tokens, which the kubelet rotates; that lands with the Helm chart.

## The `system:basic-user` caveat

FleetForge's ClusterRole grants `get`, `list`, and `watch`, and nothing else. But the identity can
also **create** `SelfSubjectAccessReviews`, because Kubernetes binds `system:basic-user` to every
authenticated principal by default. That is a `create` verb on a real API.

This is documented rather than worked around, deliberately. It is a read-only introspection API —
it answers "may I?" and changes nothing — and removing the default binding would break normal
cluster access for a security gain that does not exist. The boundary that matters is that
FleetForge holds **no mutation permission over any observed resource**, and
`scripts/mutation-denial-test.sh` proves that against the live API server rather than asserting it
from a YAML file.

Permission probes populate the interface's permissions panel. They are never the authoritative
source for a collection status: a probe says what the API server thinks it would do, whereas a
`403` on a watch is what it actually did. The `403` wins.

## Alternatives rejected
- **Long-lived Secret token.** Simple, and it is a permanent credential sitting in etcd and in
  every backup.
- **Treating 401 as a transient error and retrying silently.** Hides a real state change behind a
  retry loop, and during the retry window the interface shows stale data as current.
- **kubeconfig with client certificates.** Long-lived again, and harder to scope.
