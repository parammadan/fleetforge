//! What gets recorded.
//!
//! One JSON object per line, each self-describing. A reviewer who does not
//! trust FleetForge should be able to `jq` this file and check every claim in a
//! report against it, which is why the log *is* the storage format rather than
//! something exported from one (ADR-0018).

use chrono::{DateTime, Utc};
use ff_core::{
    ClusterSnapshot, EventFact, MaintenanceStatus, Mode, PodPhase, PreflightResult, SnapshotId,
};
use serde::{Deserialize, Serialize};

/// Identifier for one recorded session.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(String);

impl RunId {
    /// Wrap an identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Derive one from a start time, so a run is identifiable without a
    /// random-number source in a crate that is otherwise deterministic.
    #[must_use]
    pub fn from_time(at: DateTime<Utc>) -> Self {
        Self(format!("run-{}", at.format("%Y%m%dT%H%M%SZ")))
    }

    /// The identifier as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A workload identified the way an operator refers to it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkloadKey {
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
}

impl std::fmt::Display for WorkloadKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)
    }
}

/// What FleetForge predicted, flattened so the log line stands alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prediction {
    /// The snapshot analyzed.
    pub snapshot_id: SnapshotId,
    /// Nodes the operator proposed.
    pub node_names: Vec<String>,
    /// The wave size analyzed.
    pub concurrency: u32,
    /// Safe or blocked.
    pub status: MaintenanceStatus,
    /// How many pods were predicted to be evicted.
    pub pods_evicted: u32,
    /// Of those, how many a DaemonSet will not reschedule elsewhere.
    pub pods_not_rescheduled: u32,
    /// Workloads predicted to be disrupted.
    pub affected_workloads: Vec<WorkloadKey>,
    /// Finding ids behind the verdict.
    pub findings: Vec<String>,
    /// Whether the inputs were complete.
    pub authoritative: bool,
}

/// A recorded observation or decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RecordedEvent {
    /// A recording session began.
    RunStarted {
        /// What this run is for, in the operator's words.
        description: String,
        /// The cluster.
        cluster_id: String,
    },
    /// A new cluster snapshot was published.
    SnapshotObserved {
        /// Content hash of the snapshot.
        snapshot_id: SnapshotId,
        /// When it was taken.
        taken_at: DateTime<Utc>,
        /// Node count.
        nodes: usize,
        /// Pod count.
        pods: usize,
        /// Whether every kind was collected authoritatively.
        authoritative: bool,
    },
    /// A preflight analysis ran. This is the prediction that gets scored.
    PreflightRun(Box<Prediction>),
    /// A pod's phase or placement changed.
    PodChanged {
        /// Namespace.
        namespace: String,
        /// Pod name.
        name: String,
        /// The node it is on, if any.
        node: Option<String>,
        /// Its phase.
        phase: PodPhase,
        /// The workload that owns it, where resolvable.
        workload: Option<WorkloadKey>,
    },
    /// A pod disappeared from the cluster.
    PodRemoved {
        /// Namespace.
        namespace: String,
        /// Pod name.
        name: String,
        /// The node it was last seen on.
        node: Option<String>,
        /// The workload that owned it.
        workload: Option<WorkloadKey>,
    },
    /// A node's readiness or scheduling state changed.
    NodeChanged {
        /// Node name.
        name: String,
        /// Whether it reports Ready=True.
        ready: bool,
        /// Whether it is cordoned.
        unschedulable: bool,
        /// Bottlerocket version, when reported.
        bottlerocket_version: Option<String>,
    },
    /// A Kubernetes event was observed.
    KubernetesEvent {
        /// Namespace.
        namespace: Option<String>,
        /// Object kind.
        kind: String,
        /// Object name.
        name: String,
        /// `Normal` or `Warning`.
        event_type: String,
        /// Machine-readable reason.
        reason: String,
        /// Human-readable message.
        message: String,
    },
    /// A Brupop BottlerocketShadow's state changed.
    BrupopStateChanged {
        /// The node the shadow tracks.
        node: String,
        /// The state Brupop reports.
        state: String,
        /// The target Bottlerocket version, when set.
        target_version: Option<String>,
        /// The current version.
        current_version: Option<String>,
    },
    /// The recording session ended.
    RunEnded {
        /// Why it ended.
        reason: String,
    },
}

/// One line of the log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Monotonic sequence number within the run. Gaps mean lost writes.
    pub seq: u64,
    /// When FleetForge wrote this line.
    pub recorded_at: DateTime<Utc>,
    /// The run this belongs to.
    pub run_id: RunId,
    /// What kind of data this is. Recorded on every line so a log read back
    /// from disk cannot be mistaken for live state (ADR-0020).
    pub mode: Mode,
    /// The event.
    #[serde(flatten)]
    pub event: RecordedEvent,
}

impl LogEntry {
    /// The prediction carried by this entry, if it is one.
    #[must_use]
    pub fn prediction(&self) -> Option<&Prediction> {
        match &self.event {
            RecordedEvent::PreflightRun(p) => Some(p),
            _ => None,
        }
    }
}

/// Summarise a snapshot for the log.
#[must_use]
pub fn snapshot_event(snapshot: &ClusterSnapshot, authoritative: bool) -> RecordedEvent {
    RecordedEvent::SnapshotObserved {
        snapshot_id: snapshot.snapshot_id().clone(),
        taken_at: snapshot.taken_at(),
        nodes: snapshot.nodes().len(),
        pods: snapshot.pods().len(),
        authoritative,
    }
}

/// Summarise a Kubernetes event for the log.
#[must_use]
pub fn kubernetes_event(fact: &EventFact) -> RecordedEvent {
    RecordedEvent::KubernetesEvent {
        namespace: fact.namespace.clone(),
        kind: fact.involved_object.kind.clone(),
        name: fact.involved_object.name.clone(),
        event_type: fact.event_type.clone(),
        reason: fact.reason.clone(),
        message: fact.message.clone(),
    }
}

/// Flatten a preflight result into a prediction record.
#[must_use]
pub fn prediction(result: &PreflightResult, authoritative: bool) -> Prediction {
    Prediction {
        snapshot_id: result.request.snapshot_id.clone(),
        node_names: result.request.node_names.clone(),
        concurrency: result.request.desired_concurrency,
        status: result.summary.status,
        pods_evicted: result.summary.predicted_impact.pods_evicted,
        pods_not_rescheduled: result.summary.predicted_impact.pods_not_rescheduled,
        affected_workloads: result
            .summary
            .affected_workloads
            .iter()
            .map(|w| WorkloadKey {
                namespace: w.workload.namespace.clone().unwrap_or_default(),
                name: w.workload.name.clone(),
            })
            .collect(),
        findings: result.findings.iter().map(|f| f.id.to_string()).collect(),
        authoritative,
    }
}
