//! HTTP routes.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::Json as ExtractJson;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use ff_core::{ClusterSnapshot, KindCoverage};
use futures::stream::Stream;
use serde::Deserialize;
use serde::Serialize;

use crate::state::{AppState, DataSource};
use crate::{ApiError, Envelope};

/// Build the router.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/api/v1/environment", get(environment))
        .route("/api/v1/snapshot", get(snapshot))
        .route("/api/v1/nodes", get(nodes))
        .route("/api/v1/pods", get(pods))
        .route("/api/v1/workloads", get(workloads))
        .route("/api/v1/pdbs", get(pdbs))
        .route("/api/v1/events", get(events))
        .route("/api/v1/stream", get(stream))
        .route("/api/v1/preflight", axum::routing::post(preflight))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

/// Readiness means the first sync has completed.
///
/// Reporting ready before then would let a load balancer send traffic to a
/// process that would answer "no nodes" — technically true of its own state,
/// and a lie about the cluster.
async fn readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if state.snapshot().is_some() {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "syncing")
    }
}

/// What the operator needs to judge whether to trust anything else.
#[derive(Serialize)]
struct Environment {
    cluster_id: String,
    mode: ff_core::Mode,
    mode_label: &'static str,
    /// Kubernetes version reported by the API server.
    server_version: Option<String>,
    /// The Kubernetes API version the client bindings target.
    ///
    /// Shown next to `server_version` so the skew is visible rather than
    /// assumed (ADR-0022).
    client_target_version: &'static str,
    version: &'static str,
    started_at: chrono::DateTime<chrono::Utc>,
    snapshot_id: Option<String>,
    coverage: Vec<KindCoverage>,
    authoritative: bool,
    /// Whether the API server answered the most recent liveness probe.
    api_server_reachable: bool,
    /// `ok`, `unauthorized`, or `unreachable`.
    connection_state: &'static str,
    /// When the API server was last reached.
    last_contact_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// The Kubernetes API version the compiled-in k8s-openapi bindings target.
const CLIENT_TARGET_VERSION: &str = "v1.36";

async fn environment(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let coverage = state.coverage().await;
    let reachable = state.api_server_reachable();
    let usable = state.connection_usable();
    let authoritative = usable && coverage.iter().all(|c| c.status.is_authoritative());
    let snapshot = state.snapshot();

    Json(Environment {
        api_server_reachable: reachable,
        connection_state: state.connection_state(),
        last_contact_at: state.last_contact().await,
        cluster_id: state.cluster_id(),
        mode: state.mode(),
        mode_label: state.mode().label(),
        server_version: match &state.source {
            DataSource::Live(c) => Some(c.server_version().to_owned()),
            DataSource::Fixture(_) => None,
        },
        client_target_version: CLIENT_TARGET_VERSION,
        version: state.version,
        started_at: state.started_at,
        snapshot_id: snapshot
            .as_ref()
            .map(|s| s.snapshot_id().as_str().to_owned()),
        coverage,
        authoritative,
    })
}

/// Answer a query, or explain why there is no answer yet.
///
/// The `503` path is the important one: before the first sync there is no
/// snapshot, and returning `[]` would be indistinguishable from a cluster with
/// nothing in it.
async fn respond<T, F>(state: &AppState, extract: F) -> axum::response::Response
where
    T: Serialize,
    F: FnOnce(&ClusterSnapshot) -> T,
{
    let Some(snapshot) = state.snapshot() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError {
                code: "syncing",
                message: "the first cluster sync has not completed yet".to_owned(),
                retriable: true,
            }),
        )
            .into_response();
    };

    let authoritative = state.authoritative().await;
    Json(Envelope::new(
        snapshot.mode(),
        snapshot.cluster_id(),
        snapshot.taken_at(),
        authoritative,
        extract(&snapshot),
    ))
    .into_response()
}

async fn snapshot(State(state): State<Arc<AppState>>) -> axum::response::Response {
    respond(&state, |s| s.clone()).await
}

async fn nodes(State(state): State<Arc<AppState>>) -> axum::response::Response {
    respond(&state, |s| s.nodes().to_vec()).await
}

async fn pods(State(state): State<Arc<AppState>>) -> axum::response::Response {
    respond(&state, |s| s.pods().to_vec()).await
}

async fn workloads(State(state): State<Arc<AppState>>) -> axum::response::Response {
    respond(&state, |s| s.workloads().to_vec()).await
}

async fn pdbs(State(state): State<Arc<AppState>>) -> axum::response::Response {
    respond(&state, |s| s.pdbs().to_vec()).await
}

async fn events(State(state): State<Arc<AppState>>) -> axum::response::Response {
    respond(&state, |s| s.events().to_vec()).await
}

