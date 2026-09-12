//! Watch-driven collection.
//!
//! One task per Kubernetes kind, each running a kube-rs watcher into a
//! reflector store. A separate task coalesces change notifications and rebuilds
//! an immutable snapshot.
//!
//! The interesting behaviour is in the failure paths. A watch that dies must
//! not look like a cluster that emptied, and a watch that is forbidden must not
//! look like a resource that does not exist.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use ff_core::{ClusterSnapshot, KindCoverage, Mode};
use futures::StreamExt;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::core::v1::{Event, Namespace, Node, Pod};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use kube::runtime::reflector::{self, Store};
use kube::runtime::{WatchStreamExt, watcher};
use kube::{Api, Client, Resource};
use serde::de::DeserializeOwned;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

use crate::config::CollectorConfig;
use crate::error::CollectError;
use crate::normalize::{self, NormalizeContext, Origin};
use crate::status::{FailureCause, KindTracker};
use crate::store::SnapshotStore;

/// Shared per-kind status, keyed by Kubernetes kind.
type Trackers = Arc<Mutex<HashMap<String, KindTracker>>>;

/// Whether the API server is currently reachable, and when it last was.
///
/// Tracked separately from per-kind watch status on purpose. A watch reports
/// what it last saw; this reports whether we can still talk to the cluster at
/// all. Conflating them means either lying about freshness or throwing away
/// good data on a transient blip.
#[derive(Debug)]
pub struct Connection {
    reachable: AtomicBool,
    last_contact: Mutex<Option<chrono::DateTime<Utc>>>,
}

impl Connection {
    fn new() -> Self {
        Self {
            reachable: AtomicBool::new(true),
            last_contact: Mutex::new(None),
        }
    }

    /// Whether the last liveness probe reached the API server.
    #[must_use]
    pub fn is_reachable(&self) -> bool {
        self.reachable.load(Ordering::Relaxed)
    }

    /// When the API server was last reached.
    pub async fn last_contact(&self) -> Option<chrono::DateTime<Utc>> {
        *self.last_contact.lock().await
    }
}

/// A running collector.
pub struct Collector {
    store: Arc<SnapshotStore>,
    trackers: Trackers,
    connection: Arc<Connection>,
    cluster_id: String,
    server_version: String,
    tasks: Vec<JoinHandle<()>>,
}

impl Collector {
    /// The shared snapshot store.
    #[must_use]
    pub fn store(&self) -> Arc<SnapshotStore> {
        Arc::clone(&self.store)
    }

    /// The stable cluster identifier.
    #[must_use]
    pub fn cluster_id(&self) -> &str {
        &self.cluster_id
    }

    /// The Kubernetes version the API server reports.
    ///
    /// Surfaced so the client/server version skew is visible rather than
    /// assumed — see ADR-0022.
    #[must_use]
    pub fn server_version(&self) -> &str {
        &self.server_version
    }

    /// API server reachability.
    #[must_use]
    pub fn connection(&self) -> Arc<Connection> {
        Arc::clone(&self.connection)
    }

    /// Current per-kind collection status.
    ///
    /// When the API server is unreachable, every otherwise-current kind is
    /// downgraded to stale. The observed counts are **kept**: whatever was last
    /// seen is still the last thing seen, and zeroing them would read as "no
    /// PodDisruptionBudgets", which reads as "no blockers", which reads as safe
    /// to drain.
    pub async fn coverage(&self) -> Vec<KindCoverage> {
        let now = Utc::now();
        let reachable = self.connection.is_reachable();
        let last_contact = self.connection.last_contact().await;
        let trackers = self.trackers.lock().await;
        let mut coverage: Vec<KindCoverage> = trackers
            .values()
            .map(|t| {
                let mut status = t.status_at(now);
                if let (false, ff_core::CollectionStatus::InSync { synced_at }) =
                    (reachable, &status)
                {
                    let anchor = last_contact.unwrap_or(*synced_at);
                    status = ff_core::CollectionStatus::Stale {
                        last_current_at: anchor,
                        age_seconds: now.signed_duration_since(anchor).num_seconds(),
                    };
                }
                KindCoverage {
                    kind: t.kind().to_owned(),
                    status,
                    observed_count: t.observed_count(),
                }
            })
            .collect();
        coverage.sort_by(|a, b| a.kind.cmp(&b.kind));
        coverage
    }

