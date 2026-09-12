//! HTTP surface for FleetForge.
//!
//! Axum, with REST for queries and Server-Sent Events for the live stream
//! (ADR-0007). This crate owns configuration, tracing and OpenTelemetry setup,
//! Prometheus metrics, timeouts, cancellation, graceful shutdown, and the
//! redaction layer that keeps kubeconfig contents, bearer tokens, certificates,
//! and AWS account identifiers out of every response and every log line.
//!
//! # Status
//!
//! Milestone 1 defines the response envelope. The routes arrive in Milestone 2.

use ff_core::Mode;
use serde::Serialize;

/// The envelope every API response is wrapped in.
///
/// `mode` is not optional and has no default. An endpoint that cannot state
/// what kind of data it is returning does not compile, which is the point: the
/// honesty guarantee lives in the type, not in a convention (ADR-0003).
#[derive(Debug, Serialize)]
pub struct Envelope<T> {
    /// What kind of data this is.
    pub mode: Mode,
    /// The label the interface displays, so the frontend cannot derive it
    /// incorrectly.
    pub mode_label: &'static str,
    /// Stable cluster identifier. Never an API server URL.
    pub cluster_id: String,
    /// When the underlying data was observed or computed.
    pub observed_at: chrono::DateTime<chrono::Utc>,
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
        data: T,
    ) -> Self {
        Self {
            mode,
            mode_label: mode.label(),
            cluster_id: cluster_id.into(),
            observed_at,
            data,
        }
    }
}
