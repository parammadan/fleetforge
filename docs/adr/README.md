# Architecture Decision Records

One decision per file, numbered sequentially, never edited after acceptance — a decision that
changes gets a new ADR that supersedes the old one.

Template: context (the forces), decision (what we chose), consequences (what we accept),
alternatives (what we rejected and why).

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-rust-control-plane.md) | Rust for the control plane | Accepted |
| [0002](0002-watch-based-collector.md) | Watch-based collector with immutable snapshots | Accepted |
| [0003](0003-provenance-on-every-fact.md) | Provenance and data mode on every fact | Accepted |
| [0004](0004-evidence-analyzers-not-scheduler.md) | Evidence-based analyzers, not scheduler simulation | Accepted |
| [0005](0005-store-trait-sqlite-first.md) | `Store` trait, SQLite first | Accepted |
| [0006](0006-approval-bound-execution.md) | Approval bound to an immutable plan hash | Accepted |
| [0007](0007-sse-over-websockets.md) | SSE rather than WebSockets | Accepted |
| [0008](0008-single-kubernetes-client-owner.md) | Only `ff-collect` constructs a Kubernetes client | Accepted |
| [0009](0009-docker-desktop-and-kind.md) | Docker Desktop + Kind for local development | Accepted |
| [0010](0010-apache-2-licence.md) | Apache-2.0 licence | Accepted |
