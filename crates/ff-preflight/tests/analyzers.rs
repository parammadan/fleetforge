//! Analyzer tests.
//!
//! Every analyzer gets at least one case it flags and one it clears. A checker
//! that only ever fires has not been shown to discriminate, and one that only
//! ever passes has not been shown to work at all.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod builder;

use builder::*;
use ff_core::{MaintenanceStatus, Severity, TaintEffect, VolumeKind, WorkloadKind};

fn run(
    snapshot: &ff_core::ClusterSnapshot,
    nodes: &[&str],
    concurrency: u32,
) -> ff_core::PreflightResult {
    let req = request(snapshot, nodes, concurrency);
    ff_preflight::run(snapshot, &req)
}

fn has(result: &ff_core::PreflightResult, id: &str) -> bool {
    result.findings.iter().any(|f| f.id.as_str() == id)
}

fn finding<'a>(result: &'a ff_core::PreflightResult, id: &str) -> &'a ff_core::Finding {
    result
        .findings
        .iter()
        .find(|f| f.id.as_str() == id)
        .unwrap_or_else(|| {
            panic!(
                "expected finding {id}; got {:?}",
                result
                    .findings
                    .iter()
                    .map(|f| f.id.as_str())
                    .collect::<Vec<_>>()
            )
        })
}

// --- Universal invariants ----------------------------------------------------

#[test]
fn every_finding_states_its_limitations() {
    // The rule that keeps the product honest. An analyzer that cannot prove its
    // conclusion must say so, in every finding it emits (ADR-0004).
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![pdb("demo", "web-pdb", &[("app", "web")], 1, 2, 1)],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!result.findings.is_empty());
    for f in &result.findings {
        assert!(
            !f.limitations.is_empty(),
            "finding {} states no limitations",
            f.id
        );
        assert!(
            !f.explanation.is_empty(),
            "finding {} has no explanation",
            f.id
        );
    }
}

#[test]
fn a_preflight_result_is_always_what_if() {
    let snap = snapshot(
        vec![node("n1").build()],
        vec![],
        vec![],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert_eq!(result.mode(), ff_core::Mode::WhatIf);
    assert_eq!(result.provenance.mode(), ff_core::Mode::WhatIf);
    assert!(!result.provenance.mode().describes_current_reality());
}

#[test]
fn analysis_is_deterministic_for_a_given_snapshot() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build(), node("n3").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .build(),
            pod("demo", "web-2", "n2")
                .labelled(&[("app", "web")])
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![pdb("demo", "web-pdb", &[("app", "web")], 1, 2, 1)],
        full_coverage(),
    );
    let a = run(&snap, &["n1", "n2"], 1);
    let b = run(&snap, &["n1", "n2"], 1);
    assert_eq!(
        a.findings.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(),
        b.findings.iter().map(|f| f.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(
        a.summary.recommended_max_concurrency,
        b.summary.recommended_max_concurrency
    );
}

#[test]
fn a_blocked_result_recommends_zero_concurrency() {
    // The two fields cannot be allowed to contradict each other in a report.
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            1,
            &[("app", "web")],
        )],
        vec![pdb("demo", "web-pdb", &[("app", "web")], 1, 1, 0)],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(result.is_blocked());
    assert_eq!(result.summary.recommended_max_concurrency, 0);
    assert!(result.summary.concurrency_constraint.is_some());
}

// --- Coverage gating ---------------------------------------------------------

#[test]
fn a_forbidden_kind_blocks_rather_than_passing_silently() {
    // The failure this exists to prevent: PDBs unreadable, so the PDB analyzer
    // finds nothing, so nothing blocks, so the drain is reported safe.
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        coverage_missing("PodDisruptionBudget"),
    );
    let result = run(&snap, &["n1"], 1);

    assert!(
        result.is_blocked(),
        "an unreadable PodDisruptionBudget list must not produce a safe result"
    );
    let coverage = finding(&result, "FF-COVERAGE-001");
    assert_eq!(coverage.severity, Severity::Blocker);
    assert!(coverage.explanation.contains("did not run"));
}

