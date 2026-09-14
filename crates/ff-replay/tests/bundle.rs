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

/* ---------------------------------------------------------------------------
 * Malformed and incomplete bundles.
 *
 * Each of these builds a copy of the real bundle with exactly one thing broken,
 * so a failure names the thing that broke rather than "the bundle is bad".
 * ------------------------------------------------------------------------- */

/// Copy the real bundle into a temporary directory so it can be damaged.
fn corrupted_copy(damage: impl FnOnce(&std::path::Path)) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for entry in std::fs::read_dir(bundle_path()).expect("read bundle") {
        let entry = entry.expect("entry");
        if entry.path().is_file() {
            std::fs::copy(entry.path(), dir.path().join(entry.file_name())).expect("copy");
        }
    }
    damage(dir.path());
    dir
}

#[test]
fn a_bundle_whose_event_log_is_not_json_is_rejected() {
    let dir = corrupted_copy(|p| {
        std::fs::write(p.join("35-fleetforge-events-complete.jsonl"), "{not json\n").unwrap();
    });
    let Err(err) = ReplayBundle::load(dir.path()) else {
        panic!("must not load");
    };
    assert!(
        format!("{err}").contains("35-fleetforge-events-complete.jsonl"),
        "the error must name the artifact: {err}"
    );
}

#[test]
fn a_bundle_with_an_empty_event_log_is_rejected_rather_than_serving_a_blank_replay() {
    let dir = corrupted_copy(|p| {
        std::fs::write(p.join("35-fleetforge-events-complete.jsonl"), "").unwrap();
    });
    // An empty timeline would render as a cluster with nothing in it, which is
    // exactly the confusion between "no data" and "no resources" this project
    // exists to prevent.
    assert!(ReplayBundle::load(dir.path()).is_err());
}

#[test]
fn a_bundle_missing_the_blocker_finding_is_rejected() {
    let dir = corrupted_copy(|p| {
        let text = std::fs::read_to_string(p.join("03-preflight-before.json")).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
        // Remove FF-PDB-001 specifically: the bundle is well-formed, it just no
        // longer contains the thing the whole replay is about.
        if let Some(findings) = v
            .pointer_mut("/data/findings")
            .and_then(|f| f.as_array_mut())
        {
            findings.retain(|f| f.get("id").and_then(|i| i.as_str()) != Some("FF-PDB-001"));
        }
        std::fs::write(p.join("03-preflight-before.json"), v.to_string()).unwrap();
    });
    let Err(err) = ReplayBundle::load(dir.path()) else {
        panic!("must not load");
    };
    assert!(format!("{err}").contains("FF-PDB-001"), "{err}");
}

#[test]
fn a_bundle_whose_pdb_lost_its_status_is_rejected_not_defaulted_to_zero() {
    let dir = corrupted_copy(|p| {
        let text = std::fs::read_to_string(p.join("04-pdb-before.json")).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
        if let Some(items) = v.get_mut("items").and_then(|i| i.as_array_mut()) {
            for item in items.iter_mut() {
                if let Some(o) = item.as_object_mut() {
                    o.remove("status");
                }
            }
        }
        std::fs::write(p.join("04-pdb-before.json"), v.to_string()).unwrap();
    });
    // Defaulting a missing currentHealthy to 0 would render "currentHealthy = 0
    // of 0" as though it were an observation.
    let Err(err) = ReplayBundle::load(dir.path()) else {
        panic!("must not load");
    };
    assert!(format!("{err}").contains("04-pdb-before.json"), "{err}");
}

#[test]
fn a_bundle_with_no_traffic_samples_is_rejected_rather_than_reporting_zero_percent() {
    let dir = corrupted_copy(|p| {
        std::fs::write(p.join("31-traffic-post-recovery.txt"), "\n\n").unwrap();
    });
    // Zero of zero requests is not 0% availability; it is no measurement.
    assert!(ReplayBundle::load(dir.path()).is_err());
}

/* ---------------------------------------------------------------------------
 * Timestamps and ordering.
 * ------------------------------------------------------------------------- */

#[test]
fn every_event_timestamp_is_utc_and_non_decreasing() {
    let b = bundle();
    let mut previous: Option<chrono::DateTime<chrono::Utc>> = None;
    for event in b.timeline.events() {
        if let Some(prev) = previous {
            assert!(
                event.at >= prev,
                "event {} went backwards in time: {} < {}",
                event.seq,
                event.at,
                prev
            );
        }
        previous = Some(event.at);
    }
    // The capture window is the first and last event, not a stored constant.
    assert_eq!(b.context.captured_from, b.timeline.events()[0].at);
    assert_eq!(
        b.context.captured_to,
        b.timeline.events()[b.timeline.len() - 1].at
    );
}

