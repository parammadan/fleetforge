# ADR-0024 — An uninstalled resource type is authoritatively empty

**Status:** Accepted · 2026-09-12

## Context
FleetForge watches `BottlerocketShadow`, a custom resource owned by Brupop. Most clusters do not
have Brupop installed, so most clusters return `404` for that list.

Everything about the design so far pushes toward treating an empty result as untrustworthy.
ADR-0019 established that `403` and `401` must never render as "there are none", because an empty
PodDisruptionBudget list reads as "no blockers", which reads as safe to drain. The obvious move is
to apply the same rule here.

That would be wrong, and the reason is worth writing down.

## Decision
`CollectionStatus::NotInstalled` is **authoritative**.

If a CustomResourceDefinition is not installed, the resource type does not exist, and "zero
instances" is not a guess — it is the only possible answer, and it is correct. That is a
categorically different statement from `Forbidden`, where zero means "we were not allowed to
look".

`NotInstalled` is therefore its own variant rather than a flavour of `Degraded`, and a `404` on a
list is classified separately from a `403`.

## Consequences
- A cluster without Brupop reports `authoritative: true`, which is the truth. Every other kind was
  collected, and the missing one genuinely has nothing in it.
- The interface says "not installed" rather than "degraded", which matters because the two send an
  operator to different places: install the operator, versus fix a broken watch.
- The failure this avoids is subtle but real. Treating an absent optional CRD as
  non-authoritative would mark every Brupop-less cluster permanently incomplete, and an
  incompleteness warning that is always on is one nobody reads — at which point the warning stops
  working for the cases that *do* matter. Crying wolf has a cost, and it is paid by the genuine
  alarms.
- The distinction rests on `404` meaning "no such resource type". If a future Kubernetes version
  returned `404` for something else, this would mis-classify. Accepted; the alternative reading is
  worse.

## Alternatives rejected
- **Treat `404` like `403`.** Consistent, and wrong: it reports uncertainty where there is none,
  and it devalues the incompleteness signal everywhere else.
- **Do not watch the resource unless the CRD is present.** Requires discovery on startup and a
  re-check when a CRD is installed later. The watch already answers the question continuously.
- **Fold it into `Degraded` with a message.** Loses the authoritative/not distinction, which is
  the only part that affects whether an analyzer may draw a conclusion.
