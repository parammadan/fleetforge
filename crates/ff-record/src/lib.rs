//! Recording, reports, and prediction scoring.
//!
//! The durable artifact is an append-only JSONL event log (ADR-0018): one JSON
//! object per line, each carrying its sequence number and data mode. The report
//! and the prediction-versus-actual comparison are generated from it in a single
//! sequential pass, offline, with no live connection.
//!
//! The format *is* the deliverable. A reviewer who does not trust FleetForge
//! should be able to `jq` the log and check every claim in the report, which is
//! why there is no export step that could diverge from its source.
//!
//! # Replay, and what is deferred
//!
//! Recording is here; interactive playback is not (ADR-0014). The log schema is
//! designed so a timeline scrubber can be added later without re-recording
//! anything, and every line already carries the mode that would mark it
//! `REPLAY`.

pub mod error;
pub mod event;
pub mod log;
pub mod report;
pub mod score;

pub use error::RecordError;
pub use event::{
    LogEntry, Prediction, RecordedEvent, RunId, WorkloadKey, kubernetes_event, prediction,
    snapshot_event,
};
pub use log::{EventLog, read};
pub use report::{Report, TimelineItem, build, to_markdown};
pub use score::{Accuracy, Comparison, score};
