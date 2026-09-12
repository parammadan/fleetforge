//! Normalized cluster facts and the immutable snapshot that holds them.
//!
//! A [`ClusterSnapshot`] is identified by a hash of its own contents, so a
//! finding computed months ago can be re-derived exactly (ADR-0002). The hash
//! deliberately excludes observation timestamps: the same cluster state
//! observed twice is the *same* state, and should carry the same identifier.

use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::canonical;
use crate::error::{FleetForgeError, Result};
use crate::provenance::{KindCoverage, Mode, Provenance, ResourceRef};
use crate::quantity::{Bytes, Millicores};

/// The content hash identifying a [`ClusterSnapshot`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SnapshotId(String);

impl SnapshotId {
    /// Wrap an already-computed hex digest.
    ///
    /// Named `unchecked` because it performs no verification; only
    /// [`ClusterSnapshot::new`] and the tests should call it.
    #[must_use]
    pub fn from_hex_unchecked(hex: impl Into<String>) -> Self {
        Self(hex.into())
    }

    /// The full hex digest.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A short prefix, for interfaces and log lines.
    #[must_use]
    pub fn short(&self) -> &str {
        self.0.get(..12).unwrap_or(&self.0)
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{}", self.0)
    }
}

/// The effect of a node taint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TaintEffect {
    /// New pods without a matching toleration are not scheduled here.
    NoSchedule,
    /// The scheduler avoids this node but may still use it.
    PreferNoSchedule,
    /// Running pods without a matching toleration are evicted.
    NoExecute,
}

/// A node taint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Taint {
    /// Taint key.
    pub key: String,
    /// Taint value, if any.
    pub value: Option<String>,
    /// What the taint does.
    pub effect: TaintEffect,
}

/// How a toleration matches a taint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TolerationOperator {
    /// Key and value must both match.
    Equal,
    /// The key must be present; value is ignored.
    Exists,
}

/// A pod toleration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toleration {
    /// Taint key this tolerates; absent with `Exists` means "all taints".
    pub key: Option<String>,
    /// Match semantics.
    pub operator: TolerationOperator,
    /// Required value, for `Equal`.
    pub value: Option<String>,
    /// Effect this tolerates; absent means all effects.
    pub effect: Option<TaintEffect>,
    /// For `NoExecute`, how long the pod may remain after the taint appears.
    pub toleration_seconds: Option<i64>,
}

/// The status of a node condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ConditionStatus {
    /// Condition holds.
    True,
    /// Condition does not hold.
    False,
    /// The kubelet has not reported recently enough to say.
    Unknown,
}

/// A node condition, for example `Ready` or `MemoryPressure`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCondition {
    /// Condition type.
    pub condition_type: String,
    /// Condition status.
    pub status: ConditionStatus,
    /// Machine-readable reason, when reported.
    pub reason: Option<String>,
    /// When it last changed. Part of cluster state, so it is hashed.
    pub last_transition_at: Option<DateTime<Utc>>,
}

/// A normalized Kubernetes node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFact {
    /// Where this fact came from.
    pub provenance: Provenance,
    /// Node name.
    pub name: String,
    /// Node labels, ordered so hashing is deterministic.
    pub labels: BTreeMap<String, String>,
    /// Taints applied to the node.
    pub taints: Vec<Taint>,
    /// Reported conditions.
    pub conditions: Vec<NodeCondition>,
    /// Total CPU.
    pub capacity_cpu: Millicores,
    /// Total memory.
    pub capacity_memory: Bytes,
    /// CPU available to pods.
    pub allocatable_cpu: Millicores,
    /// Memory available to pods.
    pub allocatable_memory: Bytes,
    /// Availability zone, from the standard topology label.
    pub availability_zone: Option<String>,
    /// Instance type, from the standard topology label.
    pub instance_type: Option<String>,
    /// Whether the node is cordoned.
    pub unschedulable: bool,
    /// Bottlerocket OS version, when the node reports one.
    pub bottlerocket_version: Option<String>,
    /// Kubelet version.
    pub kubelet_version: Option<String>,
}

