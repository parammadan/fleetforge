//! Findings: the unit of explanation.
//!
//! A finding has to stand on its own in an exported report, read by someone who
//! does not trust FleetForge and has `kubectl` open. That is why `evidence`,
//! `calculation`, and `limitations` are structured fields rather than prose
//! baked into a message string.
//!
//! [`Finding::limitations`] is required, not optional. An analyzer that cannot
//! prove its conclusion says so in every finding it emits — see ADR-0004.

use serde::{Deserialize, Serialize};

use crate::provenance::{Provenance, ResourceRef};
use crate::snapshot::SnapshotId;

/// A stable finding identifier, for example `FF-PDB-001`.
///
/// Stable across releases so it can be linked to, suppressed, and referenced in
/// a report written months earlier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FindingId(String);

impl FindingId {
    /// Wrap an identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The identifier as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FindingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// How serious a finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational.
    Info,
    /// Worth knowing, no action implied.
    Low,
    /// Should be understood before proceeding.
    Medium,
    /// Likely to cause disruption.
    High,
    /// The maintenance must not proceed. A single blocker makes the whole
    /// operation [`MaintenanceStatus::Blocked`](crate::MaintenanceStatus).
    Blocker,
}

/// How much the analyzer's conclusion can be relied on.
///
/// This is not decoration. An aggregate capacity check is
/// [`Confidence::Heuristic`] because it cannot evaluate scheduler predicates,
/// and saying so is the difference between a tool an operator trusts and one
/// they learn to ignore.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Follows directly from cluster state. The API server itself would refuse
    /// the eviction.
    Certain,
    /// Very likely, based on declared constraints, but the scheduler has the
    /// final say.
    Likely,
    /// An approximation. Necessary but not sufficient, or based on an
    /// assumption stated in `limitations`.
    Heuristic,
}

/// One piece of raw evidence: a field value the conclusion rests on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// The object this came from.
    pub resource: ResourceRef,
    /// JSON path within the object, for example `.status.disruptionsAllowed`.
    pub field_path: String,
    /// The observed value, rendered as a string.
    pub value: String,
    /// Why this value matters to the finding.
    pub note: Option<String>,
}

/// A reproducible calculation.
///
/// `inputs` and `formula` exist so a reader can redo the arithmetic by hand and
/// get `result`. A number without its derivation is an assertion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calculation {
    /// Named inputs, in the order they appear in the formula.
    pub inputs: Vec<(String, String)>,
    /// The formula, written so a human can follow it.
    pub formula: String,
    /// The result.
    pub result: String,
    /// Units, where they are not obvious.
    pub unit: Option<String>,
}

/// A suggested fix. Never applied automatically — FleetForge constructs no
/// mutating client at all (ADR-0012).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remediation {
    /// What to do, in plain language.
    pub description: String,
    /// A command the operator can run, when one applies.
    pub command: Option<String>,
    /// What this trades away. Relaxing a PDB reduces availability guarantees,
    /// and the operator should be told that in the same breath.
    pub tradeoff: Option<String>,
}

/// A single evidence-backed conclusion about proposed maintenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Stable identifier.
    pub id: FindingId,
    /// Where this finding came from. Always [`Mode::WhatIf`](crate::Mode),
    /// because a finding is a calculation.
    pub provenance: Provenance,
    /// Severity.
    pub severity: Severity,
    /// One-line summary.
    pub title: String,
    /// The exact objects affected, with UID and resourceVersion.
    pub affected: Vec<ResourceRef>,
    /// The raw field values behind the conclusion.
    pub evidence: Vec<Evidence>,
    /// The arithmetic, where there is any.
    pub calculation: Option<Calculation>,
    /// Plain-language explanation. No hedging.
    pub explanation: String,
    /// Suggested fixes.
    pub remediation: Vec<Remediation>,
    /// How much this can be relied on.
    pub confidence: Confidence,
    /// What this check does **not** prove. Required.
    pub limitations: Vec<String>,
    /// The exact snapshot this was computed against.
    pub snapshot_id: SnapshotId,
}

impl Finding {
    /// Whether this finding forbids the maintenance.
    #[must_use]
    pub const fn is_blocker(&self) -> bool {
        matches!(self.severity, Severity::Blocker)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn severity_orders_blocker_highest() {
        assert!(Severity::Blocker > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Info < Severity::Low);
    }

    #[test]
    fn severity_max_of_a_set_is_the_worst() {
        let severities = [Severity::Low, Severity::Blocker, Severity::Medium];
        assert_eq!(severities.iter().max(), Some(&Severity::Blocker));
    }
}
