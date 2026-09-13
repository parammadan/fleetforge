//! Tests against the real captured bundle.
//!
//! These run against `evidence/eks-recovery/` rather than a synthetic fixture,
//! because the thing being asserted is that the *actual* evidence parses and
//! says what the interface will claim it says.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use ff_replay::{ClaimBasis, ReplayBundle, ReplayError};

fn bundle_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../evidence/eks-recovery")
}

fn bundle() -> ReplayBundle {
    ReplayBundle::load(bundle_path()).expect("the captured bundle loads")
}

#[test]
fn the_real_bundle_loads_and_validates() {
    let b = bundle();
    assert_eq!(b.schema_version, ff_replay::REPLAY_SCHEMA_VERSION);
    assert!(b.timeline.len() > 4000, "got {}", b.timeline.len());
    assert_eq!(b.context.nodes.len(), 3);
}

#[test]
fn an_incomplete_bundle_is_rejected_not_partially_loaded() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("35-fleetforge-events-complete.jsonl"), "").unwrap();
    assert!(matches!(
        ReplayBundle::load(dir.path()),
        Err(ReplayError::MissingArtifact { .. })
    ));
}

#[test]
fn a_missing_bundle_directory_is_an_error() {
    assert!(matches!(
        ReplayBundle::load("/nonexistent/path/xyz"),
        Err(ReplayError::BundleNotFound { .. })
    ));
}

#[test]
fn replay_state_is_deterministic() {
    let b = bundle();
    for position in [0, 1, 50, 500, b.timeline.len() - 1] {
        assert_eq!(
            b.timeline.state_at(position),
            b.timeline.state_at(position),
            "state_at({position}) is not deterministic"
        );
    }
}

#[test]
fn timeline_is_ordered_by_sequence_and_index() {
    let b = bundle();
    for w in b.timeline.events().windows(2) {
        assert!(w[0].seq <= w[1].seq, "sequence went backwards");
        assert!(w[0].index < w[1].index);
    }
}

#[test]
fn overshooting_the_end_clamps_instead_of_failing() {
    let b = bundle();
    let last = b.timeline.state_at(b.timeline.len() - 1);
    assert_eq!(last, b.timeline.state_at(usize::MAX));
}

#[test]
fn the_pdb_arithmetic_is_parsed_from_evidence_not_hard_coded() {
    let b = bundle();
    assert_eq!(b.pdb.finding_id, "FF-PDB-001");
    assert!(
        b.pdb.formula.contains("currentHealthy"),
        "{}",
        b.pdb.formula
    );
    assert_eq!(b.pdb.result, "0");
    let inputs: Vec<&str> = b.pdb.inputs.iter().map(|(k, _)| k.as_str()).collect();
    assert!(inputs.contains(&"currentHealthy"), "{inputs:?}");
    assert!(inputs.contains(&"desiredHealthy"), "{inputs:?}");
    assert!(
        !b.pdb.limitations.is_empty(),
        "a finding must state limitations"
    );
    assert!(!b.pdb.evidence.is_empty());
}

#[test]
fn availability_during_the_incident_is_classified_unavailable() {
    let b = bundle();
    let c = b
        .claims
        .iter()
        .find(|c| c.id == "availability-unknown")
        .unwrap();
    assert_eq!(c.basis, ClaimBasis::Unavailable);
    assert!(c.statement.contains("UNKNOWN"));
}

#[test]
fn the_causal_chain_is_human_rca_not_fleetforge_inference() {
    let b = bundle();
    let c = b.claims.iter().find(|c| c.id == "causal-chain").unwrap();
    assert_eq!(c.basis, ClaimBasis::HumanRca);
    assert!(
        c.limitations
            .iter()
            .any(|l| l.contains("does not implement"))
    );
}

#[test]
fn the_cni_explanation_is_an_unverified_hypothesis() {
    let b = bundle();
    for id in ["cni-hypothesis", "terraform-correction"] {
        let c = b.claims.iter().find(|c| c.id == id).unwrap();
        assert_eq!(c.basis, ClaimBasis::UnverifiedHypothesis, "{id}");
    }
}

#[test]
fn detection_is_not_claimed_as_prediction() {
    let b = bundle();
    let c = b.claims.iter().find(|c| c.id == "no-prediction").unwrap();
    assert!(c.statement.contains("did NOT predict"), "{}", c.statement);
}

#[test]
fn every_claim_cites_an_artifact_that_exists() {
    let b = bundle();
    for claim in &b.claims {
        for artifact in &claim.evidence {
            assert!(
                b.artifacts.contains(artifact),
                "claim {} cites missing artifact {artifact}",
                claim.id
            );
        }
    }
}

#[test]
fn the_traffic_rerun_is_recorded_as_a_failed_validation() {
    let b = bundle();
    assert!(!b.traffic.passed);
    assert!(b.traffic.requests > 100);
    assert!(
        b.traffic
            .interpretation
            .contains("not a measurement of availability")
    );
    let pct: f64 = b.traffic.success_pct.parse().unwrap();
    assert!((25.0..35.0).contains(&pct), "got {pct}");
}

#[test]
fn under_prediction_is_classified_separately_from_conservative() {
    let b = bundle();
    let under: Vec<_> = b
        .predictions
        .iter()
        .filter(|p| p.class == "under_predicted")
        .collect();
    assert!(
        !under.is_empty(),
        "the run contains a predicted-5/observed-8 case"
    );
    for p in under {
        assert!(p.delta > 0);
        assert!(p.verdict.starts_with("under-predicted"), "{}", p.verdict);
    }
}

#[test]
fn the_version_field_bug_is_disclosed_not_silently_corrected() {
    let b = bundle();
    let caveat = b
        .caveats
        .iter()
        .find(|c| c.id == "version-field-bug")
        .unwrap();
    assert!(caveat.statement.contains("2.0.0"));
    let early = b.timeline.state_at(30);
    assert!(
        early
            .nodes
            .iter()
            .any(|n| n.bottlerocket_version.as_deref() == Some("2.0.0")),
        "the recorded value must survive into replay"
    );
}

#[test]
fn the_banner_context_excludes_the_bogus_version() {
    let b = bundle();
    assert!(!b.context.bottlerocket_versions.iter().any(|v| v == "2.0.0"));
    assert!(
        b.context
            .bottlerocket_versions
            .iter()
            .any(|v| v == "1.64.0")
    );
}

#[test]
fn two_nodes_are_cordoned_early_which_is_the_deadlock_shape() {
    let b = bundle();
    assert_eq!(b.timeline.state_at(40).cordoned_nodes, 2);
}
