# ADR-0005 — `Store` trait with SQLite first

**Status:** Accepted · 2026-09-12

## Context
FleetForge persists snapshots, findings, plans, approvals, audit records, and observed events.
A single operator on a laptop and a shared team deployment have very different storage needs.
Requiring PostgreSQL on day one makes local development heavy on an 8 GB machine.

## Decision
Define a `Store` trait in `ff-store`. Implement it with SQLite via `sqlx` first. PostgreSQL
becomes an additional implementation, not a rewrite. Audit records are append-only *in the trait*
— there is no update or delete method to call.

## Consequences
- Zero-dependency local development; the database is a file.
- The trait keeps SQLite-only assumptions out of the rest of the codebase.
- SQLite's single-writer model will eventually constrain concurrent execution tracking. That is
  the signal to add the PostgreSQL implementation, and it is a known trigger rather than a
  surprise.
- Migrations must be written for both engines once the second exists.

## Alternatives rejected
- **PostgreSQL from the start.** Correct eventually, heavy now, and on this machine it competes
  with Kind for memory.
- **No abstraction, SQLite everywhere.** Cheaper today; a migration across the whole codebase later.

> **Amended by [ADR-0018](0018-jsonl-event-log-before-sqlite.md)** — the first `Store`
> implementation is an append-only JSONL event log; SQLite arrives with the execution
> controller, which needs concurrent writers and hash-chained audit records.
