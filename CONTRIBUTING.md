# Contributing to FleetForge

## Toolchain

| Tool | Version | Purpose |
| --- | --- | --- |
| Rust | stable, pinned in `rust-toolchain.toml` | control plane, collector, planner, agent |
| Node | ≥ 22 | frontend |
| Docker | any recent | container builds, Kind |
| Kind | ≥ 0.23 | local cluster (only when needed) |
| Helm | ≥ 3.14 | chart development |
| Terraform | ≥ 1.7 | AWS environment (M6+) |

Most work needs none of the cluster tooling. `ff-preflight` and `ff-planner` are pure functions
over recorded snapshots in `fixtures/` — develop against those first.

## Checks

Every change must pass, from a clean checkout:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
cd web && npm ci && npm run typecheck && npm test

# End-to-end, in a real browser. Needs the binary built first.
cargo build --bin fleetforge
cd web && npx playwright install chromium && npm run test:e2e
```

The end-to-end tests run against **fixture mode**, never a live cluster: the assertions are about
what the interface renders, and those must not change because a pod restarted mid-test.

`make check` runs all of it (added in M1, once the workspace exists).

## Rules that are not negotiable

1. **Provenance is mandatory.** Any new fact type carries `Provenance`. Any new response carries
   `mode`. There is no default and no "unknown" escape hatch.
2. **Only `ff-collect` constructs a Kubernetes client.** If you need cluster data elsewhere, take
   a `ClusterSnapshot`.
3. **Analyzers are pure.** No I/O in `ff-preflight` or `ff-planner`. If a check needs data it
   does not have, add it to the snapshot rather than fetching it inline.
4. **Every finding states its limitations.** An analyzer that cannot prove its conclusion says so
   in `limitations`, in every finding it emits.
5. **No panics in request paths.** No `unwrap`, `expect`, or indexing that can fail outside tests
   and `main`. Clippy enforces this.
6. **Structured errors.** `thiserror` in libraries; a single error type mapped to HTTP in `ff-api`.
7. **No secrets, ever.** Not in source, not in fixtures, not in tests, not in logs. Fixtures
   captured from a real cluster must be scrubbed of tokens, certificates, and account identifiers
   before being committed.
8. **New execution capability requires new invariant tests.** The ten invariants in
   `ARCHITECTURE.md` §7 gate the executor.

## Fixtures

Fixtures are recorded cluster snapshots in `fixtures/`, loaded by `FixtureSource` and stamped
`FIXTURE` at construction. To add one: capture from a Kind cluster, scrub it, name it for the
condition it represents (`pdb-blocks-drain.json`, `capacity-insufficient.json`), and add a test
that asserts the analyzer behaviour it exists to demonstrate.

Every analyzer needs at least two fixtures: one it flags, one it clears.

## Commits and ADRs

Small, reviewable commits, one logical change each. Any decision that constrains future work —
a dependency, a boundary, a protocol, a privilege — gets an ADR in `docs/adr/`, numbered
sequentially, using the existing template. Superseding an ADR means writing a new one that says
so, not editing the old one.

## Milestones

Work lands in milestone order (`ROADMAP.md`). A milestone is finished when its acceptance
criteria are demonstrable, its tests pass in CI, `STATUS.md` is updated, and its known
limitations are written down.