impl NodeFact {
    /// A reference to this node.
    #[must_use]
    pub fn resource_ref(&self) -> ResourceRef {
        ResourceRef {
            kind: "Node".into(),
            namespace: None,
            name: self.name.clone(),
            uid: self.provenance.uid().map(String::from),
            resource_version: self.provenance.resource_version().map(String::from),
        }
    }

    /// Whether the node reports `Ready=True`.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.conditions
            .iter()
            .any(|c| c.condition_type == "Ready" && c.status == ConditionStatus::True)
    }
}

/// Pod lifecycle phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PodPhase {
    /// Accepted but not yet running.
    Pending,
    /// At least one container is running.
    Running,
    /// All containers terminated successfully.
    Succeeded,
    /// All containers terminated, at least one in failure.
    Failed,
    /// State could not be obtained.
    Unknown,
}

impl PodPhase {
    /// Whether this pod currently occupies capacity on its node.
    #[must_use]
    pub const fn occupies_capacity(self) -> bool {
        matches!(self, Self::Pending | Self::Running | Self::Unknown)
    }
}

/// Per-container resource requests and limits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerFact {
    /// Container name.
    pub name: String,
    /// CPU request; absent means unset, which is not the same as zero.
    pub cpu_request: Option<Millicores>,
    /// Memory request.
    pub memory_request: Option<Bytes>,
    /// CPU limit.
    pub cpu_limit: Option<Millicores>,
    /// Memory limit.
    pub memory_limit: Option<Bytes>,
}

/// How a node-selector requirement matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum NodeSelectorOperator {
    /// Label value is in the listed set.
    In,
    /// Label value is not in the listed set.
    NotIn,
    /// Label key is present.
    Exists,
    /// Label key is absent.
    DoesNotExist,
    /// Label value sorts above every listed value.
    Gt,
    /// Label value sorts below every listed value.
    Lt,
}

/// One requirement within a node-selector term.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSelectorRequirement {
    /// Node label key.
    pub key: String,
    /// Match semantics.
    pub operator: NodeSelectorOperator,
    /// Values, for the operators that take them.
    pub values: Vec<String>,
}

/// A node-selector term. Requirements within a term are ANDed; terms are ORed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSelectorTerm {
    /// Requirements against node labels.
    pub match_expressions: Vec<NodeSelectorRequirement>,
    /// Requirements against node fields, for example `metadata.name`.
    pub match_fields: Vec<NodeSelectorRequirement>,
}

/// How a label-selector requirement matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum LabelSelectorOperator {
    /// Value is in the listed set.
    In,
    /// Value is not in the listed set.
    NotIn,
    /// Key is present.
    Exists,
    /// Key is absent.
    DoesNotExist,
}

/// One requirement within a label selector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelSelectorRequirement {
    /// Label key.
    pub key: String,
    /// Match semantics.
    pub operator: LabelSelectorOperator,
    /// Values, for the operators that take them.
    pub values: Vec<String>,
}

/// A Kubernetes label selector, as used by PDBs, Services, and affinity terms.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelSelector {
    /// Exact-match labels, ordered for deterministic hashing.
    pub match_labels: BTreeMap<String, String>,
    /// Expression requirements.
    pub match_expressions: Vec<LabelSelectorRequirement>,
}

impl LabelSelector {
    /// Whether this selector matches a set of labels.
    ///
    /// An empty selector matches everything, which is Kubernetes' own
    /// behaviour and a common source of accidentally cluster-wide PDBs.
    #[must_use]
    pub fn matches(&self, labels: &BTreeMap<String, String>) -> bool {
        let exact_ok = self
            .match_labels
            .iter()
            .all(|(k, v)| labels.get(k).is_some_and(|actual| actual == v));
        if !exact_ok {
            return false;
        }
        self.match_expressions.iter().all(|req| {
            let actual = labels.get(&req.key);
            match req.operator {
                LabelSelectorOperator::In => actual.is_some_and(|v| req.values.contains(v)),
                LabelSelectorOperator::NotIn => actual.is_none_or(|v| !req.values.contains(v)),
                LabelSelectorOperator::Exists => actual.is_some(),
                LabelSelectorOperator::DoesNotExist => actual.is_none(),
            }
        })
    }
}

