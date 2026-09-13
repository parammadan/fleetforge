//! Property tests for snapshot identity and the honesty invariants.
//!
//! These are load-bearing. Approval binds to a plan, a plan binds to a
//! snapshot, and staleness is judged by comparing identifiers — all of which
//! is meaningless if the hash is not canonical.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use chrono::{DateTime, TimeZone, Utc};
use ff_core::{
    Bytes, ClusterSnapshot, CollectionStatus, ConditionStatus, ContainerFact, KindCoverage,
    LabelSelector, Millicores, Mode, NodeCondition, NodeFact, PodFact, PodPhase, Provenance,
    WorkloadFact, WorkloadKind,
};
use proptest::prelude::*;

fn at(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0)
        .single()
        .expect("valid timestamp")
}

fn node(name: &str, cpu: i64, observed: i64, mode_live: bool) -> NodeFact {
    let provenance = if mode_live {
        Provenance::live("demo-cluster", "", "v1", "Node", at(observed))
    } else {
        Provenance::fixture("demo-cluster", "two-node.json", at(observed))
    }
    .with_object(None, Some(format!("uid-{name}")), Some("100".into()));

    NodeFact {
        provenance,
        name: name.to_owned(),
        labels: BTreeMap::new(),
        taints: Vec::new(),
        conditions: vec![NodeCondition {
            condition_type: "Ready".into(),
            status: ConditionStatus::True,
            reason: None,
            last_transition_at: Some(at(1_000)),
        }],
        capacity_cpu: Millicores(cpu),
        capacity_memory: Bytes::from_gib(8),
        allocatable_cpu: Millicores(cpu),
        allocatable_memory: Bytes::from_gib(7),
        availability_zone: Some("us-east-2a".into()),
        instance_type: Some("m6g.large".into()),
        unschedulable: false,
        bottlerocket_version: Some("1.19.0".into()),
        brupop_managed: false,
        kubelet_version: Some("v1.30.0".into()),
    }
}

fn pod(namespace: &str, name: &str, node_name: &str, observed: i64) -> PodFact {
    PodFact {
        provenance: Provenance::live("demo-cluster", "", "v1", "Pod", at(observed)).with_object(
            Some(namespace.into()),
            Some(format!("uid-{name}")),
            Some("200".into()),
        ),
        namespace: namespace.to_owned(),
        name: name.to_owned(),
        labels: BTreeMap::from([("app".to_owned(), "web".to_owned())]),
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
    }
}

fn coverage(kind: &str, status: CollectionStatus, count: usize) -> KindCoverage {
    KindCoverage {
        kind: kind.to_owned(),
        status,
        observed_count: count,
    }
}

fn snapshot(
    nodes: Vec<NodeFact>,
    pods: Vec<PodFact>,
    coverage: Vec<KindCoverage>,
    taken_at: i64,
    mode: Mode,
) -> ClusterSnapshot {
    ClusterSnapshot::new(
        at(taken_at),
        mode,
        "demo-cluster".into(),
        ff_core::SnapshotFacts {
            nodes,
            pods,
            coverage,
            ..Default::default()
        },
    )
    .expect("snapshot builds")
}

fn in_sync(secs: i64) -> CollectionStatus {
    CollectionStatus::InSync {
        synced_at: at(secs),
    }
}

// --- Identity ----------------------------------------------------------------

#[test]
fn observation_time_does_not_change_identity() {
    // The same cluster state, looked at twice, is the same state. If polling
    // produced a new identifier every second, "has anything changed?" would be
    // unanswerable and every approval would go stale instantly.
    let a = snapshot(
        vec![node("n1", 2000, 1_000, true)],
        vec![pod("default", "web-1", "n1", 1_000)],
        vec![in_sync(1_000)]
            .into_iter()
            .map(|s| coverage("Node", s, 1))
            .collect(),
        1_000,
        Mode::Live,
    );
    let b = snapshot(
        vec![node("n1", 2000, 9_999, true)],
        vec![pod("default", "web-1", "n1", 9_999)],
        vec![in_sync(9_999)]
            .into_iter()
            .map(|s| coverage("Node", s, 1))
            .collect(),
        9_999,
        Mode::Live,
    );
    assert_eq!(a.snapshot_id(), b.snapshot_id());
}

