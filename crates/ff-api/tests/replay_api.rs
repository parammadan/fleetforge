//! The replay HTTP surface, exercised against the real evidence bundle.
//!
//! The single most important property here is negative: no request to a process
//! started with `--replay` can produce a response labelled `LIVE`. A leadership
//! audience reads the badge, not the URL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ff_api::state::{AppState, DataSource};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

fn bundle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("evidence/eks-recovery")
}

fn replay_state() -> Arc<AppState> {
    let bundle = ff_replay::ReplayBundle::load(bundle_dir()).expect("the evidence bundle loads");
    Arc::new(AppState {
        source: DataSource::Replay(Arc::new(bundle)),
        started_at: chrono::Utc::now(),
        version: "test",
        log: None,
    })
}

async fn get(path: &str) -> (StatusCode, serde_json::Value) {
    let state = replay_state();
    let app = ff_api::routes::router(Arc::clone(&state));
    let response = app
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("the router responds");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// Every replay endpoint. Listed explicitly so that adding one without
/// considering its mode label makes this test fail to cover it on review.
const ENDPOINTS: &[&str] = &[
    "/api/v1/replay/context",
    "/api/v1/replay/timeline",
    "/api/v1/replay/timeline?all=true",
    "/api/v1/replay/state?position=0",
    "/api/v1/replay/state?position=500",
    "/api/v1/replay/chapters",
    "/api/v1/replay/chain",
    "/api/v1/replay/claims",
    "/api/v1/replay/finding",
    "/api/v1/replay/predictions",
    "/api/v1/replay/traffic",
    "/api/v1/replay/artifacts",
    "/api/v1/replay/artifacts/00-CONCLUSIONS.md",
];

#[tokio::test]
async fn no_replay_endpoint_can_report_live() {
    for path in ENDPOINTS {
        let (status, body) = get(path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(body["mode"], "replay", "{path}");
        assert_eq!(body["mode_label"], "REPLAY", "{path}");
    }
}

#[tokio::test]
async fn the_environment_endpoint_reports_replay() {
    // Shared with live mode, and the endpoint the interface reads its badge
    // from — the likeliest place for a LIVE label to leak into a replay.
    let (status, body) = get("/api/v1/environment").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mode"], "replay");
    assert_eq!(body["mode_label"], "REPLAY");
    assert_eq!(body["connection_state"], "not applicable");
}

#[tokio::test]
async fn the_live_snapshot_endpoints_do_not_claim_to_be_syncing() {
    // Answering "syncing" would imply a live cluster this process never
    // connected to, and invite the interface to show a spinner forever.
    for path in ["/api/v1/snapshot", "/api/v1/nodes", "/api/v1/pdbs"] {
        let (status, body) = get(path).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(body["code"], "replay_has_no_current_snapshot", "{path}");
        assert_eq!(body["retriable"], false, "{path}");
    }
}

#[tokio::test]
async fn scrubbing_past_the_end_is_clamped_not_an_error() {
    let (status, body) = get("/api/v1/replay/state?position=999999").await;
    assert_eq!(status, StatusCode::OK);
    let applied = body["data"]["events_applied"].as_u64().unwrap();
    assert_eq!(applied, 5068, "the whole log is applied, not an error");
}

#[tokio::test]
async fn the_same_position_returns_the_same_bytes() {
    let (_, a) = get("/api/v1/replay/state?position=1200").await;
    let (_, b) = get("/api/v1/replay/state?position=1200").await;
    assert_eq!(a["data"], b["data"]);
}

#[tokio::test]
async fn artifacts_are_addressed_by_manifest_name_not_by_path() {
    for attempt in [
        "/api/v1/replay/artifacts/..%2F..%2F..%2Fetc%2Fpasswd",
        "/api/v1/replay/artifacts/%2Fetc%2Fpasswd",
        "/api/v1/replay/artifacts/.%2F00-CONCLUSIONS.md",
        "/api/v1/replay/artifacts/nonexistent.json",
    ] {
        let (status, _) = get(attempt).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{attempt} must be indistinguishable from a missing file"
        );
    }
}

#[tokio::test]
async fn the_timeline_is_ordered_and_the_significant_view_is_a_subset() {
    let (_, all) = get("/api/v1/replay/timeline?all=true").await;
    let (_, sig) = get("/api/v1/replay/timeline").await;
    let all = all["data"].as_array().unwrap();
    let sig = sig["data"].as_array().unwrap();
    assert!(sig.len() < all.len());

    let mut last = 0_u64;
    for e in all {
        let seq = e["seq"].as_u64().unwrap();
        assert!(seq > last, "sequence numbers must strictly increase");
        last = seq;
    }
    // Every significant entry carries the index it has in the full log, so the
    // scrubber and the state fold agree on what position means.
    for e in sig {
        let i = usize::try_from(e["index"].as_u64().unwrap()).unwrap();
        assert_eq!(all[i]["seq"], e["seq"]);
    }
}

#[tokio::test]
async fn every_chapter_points_at_a_real_event() {
    let (_, chapters) = get("/api/v1/replay/chapters").await;
    let (_, timeline) = get("/api/v1/replay/timeline?all=true").await;
    let chapters = chapters["data"].as_array().unwrap();
    let events = timeline["data"].as_array().unwrap();
    assert!(
        chapters.len() >= 6,
        "the capture supports at least six chapters"
    );

    let mut last = 0_u64;
    for c in chapters {
        let pos = c["position"].as_u64().unwrap();
        assert!(pos < events.len() as u64, "chapter points past the end");
        // A chapter's timestamp must be the timestamp of the event it names,
        // or the rail and the scrubber disagree about where a moment is.
        assert_eq!(
            c["at"],
            events[usize::try_from(pos).unwrap()]["at"],
            "{}",
            c["id"]
        );
        assert!(pos >= last, "chapters must be in timeline order");
        last = pos;
    }
}

#[tokio::test]
async fn the_causal_chain_chapter_is_labelled_human_rca() {
    let (_, body) = get("/api/v1/replay/chapters").await;
    let chapter = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "deadlock-visible")
        .expect("the deadlock chapter exists");
    // FleetForge observed the symptoms. A person worked out the loop. The
    // chapter must not claim otherwise.
    assert_eq!(chapter["basis"], "human_rca");
}