/// A pod affinity or anti-affinity term.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodAffinityTerm {
    /// Which pods this term is about.
    pub label_selector: LabelSelector,
    /// The node label defining the topology domain, for example
    /// `kubernetes.io/hostname` or `topology.kubernetes.io/zone`.
    pub topology_key: String,
    /// Namespaces considered; empty means the pod's own namespace.
    pub namespaces: Vec<String>,
}

/// What to do when a topology-spread constraint cannot be satisfied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum UnsatisfiableAction {
    /// Refuse to schedule. This one can block a drain.
    DoNotSchedule,
    /// Schedule anyway, preferring balance.
    ScheduleAnyway,
}

/// A topology-spread constraint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopologySpreadConstraint {
    /// Maximum permitted difference between domains.
    pub max_skew: i32,
    /// The node label defining a domain.
    pub topology_key: String,
    /// Behaviour when unsatisfiable.
    pub when_unsatisfiable: UnsatisfiableAction,
    /// Which pods are counted.
    pub label_selector: LabelSelector,
}

/// The kind of storage a pod volume uses.
///
/// This distinction is the whole point of the volume model: node-bound storage
/// means a pod cannot move, regardless of how much capacity is free elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeKind {
    /// Bound to one node's filesystem. The pod cannot be rescheduled.
    HostPath,
    /// A `local` PersistentVolume. Node-bound.
    LocalPersistentVolume,
    /// Lives and dies with the pod on its node.
    EmptyDir,
    /// A network-attached PersistentVolume, potentially zone-bound.
    NetworkPersistentVolume,
    /// ConfigMap, Secret, projected, downward API — movable.
    Ephemeral,
}

impl VolumeKind {
    /// Whether this volume pins its pod to a specific node.
    #[must_use]
    pub const fn pins_to_node(self) -> bool {
        matches!(
            self,
            Self::HostPath | Self::LocalPersistentVolume | Self::EmptyDir
        )
    }
}

/// A volume attached to a pod.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeFact {
    /// Volume name within the pod.
    pub name: String,
    /// What kind of storage it is.
    pub kind: VolumeKind,
    /// Backing claim, for PersistentVolumeClaims.
    pub claim_name: Option<String>,
    /// Zones the underlying volume is restricted to, when known.
    pub zones: Vec<String>,
}

/// A normalized Kubernetes pod.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodFact {
    /// Where this fact came from.
    pub provenance: Provenance,
    /// Namespace.
    pub namespace: String,
    /// Pod name.
    pub name: String,
    /// Pod labels.
    pub labels: BTreeMap<String, String>,
    /// Node this pod is assigned to, if any.
    pub node_name: Option<String>,
    /// Lifecycle phase.
    pub phase: PodPhase,
    /// Owner chain, controller first. Empty means an unmanaged pod, which
    /// nothing will recreate after an eviction.
    pub owners: Vec<ResourceRef>,
    /// Containers, with requests and limits.
    pub containers: Vec<ContainerFact>,
    /// Node selector.
    pub node_selector: BTreeMap<String, String>,
    /// Tolerations.
    pub tolerations: Vec<Toleration>,
    /// Required node affinity terms, ORed together.
    pub required_node_affinity: Vec<NodeSelectorTerm>,
    /// Required pod anti-affinity terms.
    pub required_pod_anti_affinity: Vec<PodAffinityTerm>,
    /// Topology-spread constraints.
    pub topology_spread: Vec<TopologySpreadConstraint>,
    /// Volumes.
    pub volumes: Vec<VolumeFact>,
    /// Effective priority.
    pub priority: Option<i32>,
    /// Priority class name.
    pub priority_class_name: Option<String>,
    /// Termination grace period.
    pub termination_grace_period_seconds: Option<i64>,
}