#[test]
fn sequence_numbers_are_read_from_the_log_not_generated_from_position() {
    let b = bundle();
    let seqs: Vec<u64> = b.timeline.events().iter().map(|e| e.seq).collect();
    assert!(
        seqs.windows(2).all(|w| w[0] < w[1]),
        "not strictly increasing"
    );

    // This capture's sequence numbers happen to be dense — FleetForge restarted
    // several times but kept counting, so `seq` and `index + 1` agree. That
    // makes the two indistinguishable by value, so the test reads the raw log
    // line instead: `seq` must be the number the recorder wrote, not one this
    // crate invented while loading.
    let raw = std::fs::read_to_string(bundle_path().join("35-fleetforge-events-complete.jsonl"))
        .expect("the event log reads");
    let from_log: Vec<u64> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l)
                .expect("each line is JSON")
                .get("seq")
                .and_then(serde_json::Value::as_u64)
                .expect("each line carries seq")
        })
        .collect();
    assert_eq!(
        seqs, from_log,
        "sequence numbers diverge from the captured log"
    );
}

#[test]
fn the_timeline_contains_a_real_gap_in_time_that_is_not_smoothed_over() {
    let b = bundle();
    // Nothing was recorded for a stretch in the middle of the incident. A
    // replay that interpolated would hide that; this asserts the gap survives.
    let longest = b
        .timeline
        .events()
        .windows(2)
        .map(|w| (w[1].at - w[0].at).num_seconds())
        .max()
        .expect("events exist");
    assert!(
        longest > 60,
        "expected a quiet stretch of more than a minute, longest was {longest}s"
    );
}

#[test]
fn the_brupop_start_predates_the_recording_and_is_read_from_evidence() {
    let b = bundle();
    let brupop = b
        .context
        .brupop_first_seen_at
        .expect("the bundle can date Brupop's start");
    assert!(
        brupop < b.context.captured_from,
        "Brupop started at {brupop}, recording at {} — the whole 'did not predict' \
         claim rests on this ordering",
        b.context.captured_from
    );
}

/* ---------------------------------------------------------------------------
 * State reconstruction at the timestamps that matter.
 * ------------------------------------------------------------------------- */

/// The position of a named chapter, so these tests describe moments rather than
/// magic indices that shift whenever chapter derivation changes.
fn at_chapter(b: &ReplayBundle, id: &str) -> ff_replay::ReplayState {
    let chapter = b
        .chapters
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no chapter {id}"));
    b.timeline.state_at(chapter.position)
}

#[test]
fn at_the_blocker_two_nodes_are_cordoned_and_a_pod_is_pending() {
    let b = bundle();
    let s = at_chapter(&b, "blocker-detected");
    assert_eq!(s.cordoned_nodes, 2);
    assert_eq!(s.pending_pods, 1);
    let preflight = s.last_preflight.expect("a preflight has run by now");
    assert_eq!(preflight.status, "blocked");
    assert!(preflight.findings.iter().any(|f| f == "FF-PDB-001"));
}

#[test]
fn after_the_first_uncordon_one_node_remains_cordoned() {
    let b = bundle();
    let s = at_chapter(&b, "first-uncordon");
    assert_eq!(s.cordoned_nodes, 1, "one uncordon clears exactly one node");
}

#[test]
fn after_the_second_uncordon_nothing_is_cordoned() {
    let b = bundle();
    let s = at_chapter(&b, "second-uncordon");
    assert_eq!(s.cordoned_nodes, 0);
}

#[test]
fn by_the_end_every_node_reports_the_final_release() {
    let b = bundle();
    let s = b.timeline.state_at(b.timeline.len() - 1);
    assert_eq!(s.nodes.len(), 3);
    for node in &s.nodes {
        assert_eq!(
            node.bottlerocket_version.as_deref(),
            Some("1.64.0"),
            "{} did not reach the final release",
            node.name
        );
    }
}

#[test]
fn state_never_reports_more_nodes_than_the_capture_contains() {
    let b = bundle();
    // A fold that accumulated a node per event rather than per name would grow
    // without bound, and nothing else would notice.
    for position in [0, 1, 40, 731, 2932, 4669, b.timeline.len() - 1] {
        let s = b.timeline.state_at(position);
        assert!(
            s.nodes.len() <= 3,
            "{position} produced {} nodes",
            s.nodes.len()
        );
    }
}

/* ---------------------------------------------------------------------------
 * The investigation chain.
 * ------------------------------------------------------------------------- */

#[test]
fn every_chain_fact_is_observed_or_derived_and_every_arrow_is_human_rca() {
    let b = bundle();
    assert_eq!(b.chain.links.len(), 5);
    assert_eq!(b.chain.edges.len(), 4);

    for link in &b.chain.links {
        assert!(
            link.basis.is_evidence(),
            "chain fact {} is not evidence: {:?}",
            link.id,
            link.basis
        );
    }
    for edge in &b.chain.edges {
        assert_eq!(
            edge.basis,
            ff_replay::ClaimBasis::HumanRca,
            "arrow {} → {} claims more than a human drew",
            edge.from,
            edge.to
        );
    }
}

