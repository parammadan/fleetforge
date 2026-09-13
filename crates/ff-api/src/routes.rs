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
        .route("/api/v1/brupop", get(brupop))
        .route("/api/v1/report", get(report))
        .route("/api/v1/replay/context", get(replay_context))
        .route("/api/v1/replay/timeline", get(replay_timeline))
        .route("/api/v1/replay/chapters", get(replay_chapters))
        .route("/api/v1/replay/state", get(replay_state))
        .route("/api/v1/replay/claims", get(replay_claims))
        .route("/api/v1/replay/chain", get(replay_chain))
        .route("/api/v1/replay/finding", get(replay_finding))
        .route("/api/v1/replay/predictions", get(replay_predictions))
        .route("/api/v1/replay/traffic", get(replay_traffic))
        .route("/api/v1/replay/artifacts", get(replay_artifacts))
        .route("/api/v1/replay/artifacts/{name}", get(replay_artifact))
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
    // Replay is ready as soon as the bundle has loaded; there is nothing to
    // wait for.
    if state.snapshot().is_some() || state.replay().is_some() {
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
            DataSource::Replay(b) => b.context.kubernetes_version.clone(),
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
        // Replay has no single current snapshot, and saying "syncing" here would
        // imply a live cluster this process is not connected to.
        let error = if state.replay().is_some() {
            ApiError {
                code: "replay_has_no_current_snapshot",
                message: "this process is replaying a capture; cluster state \
                          depends on timeline position — use /api/v1/replay/state"
                    .to_owned(),
                retriable: false,
            }
        } else {
            ApiError {
                code: "syncing",
                message: "the first cluster sync has not completed yet".to_owned(),
                retriable: true,
            }
        };
        return (StatusCode::SERVICE_UNAVAILABLE, Json(error)).into_response();
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

/// Brupop's view of the fleet.
///
/// An empty list here is meaningful only if the `BottlerocketShadow` coverage
/// says `in sync`. In a cluster without Brupop it says `not installed`, which
/// is a different thing entirely and is reported as such.
async fn brupop(State(state): State<Arc<AppState>>) -> axum::response::Response {
    respond(&state, |s| s.brupop().to_vec()).await
}

/// The evidence report for this process's recording.
///
/// Generated from the JSONL log in one sequential pass, with no live
/// connection. Pass `?format=markdown` for the rendered version.
async fn report(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ReportQuery>,
) -> axum::response::Response {
    let Some(log) = state.log.as_ref() else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "not_recording",
                message: "this process was started without --record, so there is no log".to_owned(),
                retriable: false,
            }),
        )
            .into_response();
    };

    let entries = match ff_record::read(log.path()) {
        Ok(entries) => entries,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    code: "log_unreadable",
                    message: err.to_string(),
                    retriable: true,
                }),
            )
                .into_response();
        }
    };

    match ff_record::build(&entries) {
        Ok(report) => {
            if params.format.as_deref() == Some("markdown") {
                (
                    StatusCode::OK,
                    [("content-type", "text/markdown; charset=utf-8")],
                    ff_record::to_markdown(&report),
                )
                    .into_response()
            } else {
                Json(report).into_response()
            }
        }
        Err(err) => (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "empty_run",
                message: err.to_string(),
                retriable: true,
            }),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize, Default)]
struct ReportQuery {
    format: Option<String>,
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
        // Replay has no single current snapshot, and saying "syncing" here would
        // imply a live cluster this process is not connected to.
        let error = if state.replay().is_some() {
            ApiError {
                code: "replay_has_no_current_snapshot",
                message: "this process is replaying a capture; cluster state \
                          depends on timeline position — use /api/v1/replay/state"
                    .to_owned(),
                retriable: false,
            }
        } else {
            ApiError {
                code: "syncing",
                message: "the first cluster sync has not completed yet".to_owned(),
                retriable: true,
            }
        };
        return (StatusCode::SERVICE_UNAVAILABLE, Json(error)).into_response();
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