#[test]
fn full_coverage_does_not_produce_a_coverage_blocker() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![],
        vec![],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-COVERAGE-001"));
}

// --- PodDisruptionBudget -----------------------------------------------------

#[test]
fn flags_a_pdb_that_permits_no_disruption() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build(), node("n3").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-2", "n2")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-3", "n3")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            3,
            &[("app", "web")],
        )],
        // minAvailable 3 of 3 healthy: disruptionsAllowed 0.
        vec![pdb("demo", "web-pdb", &[("app", "web")], 3, 3, 0)],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);

    assert!(result.is_blocked());
    let f = finding(&result, "FF-PDB-001");
    assert_eq!(f.severity, Severity::Blocker);
    assert_eq!(f.confidence, ff_core::Confidence::Certain);

    // The evidence must let a reader redo the arithmetic.
    let calc = f
        .calculation
        .as_ref()
        .expect("a PDB finding shows its arithmetic");
    assert_eq!(calc.result, "0");
    assert!(calc.formula.contains("currentHealthy"));
    assert!(
        f.evidence
            .iter()
            .any(|e| e.field_path == ".status.disruptionsAllowed" && e.value == "0")
    );
    assert!(
        !f.remediation.is_empty(),
        "a blocker should suggest a way out"
    );
}

#[test]
fn clears_a_pdb_with_disruption_headroom() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build(), node("n3").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-2", "n2")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-3", "n3")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            3,
            &[("app", "web")],
        )],
        vec![pdb("demo", "web-pdb", &[("app", "web")], 2, 3, 1)],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);

    assert!(!has(&result, "FF-PDB-001"));
    assert!(
        has(&result, "FF-PDB-003"),
        "the permissive case is still reported"
    );
    assert_eq!(result.summary.status, MaintenanceStatus::Safe);
}

#[test]
fn flags_a_node_holding_more_covered_pods_than_the_budget_allows() {
    // disruptionsAllowed is 1, but this one node holds two covered pods, and a
    // drain evicts them together.
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build(), node("n3").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-2", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-3", "n2")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            3,
            &[("app", "web")],
        )],
        vec![pdb("demo", "web-pdb", &[("app", "web")], 2, 3, 1)],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(has(&result, "FF-PDB-002"));
    assert!(result.is_blocked());
}

// --- Singleton and unmanaged -------------------------------------------------

#[test]
fn flags_a_singleton_workload() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![
            pod("demo", "api-1", "n1")
                .labelled(&[("app", "api")])
                .owned_by("ReplicaSet", "api-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "api",
            1,
            &[("app", "api")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    let f = finding(&result, "FF-SINGLETON-001");
    assert_eq!(f.severity, Severity::High);
    assert!(f.explanation.contains("zero replicas"));
    // Not a blocker: it is the operator's call whether the outage is acceptable.
    assert!(!result.is_blocked());
}

#[test]
fn clears_a_multi_replica_workload() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build(), node("n3").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-2", "n2")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-SINGLETON-001"));
}

#[test]
fn flags_an_unmanaged_pod() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![pod("demo", "debug", "n1").build()],
        vec![],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(has(&result, "FF-UNMANAGED-001"));
    assert!(result.is_blocked());
}

#[test]
fn clears_a_pod_with_a_controller() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-UNMANAGED-001"));
}

// --- Capacity ----------------------------------------------------------------

