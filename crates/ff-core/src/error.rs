//! Structured errors.
//!
//! Libraries return these; `ff-api` maps them to HTTP status codes and a
//! `{ code, message, retriable, details }` envelope. Error text never embeds an
//! API server URL, a bearer token, or an AWS account identifier — see
//! `THREAT_MODEL.md` R1.

use thiserror::Error;

/// Convenience alias for fallible FleetForge operations.
pub type Result<T> = std::result::Result<T, FleetForgeError>;

/// Every way a FleetForge domain operation can fail.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FleetForgeError {
    /// A [`Provenance`](crate::Provenance) claimed a mode its source cannot
    /// support — fixture data presented as live, for example.
    ///
    /// This is the honesty invariant failing, so it is an error rather than a
    /// silent correction.
    #[error("inconsistent provenance: {reason}")]
    InconsistentProvenance {
        /// What specifically did not line up.
        reason: String,
    },

    /// A snapshot's stored identifier did not match a hash of its contents.
    #[error("snapshot integrity check failed: declared {declared}, computed {computed}")]
    SnapshotIntegrity {
        /// The identifier the snapshot carried.
        declared: String,
        /// The identifier its contents actually produce.
        computed: String,
    },

    /// A fact was required for an analysis but the collector never saw it,
    /// typically because RBAC forbade the read.
    ///
    /// Analyzing a snapshot that could not list PodDisruptionBudgets as though
    /// there were none is precisely the mistake this prevents.
    #[error("incomplete snapshot: {kind} was not collected ({reason})")]
    IncompleteSnapshot {
        /// The Kubernetes kind that is missing.
        kind: String,
        /// Why it is missing.
        reason: String,
    },

    /// A value that reached the domain model was not representable.
    #[error("invalid {field}: {reason}")]
    Invalid {
        /// Which field.
        field: String,
        /// Why it was rejected.
        reason: String,
    },

    /// Canonical serialization failed while hashing a snapshot.
    #[error("canonicalization failed: {0}")]
    Canonicalization(#[from] serde_json::Error),
}
