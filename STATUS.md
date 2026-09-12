# FleetForge — Status

Last updated: 2026-09-12 · Current milestone: **M1 complete** → M2 awaiting approval

## Completed

### M0 — repository foundation ✅
Environment inspected (2026-09-12, read-only commands only). Repository created at
`~/fleetforge`. Documents written: `README`, `VISION`, `ARCHITECTURE`, `THREAT_MODEL`,
`ROADMAP`, this file, `CONTRIBUTING`, `DEMO`, ADRs 0001–0018, `LICENSE` (Apache-2.0),
`.gitignore`, CI. Roadmap recut to the five-milestone critical path.

### M1 — workspace and domain model ✅
**Actually executed and verified on this machine, 2026-09-12:**

| Check | Result |
| --- | --- |
| Rust toolchain installed | ✅ rustc 1.98.1, pinned in `rust-toolchain.toml` |
| `cargo build --workspace` | ✅ 4 crates compile |
| `cargo fmt --all -- --check` | ✅ clean |
| `cargo clippy --all-targets --all-features -- -D warnings` | ✅ clean |
| `cargo test --workspace` | ✅ **33 passed, 0 failed** |
| `cargo deny check` | ✅ advisories, bans, licenses, sources all ok |
| `target/` disk cost | 443 MB (34 GB free remaining) |

Crates: `ff-core` (domain model, complete), `ff-collect`, `ff-preflight`, `ff-api`
(boundaries and traits declared, implementations land in M2/M3).

What `ff-core` actually enforces, with tests:

- **Provenance cannot lie.** Fields are private; construction and *deserialization* both
  validate that a source permits the mode it claims. A hand-edited event log claiming fixture
  data is `LIVE` fails to deserialize.
- **Snapshot identity is canonical.** Content hash over sorted facts and sorted JSON keys.
  Observation timestamps are stripped, so the same cluster state observed twice yields the same
  identifier; `mode` is *not* stripped, so fixture data can never collide with live data.
- **Collection status is part of identity.** A snapshot that was forbidden from listing PDBs
  hashes differently from one that listed them and found none — the two must never be confused.
  `require_authoritative()` lets an analyzer refuse to guess.
- **Tampering is detected.** Editing a fact without recomputing the hash fails deserialization.
- **Crate boundaries are tested, not documented.** `crate_boundaries.rs` fails the build if any
  crate but `ff-collect` declares `kube`, or if `ff-core` grows an I/O dependency.
- **A caller cannot assert a status.** `PreflightResult::new` derives Safe/Blocked from the
  findings and clamps concurrency to 0 when blocked, so a report cannot contradict itself.

## Not done — stated explicitly so nothing is assumed

- No Kubernetes connection attempted. No cluster read, no cluster mutation. `ff-collect`
  contains a constant and a test, not a client.
- No analyzers implemented. `ff-preflight` is a trait.
- No frontend. `web/` is empty.
- No AWS API call made — **not even `sts get-caller-identity`**. Deferred to M5.
- **CI has never run.** The workflow is enabled but there is no remote and no push. The
  cross-architecture hash-stability claim is therefore verified on `aarch64` only.
- Nothing pushed. 6 local commits.

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

Rust is installed. To verify M1 yourself:

```bash
cd ~/fleetforge && export PATH="$HOME/.cargo/bin:$PATH" && make check
```

To unblock M2, when you are ready:

```bash
brew install kind helm          # local cluster tooling
```

Approvals needed: **(a)** start M2, **(b)** install Kind/Helm or name an existing context.
M5 AWS provisioning and every individual `kubectl` change during the demonstration each require
their own separate approval in-session.