/// What the operator is proposing.
///
/// `snapshot_id` is optional. When supplied, it is checked against the current
/// snapshot and a mismatch is rejected rather than silently analyzed against
/// newer state — the operator asked about a cluster they were looking at, and
/// answering about a different one is answering a different question.
#[derive(Debug, Deserialize)]
struct PreflightRequest {
    node_names: Vec<String>,
    #[serde(default = "default_concurrency")]
    desired_concurrency: u32,
    #[serde(default)]
    snapshot_id: Option<String>,
}

const fn default_concurrency() -> u32 {
    1
}

/// Run preflight against the current snapshot.
///
/// A POST because the request body carries the node selection and concurrency.
/// It mutates nothing: it computes over an immutable snapshot and returns.
async fn preflight(
    State(state): State<Arc<AppState>>,
    ExtractJson(body): ExtractJson<PreflightRequest>,
) -> axum::response::Response {
    let Some(snapshot) = state.snapshot() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError {
                code: "syncing",
                message: "the first cluster sync has not completed yet".to_owned(),
                retriable: true,
            }),
        )
            .into_response();
    };

    let stale_request = body
        .snapshot_id
        .as_ref()
        .is_some_and(|requested| requested != snapshot.snapshot_id().as_str());
    if stale_request {
        return (
            StatusCode::CONFLICT,
            Json(ApiError {
                code: "snapshot_changed",
                message: format!(
                    "the cluster changed since that snapshot; current is {}",
                    snapshot.snapshot_id().short()
                ),
                retriable: true,
            }),
        )
            .into_response();
    }

    if body.node_names.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "no_nodes_selected",
                message: "select at least one node to analyze".to_owned(),
                retriable: false,
            }),
        )
            .into_response();
    }

    let request = ff_core::MaintenanceRequest {
        snapshot_id: snapshot.snapshot_id().clone(),
        node_names: body.node_names,
        desired_concurrency: body.desired_concurrency,
    };

    let result = ff_preflight::run(&snapshot, &request);

    // A preflight result is a calculation. The envelope says WHAT-IF because
    // the result is WHAT-IF, regardless of how live its inputs were.
    Json(Envelope::new(
        result.mode(),
        snapshot.cluster_id(),
        result.provenance.observed_at(),
        state.authoritative().await,
        result,
    ))
    .into_response()
}

/// The live stream.
///
/// Sends `snapshot.updated` when the cluster changes and a periodic
/// `heartbeat`. The heartbeat is not cosmetic: it is how the browser
/// distinguishes "nothing is happening in the cluster" from "this stream is
/// dead", which are the two things a quiet dashboard could mean.
async fn stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = async_stream::stream! {
        // Fixture mode has no watches; it emits one snapshot and then
        // heartbeats. It must still be labelled FIXTURE on every message.
        let store = state.store();

        if let Some(current) = state.snapshot() {
            yield Ok(sse_snapshot(&current, state.authoritative().await));
        }

        let mut rx = store.as_ref().map(|s| s.subscribe());
        let mut ticker = tokio::time::interval(Duration::from_secs(10));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            match rx.as_mut() {
                Some(receiver) => {
                    tokio::select! {
                        received = receiver.recv() => match received {
                            Ok(update) => {
                                let authoritative = state.authoritative().await;
                                yield Ok(sse_snapshot(&update.snapshot, authoritative));
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                // The browser fell behind. Tell it rather than
                                // silently skipping: it should re-read the
                                // current snapshot, not assume continuity.
                                yield Ok(SseEvent::default()
                                    .event("stream.lagged")
                                    .data(n.to_string()));
                            }
                            Err(_) => break,
                        },
                        _ = ticker.tick() => {
                            yield Ok(heartbeat(&state).await);
                        }
                    }
                }
                None => {
                    ticker.tick().await;
                    yield Ok(heartbeat(&state).await);
                }
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn sse_snapshot(snapshot: &ClusterSnapshot, authoritative: bool) -> SseEvent {
    let payload = Envelope::new(
        snapshot.mode(),
        snapshot.cluster_id(),
        snapshot.taken_at(),
        authoritative,
        snapshot,
    );
    SseEvent::default()
        .event("snapshot.updated")
        .json_data(payload)
        .unwrap_or_else(|_| {
            SseEvent::default()
                .event("error")
                .data("snapshot could not be serialised")
        })
}

async fn heartbeat(state: &AppState) -> SseEvent {
    #[derive(Serialize)]
    struct Heartbeat {
        at: chrono::DateTime<chrono::Utc>,
        mode_label: &'static str,
        authoritative: bool,
        api_server_reachable: bool,
        coverage: Vec<KindCoverage>,
    }
    let coverage = state.coverage().await;
    let reachable = state.api_server_reachable();
    let authoritative =
        state.connection_usable() && coverage.iter().all(|c| c.status.is_authoritative());
    SseEvent::default()
        .event("heartbeat")
        .json_data(Heartbeat {
            at: Utc::now(),
            mode_label: state.mode().label(),
            authoritative,
            api_server_reachable: reachable,
            coverage,
        })
        .unwrap_or_else(|_| SseEvent::default().event("heartbeat").data("{}"))
}