    /// Stop all watch tasks.
    pub fn shutdown(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Extract an HTTP status code from anywhere in an error chain.
///
/// Written against the error *chain* rather than specific `watcher::Error`
/// variants, so a kube-rs upgrade that reshapes those variants does not
/// silently turn a 403 into a generic failure — which would render a forbidden
/// resource as merely degraded, and lose the one thing the operator needs to
/// know to fix it.
fn http_code(err: &(dyn std::error::Error + 'static)) -> Option<u16> {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = current {
        if let Some(kube::Error::Api(response)) = e.downcast_ref::<kube::Error>() {
            return Some(response.code);
        }
        if let Some(status) = e.downcast_ref::<kube::core::Status>() {
            return Some(status.code);
        }
        current = e.source();
    }
    None
}

/// Classify a watch failure into something the interface can act on.
fn classify(err: &watcher::Error, verb: &str, resource: &str) -> FailureCause {
    match http_code(err) {
        Some(401) => FailureCause::Unauthorized,
        Some(403) => FailureCause::Forbidden {
            verb: verb.to_owned(),
            resource: resource.to_owned(),
        },
        Some(code) => FailureCause::WatchError {
            detail: format!("the API server returned HTTP {code} for {verb} {resource}"),
        },
        None => FailureCause::Disconnected {
            detail: format!("the watch on {resource} could not reach the API server"),
        },
    }
}

/// Spawn one watch task for a kind.
fn spawn_watch<K>(
    api: Api<K>,
    kind: &'static str,
    resource_plural: &'static str,
    trackers: Trackers,
    notify: Arc<Notify>,
) -> (Store<K>, JoinHandle<()>)
where
    K: Resource + Clone + std::fmt::Debug + DeserializeOwned + Send + Sync + 'static,
    K::DynamicType: Default + Eq + std::hash::Hash + Clone + std::fmt::Debug + Unpin,
{
    let (reader, writer) = reflector::store::<K>();

    let task = tokio::spawn(async move {
        let stream = watcher(api, watcher::Config::default())
            .reflect(writer)
            .default_backoff();
        futures::pin_mut!(stream);

        let mut seen: usize = 0;
        while let Some(event) = stream.next().await {
            match event {
                Ok(ev) => {
                    // Count what we have actually observed. Note that a Delete
                    // decrements, so a watch restart re-lists and resets — which
                    // is why the count is refreshed from the reflector store at
                    // snapshot time rather than trusted from here.
                    match &ev {
                        watcher::Event::Delete(_) => seen = seen.saturating_sub(1),
                        watcher::Event::Apply(_) | watcher::Event::InitApply(_) => {
                            seen = seen.saturating_add(1);
                        }
                        watcher::Event::Init => seen = 0,
                        watcher::Event::InitDone => {}
                    }
                    let mut guard = trackers.lock().await;
                    if let Some(tracker) = guard.get_mut(kind) {
                        tracker.mark_current(Utc::now(), seen);
                    }
                    drop(guard);
                    notify.notify_one();
                }
                Err(err) => {
                    let cause = classify(&err, "list", resource_plural);
                    tracing::warn!(
                        kind = kind,
                        cause = cause.label(),
                        message = %cause.redacted_message(),
                        "watch failed"
                    );
                    let mut guard = trackers.lock().await;
                    if let Some(tracker) = guard.get_mut(kind) {
                        tracker.mark_failed(cause);
                    }
                    drop(guard);
                    // Notify so the interface learns about the failure
                    // immediately. A forbidden resource that only appears after
                    // the next unrelated change is a forbidden resource the
                    // operator does not see.
                    notify.notify_one();
                }
            }
        }
    });

    (reader, task)
}

/// Every reflector store the snapshot builder reads from.
struct Stores {
    nodes: Store<Node>,
    pods: Store<Pod>,
    deployments: Store<Deployment>,
    stateful_sets: Store<StatefulSet>,
    daemon_sets: Store<DaemonSet>,
    replica_sets: Store<ReplicaSet>,
    pdbs: Store<PodDisruptionBudget>,
    events: Store<Event>,
}

/// Connect and start collecting.
///
/// # Errors
///
/// Returns an error if a client cannot be built or cluster identity cannot be
/// established.
pub async fn start(config: CollectorConfig) -> Result<Collector, CollectError> {
    let client = build_client(&config).await?;
    let (cluster_id, server_version) = identify_cluster(&client, &config).await?;

    let trackers: Trackers = Arc::new(Mutex::new(HashMap::from_iter(
        [
            ("Node", "nodes"),
            ("Pod", "pods"),
            ("Deployment", "deployments"),
            ("StatefulSet", "statefulsets"),
            ("DaemonSet", "daemonsets"),
            ("ReplicaSet", "replicasets"),
            ("PodDisruptionBudget", "poddisruptionbudgets"),
            ("Event", "events"),
        ]
        .into_iter()
        .map(|(kind, plural)| {
            (
                kind.to_owned(),
                KindTracker::new(kind, plural).with_staleness_budget(config.staleness_budget),
            )
        }),
    )));

    let notify = Arc::new(Notify::new());
    let connection = Arc::new(Connection::new());
    let mut tasks = Vec::new();

    tasks.push(spawn_liveness_probe(
        client.clone(),
        Arc::clone(&connection),
        Arc::clone(&trackers),
        config.liveness_interval,
        config.liveness_failures_before_disconnected,
    ));

    macro_rules! watch_all {
        ($ty:ty, $kind:literal, $plural:literal) => {{
            let (store, task) = spawn_watch::<$ty>(
                Api::all(client.clone()),
                $kind,
                $plural,
                Arc::clone(&trackers),
                Arc::clone(&notify),
            );
            tasks.push(task);
            store
        }};
    }

    let stores = Stores {
        nodes: watch_all!(Node, "Node", "nodes"),
        pods: watch_all!(Pod, "Pod", "pods"),
        deployments: watch_all!(Deployment, "Deployment", "deployments"),
        stateful_sets: watch_all!(StatefulSet, "StatefulSet", "statefulsets"),
        daemon_sets: watch_all!(DaemonSet, "DaemonSet", "daemonsets"),
        replica_sets: watch_all!(ReplicaSet, "ReplicaSet", "replicasets"),
        pdbs: watch_all!(
            PodDisruptionBudget,
            "PodDisruptionBudget",
            "poddisruptionbudgets"
        ),
        events: watch_all!(Event, "Event", "events"),
    };

    let store = Arc::new(SnapshotStore::default());
    tasks.push(spawn_builder(
        Arc::clone(&store),
        stores,
        Arc::clone(&trackers),
        Arc::clone(&notify),
        cluster_id.clone(),
        config.debounce,
    ));

    Ok(Collector {
        store,
        trackers,
        connection,
        cluster_id,
        server_version,
        tasks,
    })
}

/// Actively confirm the API server is reachable.
///
/// Without this, a watch-based collector cannot tell a quiet cluster from a
/// dead connection: both produce silence. Measured against a paused API server,
/// nothing else notices for minutes — the TCP connection is frozen rather than
/// closed, so no watch error is raised and the reflector stores keep serving
/// data that is no longer being refreshed.
///
/// A successful probe also refreshes the freshness anchor on every currently
/// healthy kind. That is the honest meaning of the probe: the connection works
/// and the watches are established, so what they last told us still stands.
fn spawn_liveness_probe(
    client: Client,
    connection: Arc<Connection>,
    trackers: Trackers,
    interval: Duration,
    failures_before_disconnected: u32,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut consecutive_failures: u32 = 0;

        loop {
            ticker.tick().await;
            // Bound the probe: a frozen API server accepts the connection and
            // never answers, so without a timeout this task would hang and
            // silently stop probing.
            let probe = tokio::time::timeout(interval, client.apiserver_version()).await;

            match probe {
                Ok(Ok(_)) => {
                    consecutive_failures = 0;
                    let now = Utc::now();
                    if !connection.reachable.swap(true, Ordering::Relaxed) {
                        tracing::info!("API server reachable again");
                    }
                    *connection.last_contact.lock().await = Some(now);
                    let mut guard = trackers.lock().await;
                    for tracker in guard.values_mut() {
                        tracker.confirm_still_current(now);
                    }
                }
                _ => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    if consecutive_failures >= failures_before_disconnected
                        && connection.reachable.swap(false, Ordering::Relaxed)
                    {
                        tracing::warn!(
                            failures = consecutive_failures,
                            "API server unreachable; collected data is no longer current"
                        );
                    }
                }
            }
        }
    })
}