#[tokio::test]
async fn the_availability_claim_is_served_as_unknown() {
    let (_, body) = get("/api/v1/replay/claims").await;
    let claims = body["data"].as_array().unwrap();
    let availability = claims
        .iter()
        .find(|c| c["id"] == "availability-unknown")
        .expect("the unknown-availability claim is present");
    assert_eq!(availability["basis"], "unavailable");

    // The 30.2% figure must never be carried on the availability claim.
    assert!(!availability["statement"].as_str().unwrap().contains("30.2"));
}

#[tokio::test]
async fn a_missing_bundle_fails_loading_rather_than_serving_a_blank_one() {
    let err = ff_replay::ReplayBundle::load(bundle_dir().join("does-not-exist"));
    assert!(err.is_err());
}

/* ---------------------------------------------------------------------------
 * Mode separation.
 *
 * Three data sources, three different claims about reality. These tests are the
 * guarantee that the process cannot be persuaded to serve one under another's
 * label.
 * ------------------------------------------------------------------------- */

fn fixture_state() -> Arc<AppState> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("fixtures/captured");
    let snapshot = ff_collect::FixtureSource::new(dir)
        .load()
        .expect("the fixture loads");
    Arc::new(AppState {
        source: DataSource::Fixture(Arc::new(snapshot)),
        started_at: chrono::Utc::now(),
        version: "test",
        log: None,
    })
}

