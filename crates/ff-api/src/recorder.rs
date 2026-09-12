//! Turning snapshot updates into recorded events.
//!
//! The collector publishes whole snapshots; the log wants events. This module
//! diffs consecutive snapshots and writes what changed.
//!
//! Diffing rather than tapping the watch stream directly is deliberate. The
//! snapshot is the thing analyses are computed against, so recording its
//! transitions means the log describes the same reality the findings do. A
//! parallel event feed could drift from it, and then the report would be
//! describing a cluster the analysis never saw.

use std::sync::Arc;

use ff_core::{ClusterSnapshot, PodFact, WorkloadFact};
use ff_record::{EventLog, RecordedEvent, WorkloadKey, kubernetes_event, snapshot_event};

use crate::state::AppState;

/// Resolve the workload a pod belongs to, by label selector within its
/// namespace.
///
/// The owner chain stops at the ReplicaSet, and an operator thinks in
/// Deployments, so a Deployment match is preferred when both exist.
fn workload_for<'a>(snapshot: &'a ClusterSnapshot, pod: &PodFact) -> Option<&'a WorkloadFact> {
    let candidates: Vec<&WorkloadFact> = snapshot
        .workloads()
        .iter()
        .filter(|w| w.namespace == pod.namespace && w.selector.matches(&pod.labels))
        .collect();
    candidates
        .iter()
        .find(|w| w.kind == ff_core::WorkloadKind::Deployment)
        .or_else(|| candidates.first())
        .copied()
}

fn workload_key(snapshot: &ClusterSnapshot, pod: &PodFact) -> Option<WorkloadKey> {
    workload_for(snapshot, pod).map(|w| WorkloadKey {
        namespace: w.namespace.clone(),
        name: w.name.clone(),
    })
}

/// Record the difference between two snapshots.
///
/// `previous` is `None` for the first snapshot, where everything is new and
/// nothing has changed yet.
pub fn record_transition(
    log: &EventLog,
    previous: Option<&ClusterSnapshot>,
    current: &ClusterSnapshot,
    authoritative: bool,
) {
    log.record(snapshot_event(current, authoritative));

    let Some(previous) = previous else {
        return;
    };

    // Pods that are gone. This is the signal prediction scoring is built on, so
    // it is derived from the snapshot rather than inferred from events: a pod
    // absent from the current snapshot is absent, whatever events did or did
    // not arrive.
    for pod in previous.pods() {
        let still_present = current
            .pods()
            .iter()
            .any(|p| p.namespace == pod.namespace && p.name == pod.name);
        if !still_present {
            log.record(RecordedEvent::PodRemoved {
                namespace: pod.namespace.clone(),
                name: pod.name.clone(),
                node: pod.node_name.clone(),
                workload: workload_key(previous, pod),
            });
        }
    }

    // Pods whose placement or phase changed.
    for pod in current.pods() {
        let before = previous
            .pods()
            .iter()
            .find(|p| p.namespace == pod.namespace && p.name == pod.name);
        let changed = match before {
            None => true,
            Some(b) => b.phase != pod.phase || b.node_name != pod.node_name,
        };
        if changed {
            log.record(RecordedEvent::PodChanged {
                namespace: pod.namespace.clone(),
                name: pod.name.clone(),
                node: pod.node_name.clone(),
                phase: pod.phase,
                workload: workload_key(current, pod),
            });
        }
    }

    // Node readiness and cordon state. A cordon is usually the first visible
    // sign that maintenance has actually begun.
    for node in current.nodes() {
        let before = previous.nodes().iter().find(|n| n.name == node.name);
        let changed = match before {
            None => true,
            Some(b) => {
                b.is_ready() != node.is_ready()
                    || b.unschedulable != node.unschedulable
                    || b.bottlerocket_version != node.bottlerocket_version
            }
        };
        if changed {
            log.record(RecordedEvent::NodeChanged {
                name: node.name.clone(),
                ready: node.is_ready(),
                unschedulable: node.unschedulable,
                bottlerocket_version: node.bottlerocket_version.clone(),
            });
        }
    }

    // Brupop state transitions.
    for shadow in current.brupop() {
        let before = previous.brupop_for(&shadow.node_name);
        let changed = match before {
            None => true,
            Some(b) => {
                b.current_state != shadow.current_state
                    || b.target_state != shadow.target_state
                    || b.current_version != shadow.current_version
            }
        };
        if changed {
            log.record(RecordedEvent::BrupopStateChanged {
                node: shadow.node_name.clone(),
                state: shadow
                    .current_state
                    .clone()
                    .unwrap_or_else(|| "(unknown)".to_owned()),
                target_version: shadow.target_version.clone(),
                current_version: shadow.current_version.clone(),
            });
        }
    }

    // Kubernetes events not seen before. Matched on object, reason, and first
    // occurrence, because the same event object is updated in place as its
    // count rises.
    for event in current.events() {
        let seen = previous.events().iter().any(|e| {
            e.involved_object.name == event.involved_object.name
                && e.reason == event.reason
                && e.first_seen_at == event.first_seen_at
                && e.count >= event.count
        });
        if !seen {
            log.record(kubernetes_event(event));
        }
    }
}

/// Follow the snapshot stream and record every transition.
pub fn spawn(state: Arc<AppState>, log: Arc<EventLog>) -> Option<tokio::task::JoinHandle<()>> {
    let store = state.store()?;

    Some(tokio::spawn(async move {
        let mut receiver = store.subscribe();
        let mut previous: Option<Arc<ClusterSnapshot>> = state.snapshot();
        if let Some(first) = &previous {
            record_transition(&log, None, first, state.authoritative().await);
        }

        loop {
            match receiver.recv().await {
                Ok(update) => {
                    let authoritative = state.authoritative().await;
                    record_transition(&log, previous.as_deref(), &update.snapshot, authoritative);
                    previous = Some(update.snapshot);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    // Say so in the log rather than leaving an unexplained gap.
                    // A report generated from a log with silent holes is worse
                    // than one that admits them.
                    log.record(RecordedEvent::KubernetesEvent {
                        namespace: None,
                        kind: "FleetForge".into(),
                        name: "recorder".into(),
                        event_type: "Warning".into(),
                        reason: "RecorderLagged".into(),
                        message: format!(
                            "the recorder fell behind and missed {n} snapshot update(s); \
                             this log has a gap"
                        ),
                    });
                }
                Err(_) => break,
            }
        }
    }))
}
