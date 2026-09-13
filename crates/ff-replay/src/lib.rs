//! Deterministic replay of a captured FleetForge incident.
//!
//! This crate turns an evidence bundle on disk into a typed, ordered timeline
//! that the API can serve and the interface can scrub through. It is the
//! authoritative source for replay: the browser renders what this produces and
//! invents nothing of its own.
//!
//! # Why a separate crate
//!
//! Replay is kept apart from live collection ([`ff_collect`]) and from fixture
//! loading on purpose. They are three different claims about reality — this is
//! happening, this happened, this never happened — and sharing a code path
//! between them is how one gets mistaken for another. `ff-replay` does not
//! depend on `ff-collect` and cannot construct a Kubernetes client.
//!
//! # What it guarantees
//!
//! - **Nothing is invented.** Every value traces to a line in the captured log
//!   or a file in the bundle. There is no interpolation between events and no
//!   smoothing of gaps: seventeen quiet minutes replay as seventeen quiet
//!   minutes.
//! - **Determinism.** [`state_at`](state::ReplayTimeline::state_at) is a pure
//!   fold. Same bundle, same position, same bytes, every time.
//! - **It can never claim to be live.** Every response carries
//!   [`ff_core::Mode::Replay`], and a test asserts it.
//! - **Claims carry their basis.** Observed, derived, human RCA, unverified, or
//!   explicitly unknown — see [`schema::ClaimBasis`]. A leadership interface
//!   that blurs those is telling a story, not showing evidence.

pub mod artifacts;
pub mod bundle;
pub mod chain;
pub mod chapters;
pub mod error;
pub mod schema;
pub mod state;

pub use artifacts::{ArtifactStore, redact};
pub use bundle::ReplayBundle;
pub use error::ReplayError;
pub use schema::{
    ArtifactKind, ArtifactRef, CaptureContext, Claim, ClaimBasis, DataCaveat, REPLAY_SCHEMA_VERSION,
};
pub use state::{
    NodeReplayState, PodReplayState, PreflightReplayState, ReplayEvent, ReplayState, ReplayTimeline,
};
