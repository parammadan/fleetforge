//! Kubernetes collector.
//!
//! This crate owns **every** call to the Kubernetes API (ADR-0008). Nothing
//! else in the workspace may depend on `kube`, and the boundary tests in
//! `ff-core` fail the build if that changes — including a source-level check
//! that no `kube::` path appears outside this crate.
//!
//! That rule is what makes "read-only mode cannot mutate the cluster" a
//! structural property rather than a policy someone has to remember. In the
//! vertical slice no mutating client is constructed anywhere, because no
//! execution adapter exists (ADR-0012).
//!
//! # What the collector guarantees
//!
//! It never reports absence it cannot vouch for. A watch that is forbidden, a
//! credential that expired, a stream that died, and a resource that genuinely
//! has no objects are four different states, and only the last one is an empty
//! list. See [`status`] and ADR-0019.

pub mod collector;
pub mod config;
pub mod error;
pub mod fixture;
pub mod normalize;
pub mod status;
pub mod store;

pub use collector::{Collector, start};
pub use config::CollectorConfig;
pub use error::CollectError;
pub use fixture::FixtureSource;
pub use status::{DEFAULT_STALENESS_BUDGET, FailureCause, KindTracker};
pub use store::{SnapshotStore, SnapshotUpdate};

/// The Kubernetes kinds the collector watches.
///
/// A snapshot carries a [`KindCoverage`](ff_core::KindCoverage) entry for each,
/// so an analyzer can tell the difference between "none exist" and "we could
/// not look".
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
    fn watched_kinds_are_unique() {
        let mut sorted = WATCHED_KINDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), WATCHED_KINDS.len(), "duplicate kind");
    }
}