#[test]
fn the_chain_edges_form_one_unbroken_path_through_the_links() {
    let b = bundle();
    let ids: Vec<&str> = b.chain.links.iter().map(|l| l.id.as_str()).collect();
    for (i, edge) in b.chain.edges.iter().enumerate() {
        assert_eq!(edge.from, ids[i]);
        assert_eq!(edge.to, ids[i + 1]);
    }
}

#[test]
fn chain_values_come_from_the_bundle_rather_than_from_constants() {
    let b = bundle();
    let by_id = |id: &str| {
        b.chain
            .links
            .iter()
            .find(|l| l.id == id)
            .unwrap_or_else(|| panic!("no link {id}"))
    };
    // Each of these is the arithmetic the incident actually produced. If the
    // strings were written by hand they would survive a change to the parser;
    // these assertions are here to make sure they would not.
    assert!(by_id("cordons").value.contains("2 of 3"));
    assert!(by_id("pending").value.contains("1 pod"));
    assert!(by_id("current-healthy").value.contains("= 2 of 3"));
    assert_eq!(by_id("disruptions-allowed").value, "disruptionsAllowed = 0");
    assert_eq!(
        by_id("disruptions-allowed").basis,
        ff_replay::ClaimBasis::MathematicallyDerived
    );

    for link in &b.chain.links {
        assert!(
            b.artifacts.contains(&link.artifact),
            "{} cites {}, which is not in the bundle",
            link.id,
            link.artifact
        );
    }
}

#[test]
fn the_chain_says_out_loud_that_fleetforge_did_not_draw_it() {
    let b = bundle();
    let text = b.chain.attribution.to_lowercase();
    assert!(text.contains("did not produce the chain"));
    assert!(text.contains("no analyzer"));
}

/* ---------------------------------------------------------------------------
 * The second capture: a prevented run.
 *
 * A different experiment with an opposite conclusion. These tests exist mostly
 * to stop the two blurring together — the incident may not claim prevention,
 * and the prevented run may not claim the incident's excuses.
 * ------------------------------------------------------------------------- */

fn live_bundle() -> ReplayBundle {
    ReplayBundle::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../evidence/eks-live"))
        .expect("the Phase C bundle loads")
}

#[test]
fn the_two_captures_are_detected_as_different_kinds() {
    assert_eq!(bundle().kind, ff_replay::CaptureKind::Incident);
    assert_eq!(live_bundle().kind, ff_replay::CaptureKind::Prevented);
    // Detection is by which event log is present, so a directory cannot be
    // loaded as the wrong kind by a caller passing a bad argument.
    assert_ne!(
        ff_replay::CaptureKind::Incident.event_log(),
        ff_replay::CaptureKind::Prevented.event_log()
    );
}

#[test]
fn a_directory_with_no_event_log_names_both_candidates() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("00-DIAGNOSIS.md"), "not a bundle").unwrap();
    let Err(err) = ReplayBundle::load(dir.path()) else {
        panic!("must not load");
    };
    let text = format!("{err}");
    assert!(
        text.contains("35-fleetforge-events-complete.jsonl"),
        "{text}"
    );
    assert!(text.contains("20-fleetforge-events.jsonl"), "{text}");
}

#[test]
fn the_prevented_run_claims_prevention_and_the_incident_does_not() {
    let inc = bundle();
    let live = live_bundle();

    assert!(
        live.claims.iter().any(|c| c.id == "prevented"),
        "the prevented run must say so"
    );
    assert!(
        !inc.claims.iter().any(|c| c.id == "prevented"),
        "the incident must never claim prevention"
    );
    // And the reverse: the incident's defining admission has no place here.
    assert!(inc.claims.iter().any(|c| c.id == "no-prediction"));
    assert!(!live.claims.iter().any(|c| c.id == "no-prediction"));
}

#[test]
fn prevention_is_earned_by_ordering_not_asserted() {
    let live = live_bundle();
    let events = live.timeline.events();
    let first_preflight = events
        .iter()
        .find(|e| e.kind == "preflight_run")
        .expect("a preflight was recorded");
    let first_brupop = events
        .iter()
        .find(|e| e.kind == "brupop_state_changed")
        .expect("Brupop appears in the log");

    // The whole claim rests on this single comparison.
    assert!(
        first_preflight.at < first_brupop.at,
        "preflight at {} must precede Brupop at {}",
        first_preflight.at,
        first_brupop.at
    );

    let claim = live.claims.iter().find(|c| c.id == "prevented").unwrap();
    assert_eq!(claim.basis, ClaimBasis::ObservedByFleetForge);
    assert!(claim.statement.contains("timestamped before"));
}

