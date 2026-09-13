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