impl PodFact {
    /// A reference to this pod.
    #[must_use]
    pub fn resource_ref(&self) -> ResourceRef {
        ResourceRef {
            kind: "Pod".into(),
            namespace: Some(self.namespace.clone()),
            name: self.name.clone(),
            uid: self.provenance.uid().map(String::from),
            resource_version: self.provenance.resource_version().map(String::from),
        }
    }

    /// Total CPU requested across containers. Unset requests count as zero,
    /// which is what the scheduler does.
    #[must_use]
    pub fn total_cpu_request(&self) -> Millicores {
        self.containers.iter().fold(Millicores::ZERO, |acc, c| {
            acc.saturating_add(c.cpu_request.unwrap_or(Millicores::ZERO))
        })
    }

    /// Total memory requested across containers.
    #[must_use]
    pub fn total_memory_request(&self) -> Bytes {
        self.containers.iter().fold(Bytes::ZERO, |acc, c| {
            acc.saturating_add(c.memory_request.unwrap_or(Bytes::ZERO))
        })
    }

    /// Whether any volume pins this pod to its current node.
    #[must_use]
    pub fn is_node_bound(&self) -> bool {
        self.volumes.iter().any(|v| v.kind.pins_to_node())
    }

    /// Whether no controller will recreate this pod after eviction.
    #[must_use]
    pub fn is_unmanaged(&self) -> bool {
        self.owners.is_empty()
    }

    /// The controlling workload, if there is one.
    #[must_use]
    pub fn controller(&self) -> Option<&ResourceRef> {
        self.owners.first()
    }
}

/// The kind of workload controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum WorkloadKind {
    /// A Deployment.
    Deployment,
    /// A StatefulSet.
    StatefulSet,
    /// A DaemonSet — one pod per node, so a drain does not reschedule it
    /// elsewhere.
    DaemonSet,
    /// A ReplicaSet.
    ReplicaSet,
}

impl WorkloadKind {
    /// Whether evicting a pod of this workload results in rescheduling onto
    /// another node.
    ///
    /// DaemonSet pods do not move: the pod goes away with the node and returns
    /// when the node does. Counting them as reschedulable would overstate the
    /// capacity a drain requires.
    #[must_use]
    pub const fn reschedules_elsewhere(self) -> bool {
        !matches!(self, Self::DaemonSet)
    }
}

/// A normalized workload controller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadFact {
    /// Where this fact came from.
    pub provenance: Provenance,
    /// Controller kind.
    pub kind: WorkloadKind,
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
    /// Desired replicas, where the kind has them.
    pub desired_replicas: Option<i32>,
    /// Currently ready replicas.
    pub ready_replicas: Option<i32>,
    /// Pod selector.
    pub selector: LabelSelector,
}

impl WorkloadFact {
    /// A reference to this workload.
    #[must_use]
    pub fn resource_ref(&self) -> ResourceRef {
        ResourceRef {
            kind: format!("{:?}", self.kind),
            namespace: Some(self.namespace.clone()),
            name: self.name.clone(),
            uid: self.provenance.uid().map(String::from),
            resource_version: self.provenance.resource_version().map(String::from),
        }
    }

    /// Whether this workload has exactly one replica.
    ///
    /// A singleton has no redundancy: evicting its pod is an outage for as long
    /// as rescheduling takes, whatever the PodDisruptionBudget says.
    #[must_use]
    pub fn is_singleton(&self) -> bool {
        self.desired_replicas == Some(1)
    }
}