/// Rebuild snapshots when anything changes, coalescing bursts.
fn spawn_builder(
    store: Arc<SnapshotStore>,
    stores: Stores,
    trackers: Trackers,
    notify: Arc<Notify>,
    cluster_id: String,
    debounce: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            notify.notified().await;
            // Coalesce the burst. One `kubectl scale` touches the Deployment,
            // the ReplicaSet, and every new Pod; the operator performed one
            // action and should see one update.
            tokio::time::sleep(debounce).await;

            let now = Utc::now();
            let coverage = {
                let guard = trackers.lock().await;
                let mut c: Vec<KindCoverage> = guard
                    .values()
                    .map(|t| KindCoverage {
                        kind: t.kind().to_owned(),
                        status: t.status_at(now),
                        observed_count: t.observed_count(),
                    })
                    .collect();
                c.sort_by(|a, b| a.kind.cmp(&b.kind));
                c
            };

            let ctx = |kind: &str| NormalizeContext {
                cluster_id: cluster_id.clone(),
                observed_at: now,
                collection: coverage
                    .iter()
                    .find(|c| c.kind == kind)
                    .map_or(ff_core::CollectionStatus::Syncing, |c| c.status.clone()),
                origin: Origin::Live,
            };

            let nodes: Vec<_> = stores
                .nodes
                .state()
                .iter()
                .map(|n| normalize::node::normalize(n, &ctx("Node")))
                .collect();
            let pods: Vec<_> = stores
                .pods
                .state()
                .iter()
                .map(|p| normalize::pod::normalize(p, &ctx("Pod")))
                .collect();
            let mut workloads: Vec<_> = stores
                .deployments
                .state()
                .iter()
                .map(|d| normalize::workload::deployment(d, &ctx("Deployment")))
                .collect();
            workloads.extend(
                stores
                    .stateful_sets
                    .state()
                    .iter()
                    .map(|s| normalize::workload::stateful_set(s, &ctx("StatefulSet"))),
            );
            workloads.extend(
                stores
                    .daemon_sets
                    .state()
                    .iter()
                    .map(|d| normalize::workload::daemon_set(d, &ctx("DaemonSet"))),
            );
            workloads.extend(
                stores
                    .replica_sets
                    .state()
                    .iter()
                    .map(|r| normalize::workload::replica_set(r, &ctx("ReplicaSet"))),
            );
            let pdbs: Vec<_> = stores
                .pdbs
                .state()
                .iter()
                .map(|p| normalize::pdb::normalize(p, &ctx("PodDisruptionBudget")))
                .collect();
            let events: Vec<_> = stores
                .events
                .state()
                .iter()
                .map(|e| normalize::event::normalize(e, &ctx("Event")))
                .collect();

            // Refresh the observed counts from the reflector stores, which are
            // authoritative, rather than from the incremental tally the watch
            // task keeps.
            let coverage = refresh_counts(coverage, &nodes, &pods, &workloads, &pdbs, &events);

            match ClusterSnapshot::new(
                now,
                Mode::Live,
                cluster_id.clone(),
                nodes,
                pods,
                workloads,
                pdbs,
                events,
                coverage,
            ) {
                Ok(snapshot) => {
                    // A watch burst can settle into state identical to what we
                    // already published — a rollout that completes back to the
                    // same shape, or a status field that churns and reverts.
                    // Content-addressed identity makes that detectable, so the
                    // stream stays quiet instead of telling the operator
                    // something happened when nothing did.
                    let unchanged = store
                        .current()
                        .is_some_and(|c| c.snapshot_id() == snapshot.snapshot_id());
                    if unchanged {
                        tracing::trace!("snapshot unchanged; not republishing");
                    } else {
                        store.publish(Arc::new(snapshot));
                    }
                }
                Err(err) => {
                    tracing::error!(error = %err, "failed to build snapshot");
                }
            }
        }
    })
}

