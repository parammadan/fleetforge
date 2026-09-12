//! A compact snapshot builder for analyzer tests.
//!
//! Shared by the analyzer tests so each one states only what it is about. A
//! test for the toleration analyzer should read as "a tainted node and a pod
//! without a toleration", not as forty lines of struct literal.

// A test-support module, not a published API: doc comments on every helper
// would be noise, and the helpers are named to read as prose at the call site.
#![allow(dead_code, missing_docs, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use chrono::{DateTime, TimeZone, Utc};
use ff_core::{
    Bytes, ClusterSnapshot, CollectionStatus, ConditionStatus, ContainerFact, KindCoverage,
    LabelSelector, MaintenanceRequest, Millicores, Mode, NodeCondition, NodeFact, PdbFact, PodFact,
    PodPhase, Provenance, ResourceRef, Taint, TaintEffect, Toleration, TolerationOperator,
    VolumeFact, VolumeKind, WorkloadFact, WorkloadKind,
};

pub const CLUSTER: &str = "test-cluster";

pub fn at(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0)
        .single()
        .expect("valid timestamp")
}

fn provenance(kind: &str, namespace: Option<&str>, name: &str) -> Provenance {
    Provenance::live(CLUSTER, "", "v1", kind, at(1_000)).with_object(
        namespace.map(ToOwned::to_owned),
        Some(format!("uid-{name}")),
        Some("100".into()),
    )
}

pub fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

pub struct NodeBuilder(NodeFact);

pub fn node(name: &str) -> NodeBuilder {
    NodeBuilder(NodeFact {
        provenance: provenance("Node", None, name),
        name: name.to_owned(),
        labels: BTreeMap::new(),
        taints: Vec::new(),
        conditions: vec![NodeCondition {
            condition_type: "Ready".into(),
            status: ConditionStatus::True,
            reason: None,
            last_transition_at: Some(at(900)),
        }],
        capacity_cpu: Millicores::from_cores(4),
        capacity_memory: Bytes::from_gib(8),
        allocatable_cpu: Millicores::from_cores(4),
        allocatable_memory: Bytes::from_gib(8),
        availability_zone: None,
        instance_type: None,
        unschedulable: false,
        bottlerocket_version: None,
        kubelet_version: Some("v1.37.0".into()),
    })
}

impl NodeBuilder {
    pub fn cpu(mut self, cores: i64) -> Self {
        self.0.allocatable_cpu = Millicores::from_cores(cores);
        self.0.capacity_cpu = self.0.allocatable_cpu;
        self
    }
    pub fn memory_gib(mut self, gib: i64) -> Self {
        self.0.allocatable_memory = Bytes::from_gib(gib);
        self.0.capacity_memory = self.0.allocatable_memory;
        self
    }
    pub fn labelled(mut self, pairs: &[(&str, &str)]) -> Self {
        self.0.labels = labels(pairs);
        self
    }
    pub fn zone(mut self, zone: &str) -> Self {
        self.0.availability_zone = Some(zone.to_owned());
        self
    }
    pub fn tainted(mut self, key: &str, value: Option<&str>, effect: TaintEffect) -> Self {
        self.0.taints.push(Taint {
            key: key.to_owned(),
            value: value.map(ToOwned::to_owned),
            effect,
        });
        self
    }
    pub fn cordoned(mut self) -> Self {
        self.0.unschedulable = true;
        self
    }
    pub fn not_ready(mut self) -> Self {
        self.0.conditions = vec![NodeCondition {
            condition_type: "Ready".into(),
            status: ConditionStatus::False,
            reason: Some("KubeletNotReady".into()),
            last_transition_at: Some(at(900)),
        }];
        self
    }
    pub fn build(self) -> NodeFact {
        self.0
    }
}

pub struct PodBuilder(PodFact);

pub fn pod(namespace: &str, name: &str, node_name: &str) -> PodBuilder {
    PodBuilder(PodFact {
        provenance: provenance("Pod", Some(namespace), name),
        namespace: namespace.to_owned(),
        name: name.to_owned(),
        labels: BTreeMap::new(),
        node_name: Some(node_name.to_owned()),
        phase: PodPhase::Running,
        owners: Vec::new(),
        containers: vec![ContainerFact {
            name: "app".into(),
            cpu_request: Some(Millicores(250)),
            memory_request: Some(Bytes::from_mib(256)),
            cpu_limit: None,
            memory_limit: None,
        }],
        node_selector: BTreeMap::new(),
        tolerations: Vec::new(),
        required_node_affinity: Vec::new(),
        required_pod_anti_affinity: Vec::new(),
        topology_spread: Vec::new(),
        volumes: Vec::new(),
        priority: None,
        priority_class_name: None,
        termination_grace_period_seconds: Some(30),
    })
}

