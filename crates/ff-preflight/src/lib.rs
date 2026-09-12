//! Evidence-based preflight analyzers.
//!
//! Pure functions over a [`ClusterSnapshot`](ff_core::ClusterSnapshot): no I/O,
//! no clock beyond what the caller supplies, no Kubernetes client. That is what
//! makes the analysis deterministic, exhaustively testable against fixtures,
//! and developable without a cluster on an 8 GB machine.
//!
//! Version one does not simulate the Kubernetes scheduler and does not claim
//! to. Each analyzer declares what it proves and what it does not, and every
//! finding it emits carries those limitations (ADR-0004).
//!
//! # Status
//!
//! Milestone 1 defines the trait. The analyzers arrive in Milestone 3.

use ff_core::{ClusterSnapshot, Finding, MaintenanceRequest, Result};

/// One evidence-based check.
///
/// Analyzers are independent and registered in a table, so adding a check never
/// requires editing an existing one.
pub trait Analyzer {
    /// Stable identifier prefix for the findings this analyzer emits, for
    /// example `FF-PDB`.
    fn id_prefix(&self) -> &'static str;

    /// A one-line description of what this analyzer checks.
    fn describes(&self) -> &'static str;

    /// The Kubernetes kinds this analyzer's conclusions depend on.
    ///
    /// The engine checks coverage for each before running the analyzer. An
    /// analyzer that needs PodDisruptionBudgets must not report "no blockers"
    /// when the collector was forbidden from listing them.
    fn required_kinds(&self) -> &'static [&'static str];

    /// Run the check.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot does not contain the facts this
    /// analyzer needs to reach a sound conclusion.
    fn analyze(
        &self,
        snapshot: &ClusterSnapshot,
        request: &MaintenanceRequest,
    ) -> Result<Vec<Finding>>;
}