fn refresh_counts(
    mut coverage: Vec<KindCoverage>,
    nodes: &[ff_core::NodeFact],
    pods: &[ff_core::PodFact],
    workloads: &[ff_core::WorkloadFact],
    pdbs: &[ff_core::PdbFact],
    events: &[ff_core::EventFact],
) -> Vec<KindCoverage> {
    use ff_core::WorkloadKind;
    let count_of = |kind: WorkloadKind| workloads.iter().filter(|w| w.kind == kind).count();
    for entry in &mut coverage {
        entry.observed_count = match entry.kind.as_str() {
            "Node" => nodes.len(),
            "Pod" => pods.len(),
            "Deployment" => count_of(WorkloadKind::Deployment),
            "StatefulSet" => count_of(WorkloadKind::StatefulSet),
            "DaemonSet" => count_of(WorkloadKind::DaemonSet),
            "ReplicaSet" => count_of(WorkloadKind::ReplicaSet),
            "PodDisruptionBudget" => pdbs.len(),
            "Event" => events.len(),
            _ => entry.observed_count,
        };
    }
    coverage
}

/// Install the TLS crypto provider exactly once.
///
/// kube pulls in rustls but enables no provider feature itself, so the process
/// must choose. Without this, the first TLS handshake panics — and a panic in
/// the connection path is precisely the failure mode `clippy::panic` is denied
/// to prevent, so it is handled here rather than left to chance.
fn install_crypto_provider() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // Fails only if a provider is already installed, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