/// A normalized PodDisruptionBudget.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdbFact {
    /// Where this fact came from.
    pub provenance: Provenance,
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
    /// Which pods it covers.
    pub selector: LabelSelector,
    /// `minAvailable`, as declared.
    pub min_available: Option<String>,
    /// `maxUnavailable`, as declared.
    pub max_unavailable: Option<String>,
    /// Healthy pods the API server currently counts.
    pub current_healthy: i32,
    /// Healthy pods required.
    pub desired_healthy: i32,
    /// Expected pod count.
    pub expected_pods: i32,
    /// Voluntary disruptions the API server will currently permit.
    pub disruptions_allowed: i32,
}

impl PdbFact {
    /// A reference to this PDB.
    #[must_use]
    pub fn resource_ref(&self) -> ResourceRef {
        ResourceRef {
            kind: "PodDisruptionBudget".into(),
            namespace: Some(self.namespace.clone()),
            name: self.name.clone(),
            uid: self.provenance.uid().map(String::from),
            resource_version: self.provenance.resource_version().map(String::from),
        }
    }

    /// Whether this PDB currently forbids any voluntary disruption.
    #[must_use]
    pub const fn blocks_disruption(&self) -> bool {
        self.disruptions_allowed <= 0
    }
}

/// A normalized Kubernetes event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventFact {
    /// Where this fact came from.
    pub provenance: Provenance,
    /// Namespace.
    pub namespace: Option<String>,
    /// The object the event is about.
    pub involved_object: ResourceRef,
    /// `Normal` or `Warning`.
    pub event_type: String,
    /// Machine-readable reason.
    pub reason: String,
    /// Human-readable message.
    pub message: String,
    /// When it was first seen.
    pub first_seen_at: Option<DateTime<Utc>>,
    /// When it was last seen.
    pub last_seen_at: Option<DateTime<Utc>>,
    /// How many times it has occurred.
    pub count: i32,
}

/// An immutable, content-addressed view of a cluster at one moment.
///
/// Construct with [`ClusterSnapshot::new`], which computes the identifier.
/// Deserialization re-verifies it, so an inconsistent snapshot cannot exist in
/// memory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ClusterSnapshotRepr")]
pub struct ClusterSnapshot {
    snapshot_id: SnapshotId,
    taken_at: DateTime<Utc>,
    mode: Mode,
    cluster_id: String,
    nodes: Vec<NodeFact>,
    pods: Vec<PodFact>,
    workloads: Vec<WorkloadFact>,
    pdbs: Vec<PdbFact>,
    events: Vec<EventFact>,
    coverage: Vec<KindCoverage>,
}

/// Wire representation, so deserialization can re-verify the hash.
#[derive(Deserialize)]
struct ClusterSnapshotRepr {
    snapshot_id: SnapshotId,
    taken_at: DateTime<Utc>,
    mode: Mode,
    cluster_id: String,
    nodes: Vec<NodeFact>,
    pods: Vec<PodFact>,
    workloads: Vec<WorkloadFact>,
    pdbs: Vec<PdbFact>,
    events: Vec<EventFact>,
    coverage: Vec<KindCoverage>,
}

impl TryFrom<ClusterSnapshotRepr> for ClusterSnapshot {
    type Error = FleetForgeError;

    fn try_from(repr: ClusterSnapshotRepr) -> Result<Self> {
        let snapshot = Self::new(
            repr.taken_at,
            repr.mode,
            repr.cluster_id,
            repr.nodes,
            repr.pods,
            repr.workloads,
            repr.pdbs,
            repr.events,
            repr.coverage,
        )?;
        if snapshot.snapshot_id != repr.snapshot_id {
            return Err(FleetForgeError::SnapshotIntegrity {
                declared: repr.snapshot_id.as_str().to_owned(),
                computed: snapshot.snapshot_id.as_str().to_owned(),
            });
        }
        Ok(snapshot)
    }
}