#[test]
fn flags_insufficient_aggregate_cpu() {
    let snap = snapshot(
        vec![node("n1").cpu(8).build(), node("n2").cpu(1).build()],
        vec![
            pod("demo", "big-1", "n1")
                .labelled(&[("app", "big")])
                .owned_by("ReplicaSet", "big-abc")
                .requesting(4000, 256)
                .build(),
            pod("demo", "big-2", "n1")
                .labelled(&[("app", "big")])
                .owned_by("ReplicaSet", "big-abc")
                .requesting(3000, 256)
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "big",
            2,
            &[("app", "big")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    let f = finding(&result, "FF-CPU-001");
    assert_eq!(f.severity, Severity::Blocker);
    assert_eq!(f.confidence, ff_core::Confidence::Heuristic);
    assert!(
        f.limitations.iter().any(|l| l.contains("aggregate sum")),
        "a capacity finding must disclaim bin-packing"
    );
}

#[test]
fn clears_sufficient_aggregate_cpu() {
    let snap = snapshot(
        vec![node("n1").cpu(4).build(), node("n2").cpu(8).build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .requesting(500, 256)
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-CPU-001"));
    assert!(has(&result, "FF-CPU-002"));
}

#[test]
fn daemonset_pods_do_not_count_toward_capacity_needed_elsewhere() {
    // A DaemonSet pod leaves with its node and returns with it. Counting it
    // would invent a shortfall that does not exist.
    let snap = snapshot(
        vec![node("n1").cpu(4).build(), node("n2").cpu(1).build()],
        vec![
            pod("demo", "agent-1", "n1")
                .labelled(&[("app", "agent")])
                .owned_by("DaemonSet", "agent")
                .requesting(3000, 256)
                .build(),
        ],
        vec![workload(
            WorkloadKind::DaemonSet,
            "demo",
            "agent",
            2,
            &[("app", "agent")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(
        !has(&result, "FF-CPU-001"),
        "a DaemonSet pod must not create a capacity shortfall"
    );
    assert_eq!(result.summary.predicted_impact.pods_not_rescheduled, 1);
}

#[test]
fn cordoned_nodes_do_not_count_as_available_headroom() {
    let snap = snapshot(
        vec![
            node("n1").cpu(4).build(),
            node("n2").cpu(8).cordoned().build(),
        ],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .requesting(2000, 256)
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(
        has(&result, "FF-CPU-001"),
        "a cordoned node accepts no new pods, so its capacity is not headroom"
    );
}

// --- Placement ---------------------------------------------------------------

#[test]
fn flags_a_node_selector_no_remaining_node_satisfies() {
    let snap = snapshot(
        vec![
            node("n1").labelled(&[("disk", "ssd")]).build(),
            node("n2").labelled(&[("disk", "hdd")]).build(),
        ],
        vec![
            pod("demo", "db-1", "n1")
                .labelled(&[("app", "db")])
                .owned_by("ReplicaSet", "db-abc")
                .selecting(&[("disk", "ssd")])
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "db",
            2,
            &[("app", "db")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(has(&result, "FF-SELECTOR-001"));
    assert!(result.is_blocked());
}

#[test]
fn clears_a_node_selector_another_node_satisfies() {
    let snap = snapshot(
        vec![
            node("n1").labelled(&[("disk", "ssd")]).build(),
            node("n2").labelled(&[("disk", "ssd")]).build(),
        ],
        vec![
            pod("demo", "db-1", "n1")
                .labelled(&[("app", "db")])
                .owned_by("ReplicaSet", "db-abc")
                .selecting(&[("disk", "ssd")])
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "db",
            2,
            &[("app", "db")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-SELECTOR-001"));
}

#[test]
fn flags_a_pod_that_tolerates_no_remaining_node() {
    let snap = snapshot(
        vec![
            node("n1").build(),
            node("n2")
                .tainted("dedicated", Some("gpu"), TaintEffect::NoSchedule)
                .build(),
        ],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(has(&result, "FF-TOLERATION-001"));
}

#[test]
fn clears_a_pod_with_a_matching_toleration() {
    let snap = snapshot(
        vec![
            node("n1").build(),
            node("n2")
                .tainted("dedicated", Some("gpu"), TaintEffect::NoSchedule)
                .build(),
        ],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .tolerating("dedicated", TaintEffect::NoSchedule)
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-TOLERATION-001"));
}

#[test]
fn prefer_no_schedule_taints_do_not_block() {
    // They affect scoring, not admission.
    let snap = snapshot(
        vec![
            node("n1").build(),
            node("n2")
                .tainted("soft", None, TaintEffect::PreferNoSchedule)
                .build(),
        ],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-TOLERATION-001"));
}

// --- Storage -----------------------------------------------------------------

#[test]
fn flags_node_bound_storage() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![
            pod("demo", "db-1", "n1")
                .labelled(&[("app", "db")])
                .owned_by("StatefulSet", "db")
                .with_volume("data", VolumeKind::LocalPersistentVolume)
                .build(),
        ],
        vec![workload(
            WorkloadKind::StatefulSet,
            "demo",
            "db",
            2,
            &[("app", "db")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    let f = finding(&result, "FF-STORAGE-001");
    assert_eq!(f.severity, Severity::Blocker);
    assert!(
        result
            .summary
            .affected_workloads
            .iter()
            .any(|w| w.has_immovable_pods)
    );
}

#[test]
fn treats_empty_dir_as_a_judgement_call_not_a_blocker() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![
            pod("demo", "cache-1", "n1")
                .labelled(&[("app", "cache")])
                .owned_by("ReplicaSet", "cache-abc")
                .with_volume("scratch", VolumeKind::EmptyDir)
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "cache",
            2,
            &[("app", "cache")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    let f = finding(&result, "FF-STORAGE-002");
    assert_eq!(f.severity, Severity::Medium);
    assert!(!result.is_blocked());
}

#[test]
fn clears_a_pod_with_movable_storage() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .with_volume("config", VolumeKind::Ephemeral)
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-STORAGE-001"));
    assert!(!has(&result, "FF-STORAGE-002"));
}

// --- Topology ----------------------------------------------------------------

#[test]
fn flags_removing_an_entire_availability_zone() {
    let snap = snapshot(
        vec![
            node("n1").zone("us-east-2a").build(),
            node("n2").zone("us-east-2b").build(),
        ],
        vec![],
        vec![],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    let f = finding(&result, "FF-AZ-001");
    assert_eq!(f.severity, Severity::High);
}

#[test]
fn clears_partial_zone_removal() {
    let snap = snapshot(
        vec![
            node("n1").zone("us-east-2a").build(),
            node("n2").zone("us-east-2a").build(),
            node("n3").zone("us-east-2b").build(),
        ],
        vec![],
        vec![],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-AZ-001"));
}

#[test]
fn flags_taking_every_replica_at_once() {
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build(), node("n3").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-2", "n2")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1", "n2"], 2);
    assert!(has(&result, "FF-AZ-002"));
    assert!(result.is_blocked());
}

// --- No candidate nodes ---------------------------------------------------

#[test]
fn no_schedulable_nodes_produces_one_finding_not_one_per_pod() {
    // Found on a real EKS cluster mid Brupop update: two of three nodes were
    // cordoned, so selecting the third left nowhere to reschedule. Every
    // per-pod placement analyzer fired for every pod — 12 toleration findings,
    // 6 affinity, 3 selector — all restating the same single fact. 34 findings
    // where 14 were useful.
    let snap = snapshot(
        vec![
            node("n1").build(),
            node("n2").cordoned().build(),
            node("n3").cordoned().build(),
        ],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-2", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-3", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            3,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);

    assert!(result.is_blocked());
    let f = finding(&result, "FF-NODES-001");
    assert_eq!(f.severity, Severity::Blocker);
    assert!(f.explanation.contains("cordoned"));
    // It names the in-flight-maintenance case, which is the common cause.
    assert!(f.explanation.contains("Brupop") || f.explanation.contains("maintenance"));

    // The per-pod placement checks must stand down rather than pile on.
    for id in ["FF-TOLERATION-001", "FF-SELECTOR-001", "FF-AFFINITY-001"] {
        assert!(
            !has(&result, id),
            "{id} should stand down when FF-NODES-001 fires"
        );
    }
}

#[test]
fn placement_checks_still_fire_when_some_nodes_remain() {
    // The converse: with a usable node present, per-pod detail is what helps.
    let snap = snapshot(
        vec![
            node("n1").labelled(&[("disk", "ssd")]).build(),
            node("n2").labelled(&[("disk", "hdd")]).build(),
        ],
        vec![
            pod("demo", "db-1", "n1")
                .labelled(&[("app", "db")])
                .owned_by("ReplicaSet", "db-abc")
                .selecting(&[("disk", "ssd")])
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "db",
            2,
            &[("app", "db")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1"], 1);
    assert!(!has(&result, "FF-NODES-001"), "a candidate node exists");
    assert!(
        has(&result, "FF-SELECTOR-001"),
        "per-pod detail is still useful"
    );
}

#[test]
fn no_candidates_but_nothing_to_reschedule_is_not_a_blocker() {
    // Only DaemonSet pods on the node: they leave with it and return with it,
    // so "nowhere to put them" is not a problem that exists.
    let snap = snapshot(
        vec![node("n1").build(), node("n2").cordoned().build()],
        vec![
            pod("demo", "agent-1", "n1")
                .labelled(&[("app", "agent")])
                .owned_by("DaemonSet", "agent")
                .build(),
        ],
        vec![workload(
            WorkloadKind::DaemonSet,
            "demo",
            "agent",
            2,
            &[("app", "agent")],
        )],
        vec![],
        full_coverage(),
    );
    assert!(!has(&run(&snap, &["n1"], 1), "FF-NODES-001"));
}

// --- Concurrency -------------------------------------------------------------

#[test]
fn concurrency_is_the_largest_wave_that_produces_no_blocker() {
    // Taking both nodes at once removes every replica; taking one is fine. The
    // recommendation must be 1, and it must name the finding that limits it.
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build(), node("n3").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-2", "n2")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1", "n2"], 1);

    assert!(!result.is_blocked(), "one at a time is fine");
    assert_eq!(result.summary.recommended_max_concurrency, 1);

    let constraint = result
        .summary
        .concurrency_constraint
        .as_ref()
        .expect("the number must name the finding that produced it");
    assert_eq!(constraint.finding_id.as_str(), "FF-AZ-002");
    assert!(constraint.reason.contains("2 nodes at once"));
}

#[test]
fn the_same_selection_can_be_safe_at_one_and_blocked_at_two() {
    // This is the property that makes the concurrency control meaningful rather
    // than decorative.
    let snap = snapshot(
        vec![node("n1").build(), node("n2").build(), node("n3").build()],
        vec![
            pod("demo", "web-1", "n1")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
            pod("demo", "web-2", "n2")
                .labelled(&[("app", "web")])
                .owned_by("ReplicaSet", "web-abc")
                .build(),
        ],
        vec![workload(
            WorkloadKind::Deployment,
            "demo",
            "web",
            2,
            &[("app", "web")],
        )],
        vec![],
        full_coverage(),
    );
    assert_eq!(
        run(&snap, &["n1", "n2"], 1).summary.status,
        MaintenanceStatus::Safe
    );
    assert_eq!(
        run(&snap, &["n1", "n2"], 2).summary.status,
        MaintenanceStatus::Blocked
    );
}

// --- Request hygiene ---------------------------------------------------------

#[test]
fn an_unknown_node_is_reported_not_silently_dropped() {
    let snap = snapshot(
        vec![node("n1").build()],
        vec![],
        vec![],
        vec![],
        full_coverage(),
    );
    let result = run(&snap, &["n1", "ghost"], 1);
    assert!(has(&result, "FF-REQUEST-001"));
    assert!(result.is_blocked());
}
