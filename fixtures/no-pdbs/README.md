# A fixture set with PodDisruptionBudgets deliberately missing

Identical to `../captured`, minus `pdbs.json`.

`FixtureSource` reports a kind with no fixture file as **not collected**, which is
non-authoritative — the same class of state as an RBAC denial against a live
cluster. This set exists so the interface's "we could not see this" path can be
tested without needing a restricted ServiceAccount and a live API server.

The distinction it guards: an empty PodDisruptionBudget list reads as "no
blockers", which reads as safe to drain.
