//! Preflight results and the recommendation summary.
//!
//! The summary is **output of the analysis engine**, not a rollout controller
//! (ADR-0016). It is a pure calculation over one snapshot: it holds no state,
//! observes nothing over time, and makes no decision between waves.
//!
//! [`RecommendationSummary::concurrency_constraint`] is the part that matters.
//! A bare number is a guess with an interface attached. A number that names the
//! finding which produced it is a claim a reviewer can check.

use serde::{Deserialize, Serialize};

use crate::finding::{Finding, FindingId, Severity};
use crate::provenance::{Mode, Provenance, ResourceRef};
use crate::quantity::{Bytes, Millicores};
use crate::snapshot::SnapshotId;

/// What the operator is asking about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceRequest {
    /// The snapshot to analyze against. Naming it explicitly is what makes the
    /// answer reproducible.
    pub snapshot_id: SnapshotId,
    /// Nodes the operator proposes to take out of service.
    pub node_names: Vec<String>,
    /// How many the operator intends to take at once.
    pub desired_concurrency: u32,
}

/// Whether the proposed maintenance may proceed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceStatus {
    /// No blocker was found. This is not a promise that nothing will go wrong;
    /// it means no analyzer found a reason it must not proceed, subject to the
    /// limitations each analyzer declared.
    Safe,
    /// At least one [`Severity::Blocker`] finding applies.
    Blocked,
}

/// Why the recommended concurrency is what it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintRef {
    /// The finding that produced the number.
    pub finding_id: FindingId,
    /// The resource that constrains it.
    pub resource: ResourceRef,
    /// Plain-language reason, for example "PDB web/pdb-web allows 1 disruption
    /// and node ip-10-0-2-17 holds 2 of its pods".
    pub reason: String,
}

/// Predicted effect on one workload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadImpact {
    /// The workload.
    pub workload: ResourceRef,
    /// How many of its pods sit on the selected nodes.
    pub pods_on_selected_nodes: u32,
    /// How many pods it currently has ready.
    pub ready_replicas: Option<i32>,
    /// How many it wants.
    pub desired_replicas: Option<i32>,
    /// Whether a PodDisruptionBudget currently forbids evicting them.
    pub blocked_by_pdb: bool,
    /// Whether any of its pods cannot move at all — node-bound storage, or an
    /// unsatisfiable placement constraint.
    pub has_immovable_pods: bool,
}

/// Aggregate predicted effect of the proposed maintenance.
///
/// Every field here is a prediction. Milestone 4 scores these against what
/// actually happened and publishes the misses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredictedImpact {
    /// Pods expected to be evicted.
    pub pods_evicted: u32,
    /// Of those, pods a DaemonSet will not reschedule elsewhere.
    pub pods_not_rescheduled: u32,
    /// CPU that must be absorbed by the remaining nodes.
    pub cpu_to_reschedule: Millicores,
    /// Memory that must be absorbed by the remaining nodes.
    pub memory_to_reschedule: Bytes,
    /// Allocatable CPU left after the nodes are removed.
    pub cpu_headroom_after: Millicores,
    /// Allocatable memory left after the nodes are removed.
    pub memory_headroom_after: Bytes,
    /// The smallest number of further voluntary disruptions any affected PDB
    /// will permit. Zero means the next eviction blocks.
    pub minimum_pdb_margin: i32,
}

/// The lightweight rollout guidance.
///
/// Note the absence of waves, stop conditions, and rollback criteria — those
/// belong to the planner, which is deferred (ADR-0011). This summary recommends;
/// it does not sequence and it does not enforce.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendationSummary {
    /// Safe or blocked.
    pub status: MaintenanceStatus,
    /// The largest number of nodes the analysis supports taking at once.
    pub recommended_max_concurrency: u32,
    /// Which finding produced that number. `None` only when nothing constrains
    /// it below the requested concurrency.
    pub concurrency_constraint: Option<ConstraintRef>,
    /// Workloads affected, with predicted disruption.
    pub affected_workloads: Vec<WorkloadImpact>,
    /// The findings behind the status.
    pub evidence: Vec<FindingId>,
    /// Aggregate predicted impact.
    pub predicted_impact: PredictedImpact,
}

/// The parts of a summary an analyzer engine computes.
///
/// Grouped rather than passed as loose arguments because they are one
/// coherent thing: the engine's quantitative view of the proposed maintenance.
/// [`PreflightResult::new`] derives status and evidence itself, so those are
/// deliberately absent here — a caller cannot assert a status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryInputs {
    /// The largest number of nodes the analysis supports taking at once,
    /// before any blocker is taken into account.
    pub recommended_max_concurrency: u32,
    /// Which finding produced that number.
    pub concurrency_constraint: Option<ConstraintRef>,
    /// Workloads affected, with predicted disruption.
    pub affected_workloads: Vec<WorkloadImpact>,
    /// Aggregate predicted impact.
    pub predicted_impact: PredictedImpact,
}

/// The full result of a preflight run.
///
/// Constructed only by [`PreflightResult::new`], which forces
/// [`Mode::WhatIf`] and derives the status from the findings rather than
/// accepting one from a caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightResult {
    /// Always [`Mode::WhatIf`], with the source snapshot recorded.
    pub provenance: Provenance,
    /// What was asked.
    pub request: MaintenanceRequest,
    /// Every finding, most severe first.
    pub findings: Vec<Finding>,
    /// The rollout guidance.
    pub summary: RecommendationSummary,
}

impl PreflightResult {
    /// Assemble a result, deriving status and ordering findings.
    ///
    /// The status is computed here rather than passed in: a caller cannot
    /// report `Safe` while holding a blocker finding, because the constructor
    /// will not let them.
    #[must_use]
    pub fn new(
        cluster_id: impl Into<String>,
        request: MaintenanceRequest,
        mut findings: Vec<Finding>,
        computed_at: chrono::DateTime<chrono::Utc>,
        inputs: SummaryInputs,
    ) -> Self {
        findings.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.id.as_str().cmp(b.id.as_str()))
        });

        let status = if findings.iter().any(Finding::is_blocker) {
            MaintenanceStatus::Blocked
        } else {
            MaintenanceStatus::Safe
        };

        // A blocked operation supports no concurrency at all. Clamping here
        // rather than trusting the caller means the two fields cannot
        // contradict each other in a report.
        let recommended_max_concurrency = match status {
            MaintenanceStatus::Blocked => 0,
            MaintenanceStatus::Safe => inputs.recommended_max_concurrency,
        };

        let provenance = Provenance::computed(cluster_id, request.snapshot_id.clone(), computed_at);

        let summary = RecommendationSummary {
            status,
            recommended_max_concurrency,
            concurrency_constraint: inputs.concurrency_constraint,
            affected_workloads: inputs.affected_workloads,
            evidence: findings.iter().map(|f| f.id.clone()).collect(),
            predicted_impact: inputs.predicted_impact,
        };

        Self {
            provenance,
            request,
            findings,
            summary,
        }
    }

    /// Whether the maintenance is blocked.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self.summary.status, MaintenanceStatus::Blocked)
    }

    /// The most severe finding, if there is one.
    #[must_use]
    pub fn worst_severity(&self) -> Option<Severity> {
        self.findings.iter().map(|f| f.severity).max()
    }

    /// Always [`Mode::WhatIf`].
    ///
    /// A preflight result is a calculation about a future that has not
    /// happened. Presenting it as an event that occurred is a correctness bug,
    /// so the type does not offer a way to say otherwise.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        Mode::WhatIf
    }
}
