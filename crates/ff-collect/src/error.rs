//! Collector errors.

use thiserror::Error;

/// Why the collector could not start or could not continue.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CollectError {
    /// A Kubernetes client could not be constructed.
    ///
    /// The underlying error is deliberately not embedded: kube's error strings
    /// can carry the API server URL and occasionally request detail, and this
    /// message reaches logs (`THREAT_MODEL.md` R1).
    #[error("could not connect to the Kubernetes API: {reason}")]
    Connect {
        /// A redacted description.
        reason: String,
    },

    /// The cluster identity could not be established.
    #[error("could not determine cluster identity: {reason}")]
    ClusterIdentity {
        /// A redacted description.
        reason: String,
    },

    /// A fixture could not be loaded.
    #[error("could not load fixture {file}: {reason}")]
    Fixture {
        /// The file that failed.
        file: String,
        /// Why.
        reason: String,
    },

    /// Building a snapshot from collected facts failed.
    #[error(transparent)]
    Core(#[from] ff_core::FleetForgeError),
}