#[test]
fn fact_delivery_order_does_not_change_identity() {
    // A watch delivers events in whatever order they arrive.
    let nodes_a = vec![node("n1", 2000, 1_000, true), node("n2", 4000, 1_000, true)];
    let nodes_b = vec![node("n2", 4000, 1_000, true), node("n1", 2000, 1_000, true)];
    let cov = vec![coverage("Node", in_sync(1_000), 2)];

    let a = snapshot(nodes_a, Vec::new(), cov.clone(), 1_000, Mode::Live);
    let b = snapshot(nodes_b, Vec::new(), cov, 1_000, Mode::Live);
    assert_eq!(a.snapshot_id(), b.snapshot_id());
}

#[test]
fn a_real_state_change_changes_identity() {
    let cov = vec![coverage("Node", in_sync(1_000), 1)];
    let a = snapshot(
        vec![node("n1", 2000, 1_000, true)],
        Vec::new(),
        cov.clone(),
        1_000,
        Mode::Live,
    );
    let b = snapshot(
        vec![node("n1", 4000, 1_000, true)],
        Vec::new(),
        cov,
        1_000,
        Mode::Live,
    );
    assert_ne!(a.snapshot_id(), b.snapshot_id());
}

#[test]
fn collection_status_is_part_of_identity() {
    // A snapshot that could not list PodDisruptionBudgets is NOT the same as
    // one that listed them and found none. Conflating the two is exactly how a
    // tool reports "safe" when it simply could not see.
    let visible = snapshot(
        Vec::new(),
        Vec::new(),
        vec![coverage("PodDisruptionBudget", in_sync(1_000), 0)],
        1_000,
        Mode::Live,
    );
    let forbidden = snapshot(
        Vec::new(),
        Vec::new(),
        vec![coverage(
            "PodDisruptionBudget",
            CollectionStatus::Forbidden {
                verb: "list".into(),
                resource: "poddisruptionbudgets".into(),
            },
            0,
        )],
        1_000,
        Mode::Live,
    );
    assert_ne!(visible.snapshot_id(), forbidden.snapshot_id());
}

#[test]
fn fixture_data_never_collides_with_live_data() {
    // An approval bound to a plan hash must not validate against a different
    // data mode, so mode participates in the hash.
    let cov = vec![coverage("Node", in_sync(1_000), 1)];
    let live = snapshot(
        vec![node("n1", 2000, 1_000, true)],
        Vec::new(),
        cov.clone(),
        1_000,
        Mode::Live,
    );
    let fixture = snapshot(
        vec![node("n1", 2000, 1_000, false)],
        Vec::new(),
        cov,
        1_000,
        Mode::Fixture,
    );
    assert_ne!(live.snapshot_id(), fixture.snapshot_id());
}

// --- Coverage ----------------------------------------------------------------

#[test]
fn analyzers_can_refuse_to_guess_when_a_kind_was_not_collected() {
    let s = snapshot(
        Vec::new(),
        Vec::new(),
        vec![coverage(
            "PodDisruptionBudget",
            CollectionStatus::Forbidden {
                verb: "list".into(),
                resource: "poddisruptionbudgets".into(),
            },
            0,
        )],
        1_000,
        Mode::Live,
    );
    assert!(s.require_authoritative("PodDisruptionBudget").is_err());
    assert!(
        s.require_authoritative("Node").is_err(),
        "uncollected kinds are not authoritative"
    );
}

// --- Integrity ---------------------------------------------------------------

#[test]
fn a_tampered_snapshot_does_not_deserialize() {
    let s = snapshot(
        vec![node("n1", 2000, 1_000, true)],
        Vec::new(),
        vec![coverage("Node", in_sync(1_000), 1)],
        1_000,
        Mode::Live,
    );
    let mut json: serde_json::Value = serde_json::to_value(&s).unwrap();
    // Edit a fact without recomputing the identifier, as a hand-edited event
    // log or a corrupted fixture would.
    json["nodes"][0]["capacity_cpu"] = serde_json::json!(999_999);

    let result: Result<ClusterSnapshot, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "integrity check must reject a tampered snapshot"
    );
}

#[test]
fn an_untampered_snapshot_round_trips() {
    let s = snapshot(
        vec![node("n1", 2000, 1_000, true), node("n2", 4000, 1_000, true)],
        vec![pod("default", "web-1", "n1", 1_000)],
        vec![coverage("Node", in_sync(1_000), 2)],
        1_000,
        Mode::Live,
    );
    let json = serde_json::to_string(&s).unwrap();
    let back: ClusterSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
    assert_eq!(s.snapshot_id(), back.snapshot_id());
}