    // Record the prediction so it can be scored against what actually happens.
    // This is the only reason prediction-versus-actual is possible at all: an
    // unrecorded prediction can never be checked.
    if let Some(log) = state.log.as_ref() {
        let authoritative = state.authoritative().await;
        log.record(ff_record::RecordedEvent::PreflightRun(Box::new(
            ff_record::prediction(&result, authoritative),
        )));
    }

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

/// Replay endpoints.
///
/// Every one wraps its payload in an [`Envelope`] carrying `Mode::Replay`.
/// There is no code path by which replay data leaves this process labelled
/// anything else, and a test asserts it.
fn replay_envelope<T: Serialize>(state: &AppState, data: T) -> axum::response::Response {
    let Some(bundle) = state.replay() else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "not_replaying",
                message: "this process was not started with --replay".to_owned(),
                retriable: false,
            }),
        )
            .into_response();
    };
    Json(Envelope::new(
        ff_core::Mode::Replay,
        bundle.context.cluster_id.clone(),
        bundle.context.captured_to,
        true,
        data,
    ))
    .into_response()
}

#[derive(Serialize)]
struct ReplayContext<'a> {
    schema_version: u32,
    context: &'a ff_replay::CaptureContext,
    events: usize,
    significant_events: usize,
    caveats: &'a [ff_replay::DataCaveat],
}

async fn replay_context(State(state): State<Arc<AppState>>) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    let payload = ReplayContext {
        schema_version: b.schema_version,
        context: &b.context,
        events: b.timeline.len(),
        significant_events: b.timeline.significant().len(),
        caveats: &b.caveats,
    };
    replay_envelope(&state, payload)
}

#[derive(Debug, Deserialize, Default)]
struct TimelineQuery {
    /// Only the events worth stopping on. Default true: the full log is 5,068
    /// entries and a scrubber over all of them is unusable.
    #[serde(default)]
    all: bool,
}

async fn replay_timeline(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<TimelineQuery>,
) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    if q.all {
        replay_envelope(&state, b.timeline.events())
    } else {
        replay_envelope(&state, b.timeline.significant())
    }
}

#[derive(Debug, Deserialize, Default)]
struct PositionQuery {
    position: Option<usize>,
}

async fn replay_state(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<PositionQuery>,
) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    // Clamped inside state_at, so an out-of-range scrub cannot error.
    let s = b.timeline.state_at(q.position.unwrap_or(0));
    replay_envelope(&state, s)
}

async fn replay_chapters(State(state): State<Arc<AppState>>) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    replay_envelope(&state, &b.chapters)
}

async fn replay_chain(State(state): State<Arc<AppState>>) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    replay_envelope(&state, &b.chain)
}

async fn replay_claims(State(state): State<Arc<AppState>>) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    replay_envelope(&state, &b.claims)
}

async fn replay_finding(State(state): State<Arc<AppState>>) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    replay_envelope(&state, &b.pdb)
}

async fn replay_predictions(State(state): State<Arc<AppState>>) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    replay_envelope(&state, &b.predictions)
}

async fn replay_traffic(State(state): State<Arc<AppState>>) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    replay_envelope(&state, &b.traffic)
}

async fn replay_artifacts(State(state): State<Arc<AppState>>) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    replay_envelope(&state, b.artifacts.list())
}

/// Read one artifact by name.
///
/// The name is a manifest key, never a path fragment. Anything not in the
/// manifest returns 404 — including every traversal attempt, which is
/// deliberately indistinguishable from a missing file so the endpoint cannot be
/// used to probe the filesystem.
async fn replay_artifact(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> axum::response::Response {
    let Some(b) = state.replay() else {
        return replay_envelope(&state, ());
    };
    const LIMIT: usize = 256 * 1024;
    match b.artifacts.read(&name, LIMIT) {
        Ok(content) => {
            let meta = b.artifacts.get(&name);
            replay_envelope(
                &state,
                serde_json::json!({
                    "name": name,
                    "sha256": meta.map(|m| m.sha256.clone()),
                    "kind": meta.map(|m| m.kind),
                    "bytes": meta.map(|m| m.bytes),
                    "content": content,
                }),
            )
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "no_such_artifact",
                message: "no such artifact".to_owned(),
                retriable: false,
            }),
        )
            .into_response(),
    }
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