impl PodBuilder {
    pub fn labelled(mut self, pairs: &[(&str, &str)]) -> Self {
        self.0.labels = labels(pairs);
        self
    }
    pub fn owned_by(mut self, kind: &str, name: &str) -> Self {
        self.0.owners = vec![ResourceRef {
            kind: kind.to_owned(),
            namespace: Some(self.0.namespace.clone()),
            name: name.to_owned(),
            uid: Some(format!("uid-{name}")),
            resource_version: None,
        }];
        self
    }
    pub fn requesting(mut self, cpu_milli: i64, mem_mib: i64) -> Self {
        self.0.containers = vec![ContainerFact {
            name: "app".into(),
            cpu_request: Some(Millicores(cpu_milli)),
            memory_request: Some(Bytes::from_mib(mem_mib)),
            cpu_limit: None,
            memory_limit: None,
        }];
        self
    }
    pub fn selecting(mut self, pairs: &[(&str, &str)]) -> Self {
        self.0.node_selector = labels(pairs);
        self
    }
    pub fn tolerating(mut self, key: &str, effect: TaintEffect) -> Self {
        self.0.tolerations.push(Toleration {
            key: Some(key.to_owned()),
            operator: TolerationOperator::Exists,
            value: None,
            effect: Some(effect),
            toleration_seconds: None,
        });
        self
    }
    pub fn with_volume(mut self, name: &str, kind: VolumeKind) -> Self {
        self.0.volumes.push(VolumeFact {
            name: name.to_owned(),
            kind,
            claim_name: None,
            zones: Vec::new(),
        });
        self
    }
    pub fn build(self) -> PodFact {
        self.0
    }
}

pub fn workload(
    kind: WorkloadKind,
    namespace: &str,
    name: &str,
    desired: i32,
    selector: &[(&str, &str)],
) -> WorkloadFact {
    WorkloadFact {
        provenance: provenance("Deployment", Some(namespace), name),
        kind,
        namespace: namespace.to_owned(),
        name: name.to_owned(),
        desired_replicas: Some(desired),
        ready_replicas: Some(desired),
        selector: LabelSelector {
            match_labels: labels(selector),
            match_expressions: Vec::new(),
        },
    }
}

pub fn pdb(
    namespace: &str,
    name: &str,
    selector: &[(&str, &str)],
    min_available: i32,
    current_healthy: i32,
    disruptions_allowed: i32,
) -> PdbFact {
    PdbFact {
        provenance: provenance("PodDisruptionBudget", Some(namespace), name),
        namespace: namespace.to_owned(),
        name: name.to_owned(),
        selector: LabelSelector {
            match_labels: labels(selector),
            match_expressions: Vec::new(),
        },
        min_available: Some(min_available.to_string()),
        max_unavailable: None,
        current_healthy,
        desired_healthy: min_available,
        expected_pods: current_healthy,
        disruptions_allowed,
    }
}

/// All kinds reported as authoritatively collected.
pub fn full_coverage() -> Vec<KindCoverage> {
    [
        "Node",
        "Pod",
        "Deployment",
        "StatefulSet",
        "DaemonSet",
        "ReplicaSet",
        "PodDisruptionBudget",
        "Event",
    ]
    .iter()
    .map(|kind| KindCoverage {
        kind: (*kind).to_owned(),
        status: CollectionStatus::InSync {
            synced_at: at(1_000),
        },
        observed_count: 0,
    })
    .collect()
}

/// Coverage with one kind denied by RBAC.
pub fn coverage_missing(kind: &str) -> Vec<KindCoverage> {
    full_coverage()
        .into_iter()
        .map(|mut c| {
            if c.kind == kind {
                c.status = CollectionStatus::Forbidden {
                    verb: "list".into(),
                    resource: format!("{}s", kind.to_lowercase()),
                };
            }
            c
        })
        .collect()
}

pub fn snapshot(
    nodes: Vec<NodeFact>,
    pods: Vec<PodFact>,
    workloads: Vec<WorkloadFact>,
    pdbs: Vec<PdbFact>,
    coverage: Vec<KindCoverage>,
) -> ClusterSnapshot {
    ClusterSnapshot::new(
        at(1_000),
        Mode::Live,
        CLUSTER.to_owned(),
        nodes,
        pods,
        workloads,
        pdbs,
        Vec::new(),
        coverage,
    )
    .expect("snapshot builds")
}

pub fn request(snapshot: &ClusterSnapshot, nodes: &[&str], concurrency: u32) -> MaintenanceRequest {
    MaintenanceRequest {
        snapshot_id: snapshot.snapshot_id().clone(),
        node_names: nodes.iter().map(|n| (*n).to_owned()).collect(),
        desired_concurrency: concurrency,
    }
}
