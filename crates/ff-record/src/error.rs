//! Recording errors.

use thiserror::Error;

/// Why a recording operation failed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RecordError {
    /// A filesystem operation failed.
    #[error("could not access {path}: {reason}")]
    Io {
        /// The path involved.
        path: String,
        /// What went wrong.
        reason: String,
    },

    /// Serializing or parsing a log entry failed.
    #[error("log entry could not be encoded or decoded: {0}")]
    Encoding(#[from] serde_json::Error),

    /// The writer mutex was poisoned by a panic in another thread.
    #[error("the event log writer is unusable after a panic")]
    Poisoned,

    /// A report was requested for a run with no entries.
    #[error("no recorded events for run {run_id}")]
    EmptyRun {
        /// The run that has no entries.
        run_id: String,
    },
}