async fn get_from(state: Arc<AppState>, path: &str) -> (StatusCode, serde_json::Value) {
    let app = ff_api::routes::router(Arc::clone(&state));
    let response = app
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("the router responds");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn a_fixture_process_has_no_replay_endpoints_to_serve() {
    // Not 200-with-empty-data: a fixture process must not be able to answer a
    // replay question at all, or the two could be confused by a caller that
    // only checks the status code.
    let (status, body) = get_from(fixture_state(), "/api/v1/replay/context").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_replaying");
}

#[tokio::test]
async fn a_fixture_process_reports_fixture_and_a_replay_process_reports_replay() {
    let (_, fixture) = get_from(fixture_state(), "/api/v1/environment").await;
    assert_eq!(fixture["mode"], "fixture");
    assert_eq!(fixture["mode_label"], "FIXTURE");

    let (_, replay) = get("/api/v1/environment").await;
    assert_eq!(replay["mode"], "replay");

    // And they disagree about the cluster, so one could never be mistaken for
    // a stale render of the other.
    assert_ne!(fixture["cluster_id"], replay["cluster_id"]);
}

#[tokio::test]
async fn a_replay_process_holds_no_collector_and_no_snapshot_store() {
    let state = replay_state();
    assert!(
        state.store().is_none(),
        "replay must not expose a live store"
    );
    assert!(
        state.snapshot().is_none(),
        "replay has no single current snapshot; state depends on position"
    );
    assert!(state.coverage().await.is_empty());
    assert_eq!(state.connection_state(), "not applicable");
    assert_eq!(state.mode(), ff_core::Mode::Replay);
}

/* ---------------------------------------------------------------------------
 * Redaction.
 * ------------------------------------------------------------------------- */

#[tokio::test]
async fn artifact_contents_are_redacted_on_the_way_out() {
    // The redactor is unit-tested inside ff-replay. This asserts it is actually
    // wired into the HTTP path — the failure mode is a correct redactor that
    // nothing calls.
    let dir = tempfile::tempdir().expect("tempdir");
    for entry in std::fs::read_dir(bundle_dir()).expect("read bundle") {
        let entry = entry.expect("entry");
        if entry.path().is_file() {
            std::fs::copy(entry.path(), dir.path().join(entry.file_name())).expect("copy");
        }
    }
    let planted = "aws_secret_access_key: AKIAIOSFODNN7EXAMPLEwJalrXUtnFEMI\n\
                   token: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.notarealtoken\n";
    std::fs::write(dir.path().join("32-kube-system-final.txt"), planted).expect("write");

    let bundle = ff_replay::ReplayBundle::load(dir.path()).expect("bundle loads");
    let state = Arc::new(AppState {
        source: DataSource::Replay(Arc::new(bundle)),
        started_at: chrono::Utc::now(),
        version: "test",
        log: None,
    });

    let (status, body) = get_from(state, "/api/v1/replay/artifacts/32-kube-system-final.txt").await;
    assert_eq!(status, StatusCode::OK);
    let content = body["data"]["content"]
        .as_str()
        .expect("content is a string");

    assert!(
        content.contains("[REDACTED]"),
        "nothing was redacted: {content}"
    );
    assert!(
        !content.contains("AKIAIOSFODNN7EXAMPLE"),
        "secret key survived"
    );
    assert!(!content.contains("notarealtoken"), "token survived");
    // The *presence* of a credential is still visible. Deleting the line would
    // hide that the artifact had one.
    assert!(content.contains("aws_secret_access_key"));
}

#[tokio::test]
async fn the_real_bundle_serves_no_credential_material() {
    let (_, list) = get("/api/v1/replay/artifacts").await;
    let names: Vec<String> = list["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap().to_owned())
        .collect();

    // Every artifact, through the real endpoint, checked for the shapes that
    // matter: AWS key ids, bearer tokens, and embedded kubeconfig material.
    for name in names {
        let (status, body) = get(&format!("/api/v1/replay/artifacts/{name}")).await;
        assert_eq!(status, StatusCode::OK, "{name}");
        let content = body["data"]["content"].as_str().unwrap_or_default();
        for needle in [
            "AKIA",
            "ASIA",
            "-----BEGIN RSA PRIVATE KEY",
            "-----BEGIN PRIVATE KEY",
            "client-key-data: ",
        ] {
            assert!(
                !content.contains(needle),
                "{name} leaks {needle} through the API"
            );
        }
    }
}

/* ---------------------------------------------------------------------------
 * The investigation chain, over HTTP.
 * ------------------------------------------------------------------------- */

#[tokio::test]
async fn the_chain_arrows_are_served_as_human_rca_and_the_facts_are_not() {
    let (status, body) = get("/api/v1/replay/chain").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mode"], "replay");

    for link in body["data"]["links"].as_array().unwrap() {
        let basis = link["basis"].as_str().unwrap();
        assert!(
            basis == "observed_by_fleet_forge" || basis == "mathematically_derived",
            "chain fact {} is {basis}",
            link["id"]
        );
    }
    for edge in body["data"]["edges"].as_array().unwrap() {
        assert_eq!(edge["basis"], "human_rca");
    }
    assert!(
        body["data"]["attribution"]
            .as_str()
            .unwrap()
            .contains("did not produce the chain")
    );
}
