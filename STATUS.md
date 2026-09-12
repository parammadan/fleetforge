# FleetForge — Status

Last updated: 2026-09-12 · Current milestone: **M0 — repository foundation** (complete, awaiting review)

## Completed

- **Environment inspection** (actually executed on this machine, 2026-09-12):

  | Tool | Result |
  | --- | --- |
  | macOS 26.4, arm64, 8 GB RAM, 8 CPU | ✅ |
  | Free disk | ⚠️ 35 GB available — a real constraint for Rust `target/` + container images |
  | Node / npm / pnpm | ✅ v25.8.2 / 11.11.1 / 11.20.0 |
  | Docker CLI | ✅ v29.6.1 (Docker Desktop) — **daemon not running** |
  | kubectl | ✅ v1.36.1 — **no contexts configured**, no current context |
  | AWS CLI | ✅ v2.35.21 — profile `default`, region `us-east-2` |
  | git | ✅ 2.50.1 · Homebrew ✅ 6.0.20 |
  | Rust / cargo / rustup | ❌ **not installed** |
  | Colima / Kind / Helm / Terraform | ❌ **not installed** |

- **Repository created** at `~/fleetforge`, git initialised, no commits yet.
- **Documents written**: `README`, `VISION`, `ARCHITECTURE`, `THREAT_MODEL`, `ROADMAP`, this file,
  `CONTRIBUTING`, `DEMO`, ADRs 0001–0018, `LICENSE` (Apache-2.0), `.gitignore`, CI skeleton.

## Not done — stated explicitly so nothing is assumed

- No Rust code, no TypeScript code, no Cargo workspace files.
- No dependency installed for this project.
- No Kubernetes connection attempted. No cluster read, no cluster mutation.
- No AWS API call made — **not even `sts get-caller-identity`**. Deferred until M6 planning.
- No CI run. The workflow file is a skeleton and has never executed.
- Nothing committed or pushed.

## In progress

- **Roadmap recut to the critical path** (2026-09-12, at your direction). Five milestones, not
  eleven. M0 foundation → M1 workspace → M2 live read-only → M3 preflight + recommendation summary
  → M4 Brupop observation + event log + report → M5 ephemeral EKS run and recording.
- Five subsystems deferred with ADRs stating the evidence that would justify each: wave planner
  (0011), execution controller (0012), host agent (0013), replay engine (0014), chaos framework
  (0015).

## Next (after approval)

1. **M1 — workspace and domain model.** Install Rust, create the cargo workspace (`ff-core`,
   `ff-collect`, `ff-preflight`, `ff-api`), implement `ff-core` types with tests, get
   `fmt` + `clippy -D warnings` + `test` green in CI on both architectures. ~4 days.
2. **M2 — live read-only slice.** Requires separate explicit approval.

## Blockers

| # | Blocker | Needed from Param |
| --- | --- | --- |
| B1 | No Rust toolchain | Approval to install `rustup` + stable toolchain (~1.5 GB) |
| B2 | No Kubernetes cluster | Approval to install Kind + Helm at M2, or the name of an existing context |
| B3 | 35 GB free disk | Confirmation this is acceptable, or a decision to prune first |
| B4 | AWS access at M5 | SSO or a named profile. `~/.aws/credentials` currently holds static long-lived keys — I have not read their values and would rather not use them |

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

```bash
# B1 — Rust toolchain (~1.5 GB). Run yourself, or tell me to run it.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# B2 — local cluster tooling, only when we reach M2
brew install kind helm
```

Approvals needed: **(a)** install Rust, **(b)** architecture sign-off, **(c)** start M1.
M2, M5 AWS provisioning, and every individual `kubectl` change during the demonstration each
require their own separate approval in-session.
