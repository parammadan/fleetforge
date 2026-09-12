//! HTTP surface for FleetForge.
//!
//! Axum, with REST for queries and Server-Sent Events for the live stream
//! (ADR-0007). Every response is wrapped in an [`Envelope`] carrying the data
//! mode, so there is no unlabelled path out of this process.
//!
//! This crate constructs no Kubernetes client. It holds a [`ff_collect`]
//! collector and reads snapshots from it — see ADR-0008 and the source-level
//! boundary test in `ff-core`.

pub mod routes;
pub mod state;

use ff_core::Mode;
use serde::Serialize;

/// The envelope every API response is wrapped in.
///
/// `mode` is not optional and has no default. An endpoint that cannot state
/// what kind of data it is returning does not compile (ADR-0003).
#[derive(Debug, Serialize)]
pub struct Envelope<T> {
    /// What kind of data this is.
    pub mode: Mode,
    /// The label the interface displays, so the frontend cannot derive it
    /// incorrectly or forget to.
    pub mode_label: &'static str,
    /// Stable cluster identifier. Never an API server URL.
    pub cluster_id: String,
    /// When the underlying data was observed or computed.
    pub observed_at: chrono::DateTime<chrono::Utc>,
    /// Whether every kind backing this response was collected authoritatively.
    ///
    /// When false, absence in `data` means "we could not see", not "there are
    /// none" — and the interface must say so rather than rendering an empty
    /// list (ADR-0019).
    pub authoritative: bool,
    /// The payload.
    pub data: T,
}

impl<T> Envelope<T> {
    /// Wrap a payload.
    #[must_use]
    pub fn new(
        mode: Mode,
        cluster_id: impl Into<String>,
        observed_at: chrono::DateTime<chrono::Utc>,
        authoritative: bool,
        data: T,
    ) -> Self {
        Self {
            mode,
            mode_label: mode.label(),
            cluster_id: cluster_id.into(),
            observed_at,
            authoritative,
            data,
        }
    }
}

/// A structured error response.
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// Machine-readable code.
    pub code: &'static str,
    /// Human-readable message. Never contains a URL, token, or certificate.
    pub message: String,
    /// Whether retrying might succeed.
    pub retriable: bool,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn an_envelope_always_carries_its_label() {
        let e = Envelope::new(Mode::Fixture, "c1", chrono::Utc::now(), true, 42);
        assert_eq!(e.mode_label, "FIXTURE");
        let json = serde_json::to_string(&e).expect("serializes");
        assert!(json.contains("FIXTURE"));
    }

    #[test]
    fn a_what_if_envelope_is_never_labelled_live() {
        let e = Envelope::new(Mode::WhatIf, "c1", chrono::Utc::now(), true, ());
        assert_eq!(e.mode_label, "WHAT-IF");
        assert!(!e.mode.describes_current_reality());
    }
}
