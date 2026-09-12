# Architecture Decision Records

One decision per file, numbered sequentially, never edited after acceptance — a decision that
changes gets a new ADR that supersedes or amends the old one.

Template: context (the forces), decision (what we chose), consequences (what we accept),
alternatives (what we rejected and why). Deferral ADRs additionally state **the evidence that
would justify building the thing**, so "not yet" has a trigger rather than being a permanent maybe.

## Foundational

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-rust-control-plane.md) | Rust for the control plane | Accepted |
| [0002](0002-watch-based-collector.md) | Watch-based collector with immutable snapshots | Accepted |
| [0003](0003-provenance-on-every-fact.md) | Provenance and data mode on every fact | Accepted |
| [0004](0004-evidence-analyzers-not-scheduler.md) | Evidence-based analyzers, not scheduler simulation | Accepted |
| [0005](0005-store-trait-sqlite-first.md) | `Store` trait, SQLite first | Amended by 0018 |
| [0006](0006-approval-bound-execution.md) | Approval bound to an immutable plan hash | Accepted |
| [0007](0007-sse-over-websockets.md) | SSE rather than WebSockets | Accepted |
| [0008](0008-single-kubernetes-client-owner.md) | Only `ff-collect` constructs a Kubernetes client | Accepted |
| [0009](0009-docker-desktop-and-kind.md) | Docker Desktop + Kind for local development | Accepted |
| [0010](0010-apache-2-licence.md) | Apache-2.0 licence | Accepted |

## Scope — deferred subsystems

| ADR | Deferred | Trigger to revisit |
| --- | --- | --- |
| [0011](0011-defer-wave-planner.md) | Wave planner | More nodes than a human will sequence by hand |
| [0012](0012-defer-execution-controller.md) | Execution controller | An executor Brupop does not cover, plus a real second user |
| [0013](0013-defer-host-agent.md) | Host-observability agent | A prediction miss the API server could not explain |
| [0014](0014-defer-replay-engine.md) | Interactive replay | The JSONL log proving insufficient for review |
| [0015](0015-defer-chaos-framework.md) | Chaos framework | Analyzers passing fixtures but missing real failures |

## Slice decisions

| ADR | Decision | Status |
| --- | --- | --- |
| [0016](0016-recommendation-summary-is-preflight-output.md) | Recommendation summary is preflight output, not a controller | Accepted |
| [0017](0017-ephemeral-eks-demo-environment.md) | Ephemeral EKS; the artifacts are the deliverable | Accepted |
| [0018](0018-jsonl-event-log-before-sqlite.md) | Append-only JSONL event log before SQLite | Accepted |
