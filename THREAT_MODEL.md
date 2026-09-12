# FleetForge — Threat Model

Status: **initial** (Milestone 0). Revised at every milestone that adds a trust boundary.

## 1. What we are protecting

| Asset | Why it matters |
| --- | --- |
| Kubernetes credentials (kubeconfig, SA tokens, client certs) | Cluster takeover |
| AWS credentials / SSO sessions | Account compromise |
| The ability to mutate the cluster | Drain/reboot = production outage |
| Cluster topology and workload inventory | Reconnaissance for a later attack |
| Audit records | Their value is entirely in being tamper-evident |
| Host signals from `ff-agent` | Node-level detail, and the agent's own privileges |

## 2. Trust boundaries

```
 Browser ──1── ff-api ──2── Kubernetes API server
                  │
                  ├──3── ff-agent (DaemonSet, per node)
                  ├──4── ff-store (SQLite/PostgreSQL)
                  └──5── AWS APIs (EKS, EC2) — M6+
```

1. **Browser ↔ API.** Untrusted input; also the boundary where secrets most easily leak *out*.
2. **API ↔ cluster.** Two distinct identities: read-only (always) and execution (only when an
   approved plan requires it).
3. **API ↔ agent.** The agent runs on every node and is therefore a lateral-movement target.
4. **API ↔ store.** Holds audit records and cluster inventory.
5. **API ↔ AWS.** Node-group replacement is destructive and costs money.

## 3. Adversaries considered

- **Curious or careless operator** — the most likely one. Misreads a `WHAT-IF` as `LIVE`,
  approves a stale plan, drains an AZ. Mitigated by design, not by training.
- **Authenticated low-privilege user** — tries to approve their own plan, replay their way into
  an execution, or widen the allow-list.
- **Compromised workload in the cluster** — reaches the FleetForge service or the agent's socket.
- **Supply chain** — a malicious crate or npm package inside the control plane.
- **Network observer** — between agent and control plane, or browser and API.

Out of scope for now: a hostile cluster administrator (already holds full cluster rights), and
physical access.

## 4. Risks and mitigations

### R1 — Credential disclosure through the API or logs
*Kubeconfig contents, bearer tokens, certificates, or AWS account IDs appear in a response, an
error, or a log line.*

- Credentials never enter `ff-core` types; the API cannot serialize what it cannot see.
- A single redaction layer in `ff-api` wraps every response and every `tracing` event.
- `cluster_id` is a stable opaque identifier, never the API server URL.
- Structured errors carry a code, not the underlying kube error string, when that string may
  embed a URL or token.
- Tests assert that known-secret values never appear in a rendered response or captured log.

### R2 — Unauthorized or accidental cluster mutation
*The highest-severity risk in the product.*

- Read-only by default. `ff-collect` owns the only Kubernetes client; a mutating client is
  constructed only after an approval token validates.
- Separate ServiceAccount for execution, with an allow-list of resource kinds **and field paths**.
- Approval binds to an exact plan hash; a recomputed identical plan needs a new approval.
- Snapshot freshness re-checked before each wave; a stale snapshot aborts.
- Replay and fixture modes are constructed without an adapter handle — unreachable, not disabled.
- Every mutation writes an audit record before and after, with postcondition verification.

### R3 — Misrepresentation of data (`WHAT-IF` shown as `LIVE`)
*A correctness bug with real operational consequences.*

- `Mode` is mandatory on every fact and every response envelope; there is no default.
- Fixture-sourced facts are constructed only by `FixtureSource` and are stamped at construction.
- Property test: no code path can emit fixture-derived data with `mode: Live`.
- The UI renders the mode as a persistent chrome element, not a dismissible badge.
- Stale streams display as stale; absence of events is never rendered as health.

### R4 — Privilege escalation via `ff-agent`
*A DaemonSet on every node is an attractive target.*

- No `privileged: true`. All capabilities dropped, then re-added individually with written
  justification in this document. Read-only host mounts. No host network unless a specific signal
  requires it, documented here when it does.
- The agent **reports**; it accepts no commands and exposes no mutating endpoint.
- mTLS to the control plane; the agent's identity is scoped to its own node's data.
- eBPF stays out of scope until the base product is reliable, precisely because it would rewrite
  this section.

### R5 — Approval workflow bypass
- Blocked plans are unapprovable at the type level (an approval cannot be constructed for a plan
  carrying a `Blocker` finding).
- Approver identity is recorded and must differ from the requester when policy requires it.
- Approval tokens are single-use, time-bounded, and hash-bound.

### R6 — Audit tampering
- Append-only by contract; no update or delete path exists in the `Store` trait.
- Hash-chained records so a deletion is detectable.
- Audit writes happen before the mutation, not after.

### R7 — Supply chain
- `cargo deny` and `cargo audit` in CI; `npm audit` for the frontend.
- Dependabot or Renovate.
- Container image scanning for both ARM64 and x86_64 builds.
- Lockfiles committed; builds reproducible from a clean checkout.

### R8 — AWS blast radius
- Read-only identity inspection before any provisioning.
- Terraform only; no imperative resource creation.
- Every resource tagged `Project=FleetForge`, `Environment=dev`, `ManagedBy=Terraform`,
  `Owner=Param`.
- Explicit human approval of a shown `terraform plan` before any apply.
- Destroy is documented in `DEMO.md` and never executed automatically.

## 5. Secrets handling

- No AWS secret access keys in prompts, source, shell history, logs, or committed `.env` files.
  AWS access comes from SSO or a named CLI profile resolved by the standard credential chain.
- Kubernetes access comes from the kubeconfig context the operator selects, or from in-cluster
  ServiceAccount projection when FleetForge runs inside the cluster.
- `.gitignore` blocks `.env`, `*.kubeconfig`, `*.pem`, `*.tfvars`, `*.tfstate*`.
- Secret scanning in CI; a hit fails the build.

## 6. Known gaps at Milestone 0

Stated plainly because they are real:

- FleetForge has no authentication of its own yet. M2 binds to `127.0.0.1` and is single-user.
  Real authN/authZ is a hard prerequisite for M5 (first mutating endpoint), not a follow-up.
- No multi-tenancy. One operator, one cluster.
- The agent does not exist yet; its privilege posture is a design commitment, not a verified fact.
- Audit hash-chaining is designed but unimplemented.
