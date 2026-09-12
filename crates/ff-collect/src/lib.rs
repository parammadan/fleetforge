//! Kubernetes collector.
//!
//! This crate owns **every** call to the Kubernetes API (ADR-0008). Nothing
//! else in the workspace may depend on `kube`, and
//! `tests/crate_boundaries.rs` in `ff-core` fails the build if that changes.
//!
//! That single rule is what makes "read-only mode cannot mutate the cluster" a
//! structural property rather than a policy someone has to remember. In the
//! vertical slice no mutating client is constructed anywhere, because no
//! execution adapter exists (ADR-0012).
//!
//! # Status
//!
//! Milestone 1 defines the boundary. The watchers, normalizers, and
//! `FixtureSource` arrive in Milestone 2.

/// The Kubernetes kinds the collector watches.
///
/// Declared here in Milestone 1 so the coverage model has something concrete to
/// check against: a snapshot must carry a
/// [`KindCoverage`](ff_core::KindCoverage) entry for each of these, and an
/// analyzer that needs one may refuse to run when it is missing.
pub const WATCHED_KINDS: &[&str] = &[
    "Node",
    "Pod",
    "Deployment",
    "StatefulSet",
    "DaemonSet",
    "ReplicaSet",
    "PodDisruptionBudget",
    "Event",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watched_kinds_are_unique_and_sorted_for_stable_coverage_reporting() {
        let mut sorted = WATCHED_KINDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), WATCHED_KINDS.len(), "duplicate kind");
    }
}