// --- Domain helpers ----------------------------------------------------------

#[test]
fn daemonset_pods_are_not_counted_as_rescheduling_elsewhere() {
    // A DaemonSet pod leaves with its node and returns with it. Counting it as
    // needing capacity elsewhere would overstate what a drain requires.
    assert!(!WorkloadKind::DaemonSet.reschedules_elsewhere());
    assert!(WorkloadKind::Deployment.reschedules_elsewhere());
    assert!(WorkloadKind::StatefulSet.reschedules_elsewhere());
}

#[test]
fn an_empty_label_selector_matches_everything() {
    // This is Kubernetes' own behaviour and a common source of accidentally
    // cluster-wide PodDisruptionBudgets, so the model must reproduce it rather
    // than quietly "fixing" it.
    let selector = LabelSelector::default();
    assert!(selector.matches(&BTreeMap::new()));
    assert!(selector.matches(&BTreeMap::from([("app".to_owned(), "web".to_owned())])));
}

#[test]
fn a_singleton_workload_is_recognised() {
    let w = WorkloadFact {
        provenance: Provenance::live("demo-cluster", "apps", "v1", "Deployment", at(1_000)),
        kind: WorkloadKind::Deployment,
        namespace: "default".into(),
        name: "web".into(),
        desired_replicas: Some(1),
        ready_replicas: Some(1),
        selector: LabelSelector::default(),
    };
    assert!(w.is_singleton());
}

// --- Properties --------------------------------------------------------------

proptest! {
    /// Identity depends on content, never on when we looked.
    #[test]
    fn identity_is_independent_of_observation_time(
        t1 in 1i64..1_000_000,
        t2 in 1i64..1_000_000,
        cpu in 100i64..64_000,
    ) {
        let cov = vec![coverage("Node", in_sync(t1), 1)];
        let a = snapshot(vec![node("n1", cpu, t1, true)], Vec::new(), cov, t1, Mode::Live);
        let cov = vec![coverage("Node", in_sync(t2), 1)];
        let b = snapshot(vec![node("n1", cpu, t2, true)], Vec::new(), cov, t2, Mode::Live);
        prop_assert_eq!(a.snapshot_id(), b.snapshot_id());
    }

    /// Distinct cluster states get distinct identifiers.
    #[test]
    fn distinct_capacity_yields_distinct_identity(
        cpu_a in 100i64..64_000,
        cpu_b in 100i64..64_000,
    ) {
        prop_assume!(cpu_a != cpu_b);
        let cov = vec![coverage("Node", in_sync(1_000), 1)];
        let a = snapshot(vec![node("n1", cpu_a, 1_000, true)], Vec::new(), cov.clone(), 1_000, Mode::Live);
        let b = snapshot(vec![node("n1", cpu_b, 1_000, true)], Vec::new(), cov, 1_000, Mode::Live);
        prop_assert_ne!(a.snapshot_id(), b.snapshot_id());
    }

    /// Hashing is a pure function: building the same snapshot twice in the same
    /// process must not drift.
    #[test]
    fn identity_is_stable_across_repeated_construction(cpu in 100i64..64_000) {
        let cov = vec![coverage("Node", in_sync(1_000), 1)];
        let a = snapshot(vec![node("n1", cpu, 1_000, true)], Vec::new(), cov.clone(), 1_000, Mode::Live);
        let b = snapshot(vec![node("n1", cpu, 1_000, true)], Vec::new(), cov, 1_000, Mode::Live);
        prop_assert_eq!(a.snapshot_id(), b.snapshot_id());
    }

    /// Pod ordering from a watch must not leak into identity.
    #[test]
    fn identity_is_independent_of_pod_order(n in 2usize..6) {
        let cov = vec![coverage("Pod", in_sync(1_000), n)];
        let mut forward: Vec<PodFact> = (0..n)
            .map(|i| pod("default", &format!("web-{i}"), "n1", 1_000))
            .collect();
        let mut reversed = forward.clone();
        reversed.reverse();
        forward.rotate_left(1);

        let a = snapshot(Vec::new(), forward, cov.clone(), 1_000, Mode::Live);
        let b = snapshot(Vec::new(), reversed, cov, 1_000, Mode::Live);
        prop_assert_eq!(a.snapshot_id(), b.snapshot_id());
    }
}