#[test]
fn the_prevented_run_records_that_its_prediction_was_wrong() {
    let live = live_bundle();
    // The unflattering pair. A capture that only carried its successes would be
    // marketing, and the interface would have nothing to be trusted about.
    let wrong = live
        .claims
        .iter()
        .find(|c| c.id == "prediction-was-wrong")
        .expect("the miss is recorded");
    assert!(wrong.statement.contains("wrong"));
    assert_eq!(wrong.basis, ClaimBasis::ObservedByFleetForge);

    let untested = live
        .claims
        .iter()
        .find(|c| c.id == "prediction-untested")
        .expect("the unfairness of the test is recorded");
    assert_eq!(
        untested.basis,
        ClaimBasis::Unavailable,
        "an unfair test establishes nothing in either direction"
    );
    assert!(untested.statement.contains("neither"));
}

#[test]
fn the_networking_finding_refutes_without_overreaching() {
    let live = live_bundle();
    let c = live
        .claims
        .iter()
        .find(|c| c.id == "networking-root-cause")
        .expect("the networking finding is present");
    assert_eq!(c.basis, ClaimBasis::MathematicallyDerived);
    assert!(c.statement.contains("not add-on ordering"));
    // It must not claim to have explained the earlier capture's traffic result.
    let limits = c.limitations.join(" ");
    assert!(limits.contains("NOT established"), "{limits}");
    assert!(limits.contains("cannot be re-probed"), "{limits}");
}

#[test]
fn the_prevented_chain_contains_no_counterfactual() {
    let live = live_bundle();
    // "It would have deadlocked" is the tempting sentence here, and it is not a
    // fact — the deadlock did not happen in this run.
    let text = format!(
        "{} {}",
        live.chain.attribution,
        live.chain
            .edges
            .iter()
            .map(|e| e.because.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    )
    .to_lowercase();
    assert!(!text.contains("would have deadlocked") || text.contains("does not claim"));
    assert!(live.chain.attribution.contains("counterfactual"));

    for link in &live.chain.links {
        assert!(
            link.basis.is_evidence(),
            "chain fact {} is not evidence",
            link.id
        );
        assert!(
            live.artifacts.contains(&link.artifact),
            "{} cites {} which is absent",
            link.id,
            link.artifact
        );
    }
}

#[test]
fn the_prevented_traffic_sample_is_not_called_availability() {
    let live = live_bundle();
    let t = &live.traffic;
    assert!(t.requests > 0);
    // It passed, which makes it more dangerous than the incident's failed run:
    // a 99.6% figure is exactly what somebody would screenshot as uptime. So the
    // requirement is not that the word is absent — banning the word would also
    // ban the denial — but that an explicit denial is present.
    let text = t.interpretation.to_lowercase();
    assert!(
        text.contains("not an availability") || text.contains("not a measurement of availability"),
        "the passing sample must deny being an availability figure: {}",
        t.interpretation
    );
    assert!(
        text.contains("one sampler") || text.contains("no real users"),
        "it must say how narrow the measurement is: {}",
        t.interpretation
    );
}

#[test]
fn both_bundles_carry_the_same_schema_version() {
    assert_eq!(bundle().schema_version, live_bundle().schema_version);
}

#[test]
fn the_prevented_run_does_not_inherit_the_incidents_caveats() {
    let live = live_bundle();
    let ids: Vec<&str> = live.caveats.iter().map(|c| c.id.as_str()).collect();

    // Both of these are true of the incident and false here: this run had one
    // run_started marker, and FleetForge was stopped before teardown began.
    // Reusing the incident's builder served them as fact, which is worse than
    // having no caveats at all.
    assert!(!ids.contains(&"restarts"), "restarts is false for this capture: {ids:?}");
    assert!(!ids.contains(&"teardown-tail"), "no teardown is in this capture: {ids:?}");
    assert_eq!(
        live.timeline
            .events()
            .iter()
            .filter(|e| e.kind == "run_started")
            .count(),
        1,
        "the restart caveat would only be honest if this were > 1"
    );

    // And the ones that are true of it are present.
    for expected in [
        "deliberate-condition",
        "unfair-prediction-window",
        "sampler-stopped-early",
    ] {
        assert!(ids.contains(&expected), "missing {expected}: {ids:?}");
    }
}

#[test]
fn the_incident_keeps_its_own_caveats() {
    let ids: Vec<String> = bundle().caveats.iter().map(|c| c.id.clone()).collect();
    assert!(ids.iter().any(|i| i == "version-field-bug"));
    assert!(ids.iter().any(|i| i == "restarts"));
}
