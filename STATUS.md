# FleetForge — Status

Last updated: 2026-09-12 · Current milestone: **M3 complete** → M4 awaiting approval

## Completed

### M0 — repository foundation ✅
Environment inspected, repository created, documents and ADRs 0001–0018 written, roadmap recut to
the five-milestone critical path.

### M1 — workspace and domain model ✅
Rust 1.98.1 pinned; `ff-core` domain model with validated provenance, canonical content hashing,
and crate-boundary tests. 33 tests.

### M2 — live read-only slice ✅ 2026-09-12

**Environment** (created, running, verified):

| | |
| --- | --- |
| Runtime | Docker Desktop 29.6.1 (already installed; Colima not added). 8 CPU / 3918 MiB — met the bar, so **no global settings were changed** |
| Cluster | `kind` v0.33.0, `fleetforge-dev`, 1 control-plane + 2 workers, all `Ready` |
| Node image | `kindest/node:v1.37.0@sha256:a1ed56cf…`, verified against both the installed binary and the published v0.33.0 release notes; arm64 confirmed after pull in `infra/local/IMAGES.md` |
| Demo images | `nginx:1.27-alpine@sha256:65645c7b…`, `pause:3.10@sha256:ee6521f2…`; resolved digests match the pins |
| Workload | `web` (3 replicas), `api` (1 — a singleton), `node-agent` DaemonSet (2), `web-pdb` with `ALLOWED DISRUPTIONS: 1` |

**Tests — all actually executed:**

| Suite | Result |
| --- | --- |
| `cargo test --workspace` | ✅ **87 passed, 0 failed** |
| `cargo test -p ff-collect --test live_cluster -- --ignored` | ✅ **3 passed** against the real cluster |
| `npm run typecheck` · `npm test` · `npm run build` | ✅ clean · **8 passed** · builds (239 kB JS) |
| `cargo fmt --check` · `cargo clippy -- -D warnings` · `cargo deny check` | ✅ all clean |
| `make check` | ✅ **PASS** |

**Requirements, each proven by something that ran:**

| # | Requirement | Evidence |
| --- | --- | --- |
| 1–3 | Reproducible pinned kind config, cluster `fleetforge-dev` | `infra/local/kind.yaml`, `IMAGES.md` |
| 4–6 | Read-only SA, only get/list/watch, no cluster-admin | `infra/local/rbac/`, and `mutation-denial-test.sh`: **60 mutating verb/resource pairs denied, 6 secret/configmap reads denied, 27 required reads allowed** |
| 7 | No credentials committed | `.gitignore` blocks `infra/local/.kubeconfig-*`, verified with `git check-ignore` |
| 8 | Real demo namespace with PDB | `kubectl get pdb -n demo` → `ALLOWED DISRUPTIONS 1` |
| 9–11 | `ff-collect` connected; nodes/pods/deployments/PDBs with provenance, coverage, resourceVersions, timestamps | `fleetforge --once` output; `/api/v1/environment` |
| 12 | Marked `LIVE` because it is watched | `mode_label: "LIVE"`; fixture run of the *same* state yields a different snapshot id because `mode` is hashed |
| 13 | External `kubectl scale` appears automatically | `watch-demo.sh`: `webPods` 3 → **5** three seconds after the scale, no refresh, no polling timer |
| 14 | Pod deletion and recreation appear | same run: 5 → 6 → 5 across delete/recreate |
| 15 | Identity cannot mutate | `mutation-denial-test.sh` asks the live API server, exits non-zero on any violation |
| 16 | Denial is `Forbidden`, not an empty list | Real 403 as `fleetforge-restricted`: `PodDisruptionBudget 0 forbidden`, `authoritative false` |
| 17 | Reconnection, stale, disconnection, forbidden, loading, empty | `resilience-demo.sh` (paused API server → `stale`, counts retained, recovers); `expired-credential-test.sh` (invalid at startup → refuses to start; invalidated mid-run → `degraded`) |
| 18 | Crate boundaries preserved | 3 boundary tests, now including a **source-level** check that no `kube::` path appears outside `ff-collect`, with a guard against passing vacuously |
| 19 | `make check` and frontend tests | ✅ above |
| 20 | Docs updated | ADRs 0019–0023, `DEMO.md`, `ARCHITECTURE.md`, this file |

**Two defects found and fixed during M2, both worth naming:**

1. **A dead API server looked healthy.** Freshness was measured as "time since the last watch
   event", but a watch is silent both when nothing is happening and when the connection has died.
   With the control plane paused, FleetForge reported `authoritative=true` and every kind
   `in_sync` for the full outage. Followed through, a frozen PDB list reads as "no blockers",
   which reads as *safe to drain*. Fixed with an independent liveness probe (ADR-0023).
2. **Unauthorized was reported as unreachable.** Safe, but it would send an operator to debug the
   network when the fix is to reissue a token. Now distinguished: `degraded` with a credential
   message versus `stale`.

### M3 — evidence-based preflight ✅ 2026-09-12

Ten analyzers over the snapshot `ff-collect` produces, plus the recommendation summary and the
preflight workspace UI.

| Analyzer | Findings | Confidence |
| --- | --- | --- |
| PodDisruptionBudget | `FF-PDB-001` no disruption permitted · `FF-PDB-002` one node holds more covered pods than allowed · `FF-PDB-003` permitted | Certain |
| Singleton workload | `FF-SINGLETON-001` | Certain |
| Unmanaged pod | `FF-UNMANAGED-001` | Certain |
| Aggregate CPU / memory | `FF-CPU-001/002` · `FF-MEM-001/002` | **Heuristic** |
| Node selector | `FF-SELECTOR-001` | Likely |
| Toleration | `FF-TOLERATION-001` | Likely |
| Required node affinity | `FF-AFFINITY-001` | Likely |
| Node-bound storage | `FF-STORAGE-001` · `FF-STORAGE-002` (emptyDir) | Certain |
| AZ / multi-node | `FF-AZ-001` whole zone · `FF-AZ-002` every replica | Heuristic |
| Coverage gate | `FF-COVERAGE-001` · `FF-REQUEST-001` | Certain |