async fn build_client(config: &CollectorConfig) -> Result<Client, CollectError> {
    use kube::config::{Config, KubeConfigOptions, Kubeconfig};

    install_crypto_provider();

    let kube_config = if let Some(path) = &config.kubeconfig {
        let kubeconfig = Kubeconfig::read_from(path).map_err(|e| CollectError::Connect {
            reason: format!("kubeconfig could not be read: {}", redact(&e.to_string())),
        })?;
        Config::from_custom_kubeconfig(
            kubeconfig,
            &KubeConfigOptions {
                context: config.context.clone(),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| CollectError::Connect {
            reason: redact(&e.to_string()),
        })?
    } else {
        Config::from_kubeconfig(&KubeConfigOptions {
            context: config.context.clone(),
            ..Default::default()
        })
        .await
        .map_err(|e| CollectError::Connect {
            reason: redact(&e.to_string()),
        })?
    };

    Client::try_from(kube_config).map_err(|e| CollectError::Connect {
        reason: redact(&e.to_string()),
    })
}

/// Establish a stable, non-identifying cluster id, and read the server version.
///
/// The `kube-system` namespace UID is the conventional stable cluster
/// identifier. It is opaque, it survives restarts, and — unlike the API server
/// URL — it reveals nothing about where the cluster lives, which is why the
/// URL is never used (`THREAT_MODEL.md` R1).
async fn identify_cluster(
    client: &Client,
    config: &CollectorConfig,
) -> Result<(String, String), CollectError> {
    let version = client
        .apiserver_version()
        .await
        .map(|v| v.git_version)
        .unwrap_or_else(|_| "unknown".to_owned());

    if let Some(id) = &config.cluster_id {
        return Ok((id.clone(), version));
    }

    let namespaces: Api<Namespace> = Api::all(client.clone());
    let kube_system =
        namespaces
            .get("kube-system")
            .await
            .map_err(|e| CollectError::ClusterIdentity {
                reason: redact(&e.to_string()),
            })?;

    let uid = kube_system
        .metadata
        .uid
        .ok_or_else(|| CollectError::ClusterIdentity {
            reason: "the kube-system namespace has no UID".to_owned(),
        })?;

    Ok((uid, version))
}

/// Strip anything that could carry a URL or credential out of an error string.
fn redact(message: &str) -> String {
    message
        .split_whitespace()
        .filter(|token| {
            !token.contains("://")
                && !token.starts_with("Bearer")
                && !token.contains("token")
                && !token.contains(':')
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn api_error(code: u16) -> kube::Error {
        kube::Error::Api(Box::new(kube::core::Status {
            code,
            message: "denied".into(),
            reason: "Denied".into(),
            ..Default::default()
        }))
    }

    #[test]
    fn redaction_removes_urls_and_credentials() {
        let msg = "failed to connect to https://10.0.0.1:6443 with Bearer eyJhbGciOi";
        let clean = redact(msg);
        assert!(!clean.contains("://"), "{clean}");
        assert!(!clean.contains("10.0.0.1"), "{clean}");
        assert!(!clean.contains("Bearer"), "{clean}");
    }

    #[test]
    fn a_403_is_classified_as_forbidden_with_the_resource_named() {
        let err = watcher::Error::InitialListFailed(api_error(403));
        match classify(&err, "list", "poddisruptionbudgets") {
            FailureCause::Forbidden { verb, resource } => {
                assert_eq!(verb, "list");
                assert_eq!(resource, "poddisruptionbudgets");
            }
            other => panic!("expected Forbidden, got {other:?}"),
        }
    }

    #[test]
    fn a_401_is_classified_as_unauthorized_not_as_an_empty_result() {
        let err = watcher::Error::InitialListFailed(api_error(401));
        assert_eq!(
            classify(&err, "list", "nodes"),
            FailureCause::Unauthorized,
            "an expired token must be its own state"
        );
    }
}