impl ClusterSnapshot {
    /// Build a snapshot, computing its content hash.
    ///
    /// Facts are sorted into canonical order first, so the identifier does not
    /// depend on the order a watch happened to deliver events.
    ///
    /// # Errors
    ///
    /// Returns an error if the facts cannot be canonically serialized.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        taken_at: DateTime<Utc>,
        mode: Mode,
        cluster_id: String,
        mut nodes: Vec<NodeFact>,
        mut pods: Vec<PodFact>,
        mut workloads: Vec<WorkloadFact>,
        mut pdbs: Vec<PdbFact>,
        mut events: Vec<EventFact>,
        mut coverage: Vec<KindCoverage>,
    ) -> Result<Self> {
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        pods.sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));
        workloads.sort_by(|a, b| {
            (&a.namespace, &a.name, a.kind as u8).cmp(&(&b.namespace, &b.name, b.kind as u8))
        });
        pdbs.sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));
        events.sort_by_key(|e| {
            (
                e.involved_object.sort_key(),
                e.reason.clone(),
                e.first_seen_at,
            )
        });
        coverage.sort_by(|a, b| a.kind.cmp(&b.kind));

        let mut snapshot = Self {
            snapshot_id: SnapshotId::from_hex_unchecked(String::new()),
            taken_at,
            mode,
            cluster_id,
            nodes,
            pods,
            workloads,
            pdbs,
            events,
            coverage,
        };
        snapshot.snapshot_id = canonical::content_hash(&snapshot)?;
        Ok(snapshot)
    }

    /// The content hash identifying this snapshot.
    #[must_use]
    pub const fn snapshot_id(&self) -> &SnapshotId {
        &self.snapshot_id
    }

    /// When the snapshot was taken. Not part of the hash: the same cluster
    /// state observed twice is the same state.
    #[must_use]
    pub const fn taken_at(&self) -> DateTime<Utc> {
        self.taken_at
    }

    /// What kind of data this snapshot holds.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// The stable cluster identifier.
    #[must_use]
    pub fn cluster_id(&self) -> &str {
        &self.cluster_id
    }

    /// Nodes, in canonical order.
    #[must_use]
    pub fn nodes(&self) -> &[NodeFact] {
        &self.nodes
    }

    /// Pods, in canonical order.
    #[must_use]
    pub fn pods(&self) -> &[PodFact] {
        &self.pods
    }

    /// Workloads, in canonical order.
    #[must_use]
    pub fn workloads(&self) -> &[WorkloadFact] {
        &self.workloads
    }

    /// PodDisruptionBudgets, in canonical order.
    #[must_use]
    pub fn pdbs(&self) -> &[PdbFact] {
        &self.pdbs
    }

    /// Events, in canonical order.
    #[must_use]
    pub fn events(&self) -> &[EventFact] {
        &self.events
    }

    /// Per-kind collection coverage.
    #[must_use]
    pub fn coverage(&self) -> &[KindCoverage] {
        &self.coverage
    }

    /// Whether a kind was observed well enough for its absence to mean
    /// anything.
    ///
    /// # Errors
    ///
    /// Returns [`FleetForgeError::IncompleteSnapshot`] when the kind was not
    /// collected authoritatively, so an analyzer can refuse to guess rather
    /// than silently reporting "safe".
    pub fn require_authoritative(&self, kind: &str) -> Result<()> {
        match self.coverage.iter().find(|c| c.kind == kind) {
            Some(c) if c.status.is_authoritative() => Ok(()),
            Some(c) => Err(FleetForgeError::IncompleteSnapshot {
                kind: kind.to_owned(),
                reason: c.status.label().to_owned(),
            }),
            None => Err(FleetForgeError::IncompleteSnapshot {
                kind: kind.to_owned(),
                reason: "not collected".to_owned(),
            }),
        }
    }

    /// Pods currently assigned to a node and occupying its capacity.
    #[must_use]
    pub fn pods_on_node(&self, node_name: &str) -> Vec<&PodFact> {
        self.pods
            .iter()
            .filter(|p| p.node_name.as_deref() == Some(node_name) && p.phase.occupies_capacity())
            .collect()
    }
}