**Tests — all executed:**

| Suite | Result |
| --- | --- |
| `cargo test --workspace` | ✅ **118 passed, 0 failed** (31 new analyzer tests) |
| `cargo test -p ff-collect --test live_cluster -- --ignored` | ✅ 3 passed against the live cluster |
| `npm test` · `npm run typecheck` · `npm run build` | ✅ **15 passed** · clean · 248 kB |
| `make check` | ✅ PASS |

**The M3 demonstration, run end to end** (`./scripts/preflight-demo.sh`): SAFE → tighten the PDB
with `kubectl` → **BLOCKED** with the exact object, the arithmetic, and the limitations → relax it
→ SAFE. Each run prints the snapshot id it analyzed. See `DEMO.md` for the verbatim output.

**Three design decisions worth naming:**

1. **Concurrency is defined, not guessed.** `recommended_max_concurrency` is *the largest wave
   size that produces no blocker*, found by re-running the analysis at each size. An operator can
   verify the number by re-running preflight at it. `concurrency_constraint` names the finding
   that blocked at one node more, so "why 1?" has an answer rather than a heuristic to trust.
2. **The concurrency control is real.** Preflight analyzes a *wave* — the k highest-impact nodes
   from the selection — so the same selection is genuinely safe at 1 and blocked at 2. There is a
   test asserting exactly that.
3. **An analyzer that cannot see its inputs does not run, and its silence is a blocker.** If the
   PDB list was forbidden, `FF-COVERAGE-001` blocks rather than the PDB analyzer finding nothing
   and the result reading as safe. That is the same failure as M2's, one layer up.

**A real finding the analyzers caught unprompted:** `fleetforge-dev-worker` holds 2 of the 3 `web`
pods, so draining it is blocked by `FF-PDB-002` even under the permissive budget — a drain evicts
a node's pods together while the budget allows one disruption at a time. Same cluster, same
budget, different node, different answer. I had not set that up; the analyzer found it.

## Not done — stated explicitly

- **CI has never run.** There is no git remote. The workflow is written and enabled, so the
  cross-architecture snapshot-hash claim is verified on `aarch64` only.
- **The UI has still not been viewed in a browser.** Typecheck, 15 component tests, and a
  production build all pass, and every API response was verified by hand — but no human or headless
  browser has rendered the page. Playwright coverage of the UI states remains unwritten. This is
  the largest unverified claim in the project.
- **No scheduler predicate evaluation.** Capacity findings are `Heuristic` and say so in their own
  `limitations`. `FF-STORAGE-001` cannot read PersistentVolume node affinity, because FleetForge
  has no PV read permission at this milestone — a network PV that is in fact zone-bound is not
  flagged.
- **No pod anti-affinity or topology-spread analyzer.** Both are modelled in `ff-core` and
  collected by `ff-collect`, but no analyzer reads them yet. `FF-AZ-001/002` covers the zone case
  only.
- No AWS call. No Brupop. No cluster mutation capability.
- `target/` is now **4.9 GB**, with 24 GB free. Worth a `cargo clean` before M5, which adds Terraform and container builds.

## Decisions taken

| ID | Decision | Rationale |
| --- | --- | --- |
| D1 | Repository at `~/fleetforge` | Matches the layout of your other projects |
| D2 | SSE over WebSockets | One-directional data; free reconnection; plain `GET` for auth — ADR-0007 |
| D3 | `ff-collect` owns the only Kubernetes client | Makes "read-only cannot mutate" testable — ADR-0008 |
| D4 | Evidence-based analyzers, not scheduler simulation, in V1 | Honest and achievable — ADR-0004 |
| D5 | Apache-2.0 | Standard for this ecosystem; compatible with Brupop's licence — ADR-0010 |
| D6 | Docker Desktop rather than Colima | Already installed; no reason to add a second VM runtime — ADR-0009 |
| D7 | Recommendation summary is preflight output, not a controller | A calculation over one snapshot, not a stateful subsystem — ADR-0016 |
| D8 | EKS is ephemeral; the artifacts are the deliverable | ~$8/day only while capturing the demo; forces reproducible stand-up — ADR-0017 |
| D9 | Append-only JSONL event log before SQLite | "Persisted event JSON" is a deliverable a reviewer can `jq`; defers sqlx compile cost — ADR-0018 |
| D10 | Brupop executes; FleetForge never mutates | No mutating client is constructed, so the safety property is structural — ADR-0012 |

## Commands or approvals required from you

Nothing has been installed or changed outside `~/fleetforge`. To unblock M1:

To see it yourself — the cluster is already running:

```bash
cd ~/fleetforge && export PATH="$HOME/.cargo/bin:$PATH"

make check                                    # fmt, clippy -D warnings, 87 tests
./scripts/mutation-denial-test.sh             # proves the identity cannot mutate
./scripts/watch-demo.sh                       # scale + pod delete appearing live
./scripts/resilience-demo.sh                  # API server outage handling
./scripts/expired-credential-test.sh          # credential expiry handling

./target/debug/fleetforge --kubeconfig infra/local/.kubeconfig-fleetforge-reader
cd web && npm run dev                         # then open http://127.0.0.1:5173
```

Approvals needed: **(a)** start M3.
M5 AWS provisioning and every individual `kubectl` change during the demonstration each require
their own separate approval in-session.
