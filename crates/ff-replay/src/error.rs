//! Replay errors.
//!
//! A bundle that cannot be fully validated is rejected rather than partially
//! loaded. A leadership interface built on half a bundle would present gaps as
//! facts, and the whole point of this crate is that every number on screen
//! traces to a file on disk.

use thiserror::Error;

/// Why a replay bundle could not be loaded or served.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReplayError {
    /// The bundle directory does not exist or cannot be read.
    #[error("replay bundle not found at {path}")]
    BundleNotFound {
        /// The directory that was looked for.
        path: String,
    },

    /// A file the bundle cannot do without is missing.
    #[error("replay bundle is incomplete: {artifact} is required but absent")]
    MissingArtifact {
        /// The missing file.
        artifact: String,
    },

    /// A file exists but could not be parsed.
    #[error("replay bundle artifact {artifact} is malformed: {reason}")]
    MalformedArtifact {
        /// The file.
        artifact: String,
        /// What was wrong.
        reason: String,
    },

    /// The event log is present but unusable as a timeline.
    #[error("replay timeline is invalid: {reason}")]
    InvalidTimeline {
        /// What was wrong.
        reason: String,
    },

    /// An artifact was requested by a name that is not in the manifest.
    ///
    /// Deliberately indistinguishable from "does not exist": the API must not
    /// let a caller probe the filesystem by interpreting different errors.
    #[error("no such artifact")]
    UnknownArtifact,

    /// A filesystem operation failed.
    #[error("could not read {path}: {reason}")]
    Io {
        /// The path involved.
        path: String,
        /// What went wrong.
        reason: String,
    },
}
