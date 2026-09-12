# ADR-0003 — Provenance and data mode on every fact

**Status:** Accepted · 2026-09-12

## Context
FleetForge mixes four kinds of data: live cluster reads, calculated what-if outcomes, recorded
history, and test fixtures. Confusing them is the most damaging failure the product can have —
an operator who believes a `WHAT-IF` already happened, or a demonstration where fixture data is
mistaken for a real cluster, does real harm and destroys trust permanently.

The failure mode is not malice. It is a developer adding an endpoint and forgetting the label.

## Decision
`Provenance` is a required field on every fact type and `mode` is required on every API response
envelope. Neither has a default value or an "unknown" variant. Fixture-sourced facts are stamped
at construction by `FixtureSource`, which is the only way to build them.

## Consequences
- Adding a fact type without provenance does not compile.
- The UI can always render a truthful label, because the data always carries one.
- A property test asserts that fixture-derived data can never be emitted as `Live`.
- Slight verbosity in every struct. Worth it.

## Alternatives rejected
- **A mode flag on the connection.** One global flag, easy to get wrong, and wrong in exactly the
  case that matters: mixed live and computed data on one screen.
- **Label in the UI only.** Puts the honesty guarantee in the layer least able to enforce it.
