//! The analyzer contract, and a builder that makes findings hard to under-fill.

use chrono::{DateTime, Utc};
use ff_core::{
    Calculation, Confidence, Evidence, Finding, FindingId, Provenance, Remediation, ResourceRef,
    Result, Severity, SnapshotId,
};

use crate::context::AnalysisContext;

/// One evidence-based check.
///
/// Analyzers are independent and registered in a table, so adding a check never
/// requires editing an existing one.
pub trait Analyzer: Send + Sync {
    /// Stable identifier prefix for the findings this analyzer emits.
    fn id_prefix(&self) -> &'static str;

    /// A one-line description of what this analyzer checks.
    fn describes(&self) -> &'static str;

    /// The Kubernetes kinds this analyzer's conclusions depend on.
    ///
    /// The engine refuses to run an analyzer whose inputs were not collected
    /// authoritatively. An analyzer that needs PodDisruptionBudgets must not
    /// report "no blockers" when the collector was forbidden from listing them.
    fn required_kinds(&self) -> &'static [&'static str];

    /// Run the check.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot lacks facts needed for a sound
    /// conclusion.
    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>>;
}

/// Builds a [`Finding`] with every required field supplied.
///
/// `limitations` has no default and no "none" shortcut. An analyzer that has
/// genuinely nothing to disclaim must still say so in words, because the field
/// is read by someone deciding whether to trust the conclusion (ADR-0004).
pub struct FindingBuilder {
    id: FindingId,
    severity: Severity,
    title: String,
    affected: Vec<ResourceRef>,
    evidence: Vec<Evidence>,
    calculation: Option<Calculation>,
    explanation: String,
    remediation: Vec<Remediation>,
    confidence: Confidence,
    limitations: Vec<String>,
}

impl FindingBuilder {
    /// Start a finding.
    #[must_use]
    pub fn new(
        id: &str,
        severity: Severity,
        confidence: Confidence,
        title: impl Into<String>,
    ) -> Self {
        Self {
            id: FindingId::new(id),
            severity,
            title: title.into(),
            affected: Vec::new(),
            evidence: Vec::new(),
            calculation: None,
            explanation: String::new(),
            remediation: Vec::new(),
            confidence,
            limitations: Vec::new(),
        }
    }

    /// Objects this finding is about.
    #[must_use]
    pub fn affecting(mut self, refs: impl IntoIterator<Item = ResourceRef>) -> Self {
        self.affected.extend(refs);
        self
    }

    /// A raw field value the conclusion rests on.
    #[must_use]
    pub fn evidence(
        mut self,
        resource: ResourceRef,
        field_path: &str,
        value: impl std::fmt::Display,
        note: Option<&str>,
    ) -> Self {
        self.evidence.push(Evidence {
            resource,
            field_path: field_path.to_owned(),
            value: value.to_string(),
            note: note.map(ToOwned::to_owned),
        });
        self
    }

    /// The arithmetic, written so a reader can redo it by hand.
    #[must_use]
    pub fn calculation(
        mut self,
        inputs: Vec<(String, String)>,
        formula: impl Into<String>,
        result: impl std::fmt::Display,
        unit: Option<&str>,
    ) -> Self {
        self.calculation = Some(Calculation {
            inputs,
            formula: formula.into(),
            result: result.to_string(),
            unit: unit.map(ToOwned::to_owned),
        });
        self
    }

    /// Plain-language explanation.
    #[must_use]
    pub fn explaining(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = explanation.into();
        self
    }

    /// A suggested fix. Never applied automatically.
    #[must_use]
    pub fn remediation(
        mut self,
        description: impl Into<String>,
        command: Option<String>,
        tradeoff: Option<&str>,
    ) -> Self {
        self.remediation.push(Remediation {
            description: description.into(),
            command,
            tradeoff: tradeoff.map(ToOwned::to_owned),
        });
        self
    }

    /// Something this check does **not** prove.
    #[must_use]
    pub fn limitation(mut self, text: impl Into<String>) -> Self {
        self.limitations.push(text.into());
        self
    }

    /// Finish the finding.
    ///
    /// # Panics
    ///
    /// Never in released code paths: the debug assertions below catch an
    /// under-filled finding during development and tests, which is when an
    /// analyzer author should hear about it.
    #[must_use]
    pub fn build(self, cluster_id: &str, snapshot_id: &SnapshotId, at: DateTime<Utc>) -> Finding {
        debug_assert!(
            !self.limitations.is_empty(),
            "finding {} has no stated limitations; say 'none' explicitly if that is true",
            self.id
        );
        debug_assert!(
            !self.explanation.is_empty(),
            "finding {} has no explanation",
            self.id
        );
        Finding {
            id: self.id,
            provenance: Provenance::computed(cluster_id, snapshot_id.clone(), at),
            severity: self.severity,
            title: self.title,
            affected: self.affected,
            evidence: self.evidence,
            calculation: self.calculation,
            explanation: self.explanation,
            remediation: self.remediation,
            confidence: self.confidence,
            limitations: self.limitations,
            snapshot_id: snapshot_id.clone(),
        }
    }
}
