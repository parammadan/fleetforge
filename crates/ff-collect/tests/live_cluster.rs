//! Integration tests against a real Kubernetes cluster.
//!
//! Ignored by default: they need a cluster, so a plain `cargo test` on a laptop
//! with no kubeconfig must not fail. Run them explicitly:
//!
//! ```text
//! ./scripts/kind-up.sh
//! ./scripts/make-kubeconfig.sh fleetforge-reader
//! cargo test -p ff-collect --test live_cluster -- --ignored --test-threads=1
//! ```
//!
//! These assert the things unit tests structurally cannot: that the normalizers
//! survive the objects a real API server actually emits, and that RBAC denial
//! surfaces as `Forbidden` rather than as an empty list.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::time::Duration;

use ff_collect::{CollectorConfig, FixtureSource};
use ff_core::{CollectionStatus, Mode};

fn kubeconfig(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("infra/local")
        .join(format!(".kubeconfig-{name}"))
}

/// Wait for the first snapshot, or give up.
async fn first_snapshot(
    collector: &ff_collect::Collector,
) -> std::sync::Arc<ff_core::ClusterSnapshot> {
    for _ in 0..80 {
        if let Some(snapshot) = collector.store().current() {
            return snapshot;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("the first sync did not complete within 20s");
}

#[tokio::test]
#[ignore = "requires a live cluster; see module docs"]
async fn collects_real_cluster_state_read_only() {
    let path = kubeconfig("fleetforge-reader");
    assert!(path.exists(), "run ./scripts/make-kubeconfig.sh first");

    let collector = ff_collect::start(CollectorConfig::default().with_kubeconfig(&path))
        .await
        .expect("connects to the cluster");

    let snapshot = first_snapshot(&collector).await;

    assert_eq!(snapshot.mode(), Mode::Live);
    assert!(
        !snapshot.cluster_id().contains("://"),
        "the cluster id must never be an API server URL"
    );
    assert!(!snapshot.nodes().is_empty(), "a real cluster has nodes");

    // Every fact carries identity that a reader can check against the cluster.
    for node in snapshot.nodes() {
        assert!(node.provenance.uid().is_some(), "{} has no UID", node.name);
        assert!(
            node.provenance.resource_version().is_some(),
            "{} has no resourceVersion",
            node.name
        );
        assert_eq!(node.provenance.mode(), Mode::Live);
        assert!(
            node.allocatable_cpu.0 > 0,
            "{} reported no allocatable CPU — the quantity parser failed on real input",
            node.name
        );
    }

    // The demo PodDisruptionBudget must be visible and parsed from status.
    let pdb = snapshot
        .pdbs()
        .iter()
        .find(|p| p.namespace == "demo" && p.name == "web-pdb")
        .expect("the demo PDB should be collected");
    assert_eq!(pdb.min_available.as_deref(), Some("2"));
    assert!(
        !pdb.blocks_disruption(),
        "web-pdb is permissive by design at M2; M3 tightens it"
    );

    // DaemonSet pods do not reschedule elsewhere, which changes drain arithmetic.
    let daemon_sets: Vec<_> = snapshot
        .workloads()
        .iter()
        .filter(|w| w.kind == ff_core::WorkloadKind::DaemonSet)
        .collect();
    assert!(!daemon_sets.is_empty(), "kube-proxy alone guarantees one");
    assert!(daemon_sets.iter().all(|w| !w.kind.reschedules_elsewhere()));

    // Everything must be authoritative for this identity.
    for coverage in collector.coverage().await {
        assert!(
            coverage.status.is_authoritative(),
            "{} was not collected authoritatively: {:?}",
            coverage.kind,
            coverage.status
        );
    }
}

#[tokio::test]
#[ignore = "requires a live cluster; see module docs"]
async fn rbac_denial_is_forbidden_not_an_empty_list() {
    // The single most important behaviour in the collector. An empty
    // PodDisruptionBudget list means no blockers, which means safe to drain.
    let path = kubeconfig("fleetforge-restricted");
    assert!(
        path.exists(),
        "run ./scripts/make-kubeconfig.sh fleetforge-restricted first"
    );

    let collector = ff_collect::start(CollectorConfig::default().with_kubeconfig(&path))
        .await
        .expect("connects to the cluster");

    let _ = first_snapshot(&collector).await;
    // Give the forbidden watch a moment to report.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let coverage = collector.coverage().await;
    let pdbs = coverage
        .iter()
        .find(|c| c.kind == "PodDisruptionBudget")
        .expect("PDB coverage is always reported");

    match &pdbs.status {
        CollectionStatus::Forbidden { verb, resource } => {
            assert_eq!(verb, "list");
            assert_eq!(resource, "poddisruptionbudgets");
        }
        other => panic!("expected Forbidden from a real 403, got {other:?}"),
    }
    assert!(!pdbs.status.is_authoritative());

    // Kinds this identity *can* read must be unaffected.
    let nodes = coverage.iter().find(|c| c.kind == "Node").unwrap();
    assert!(nodes.status.is_authoritative(), "node access is unaffected");
}

#[tokio::test]
#[ignore = "requires captured fixtures; run ./scripts/capture-fixtures.sh"]
async fn captured_fixtures_are_fixture_mode_not_live() {
    // ADR-0020: these files came from a live cluster, and that does not make
    // them live.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captured");
    if !dir.join("nodes.json").exists() {
        panic!("run ./scripts/capture-fixtures.sh first");
    }

    let snapshot = FixtureSource::new(&dir).load().expect("fixtures load");

    assert_eq!(snapshot.mode(), Mode::Fixture);
    assert!(!snapshot.nodes().is_empty());
    for node in snapshot.nodes() {
        assert_eq!(
            node.provenance.mode(),
            Mode::Fixture,
            "every fact from disk is FIXTURE, whatever its origin"
        );
        assert!(!node.provenance.mode().describes_current_reality());
    }
}
