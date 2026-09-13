//! Deterministic replay state.
//!
//! [`ReplayTimeline::state_at`] is a pure fold over the captured events up to a
//! position. No clock, no randomness, no interpolation: the same bundle and the
//! same position always produce byte-identical state.
//!
//! That matters more than it sounds. A leadership demonstration is watched
//! twice — once in rehearsal and once for real — and a timeline that drifts
//! between the two is a timeline nobody can point at.
//!
//! **Nothing here invents events.** If the cluster produced no observation
//! between 14:36 and 14:53, the replay shows no change for seventeen minutes.
//! Smoothing that would be fabricating Kubernetes activity.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One captured event, normalised for replay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayEvent {
    /// Position in the timeline, 0-based and dense.
    pub index: usize,
    /// The sequence number from the original log. Gaps are real and preserved.
    pub seq: u64,
    /// When FleetForge recorded it.
    pub at: DateTime<Utc>,
    /// Event discriminator from the log.
    pub kind: String,
    /// A one-line human summary.
    pub summary: String,
    /// Whether this is a moment worth stopping on. The complete log is 5,068
    /// entries, the overwhelming majority routine Kubernetes chatter; a
    /// timeline that treats all of them equally is unreadable.
    pub significant: bool,
    /// The raw log line, for the evidence drawer.
    pub raw: serde_json::Value,
}

/// A node's state at a point in the replay.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeReplayState {
    /// Node name.
    pub name: String,
    /// Whether it reported Ready.
    pub ready: bool,
    /// Whether it was cordoned.
    pub unschedulable: bool,
    /// Bottlerocket release as recorded.
    ///
    /// Early events carry `2.0.0` — the updater-interface-version label, read
    /// into the wrong field by a bug fixed partway through the capture. Shown
    /// as recorded and flagged by a data caveat rather than silently corrected,
    /// because rewriting captured evidence to look better is the one thing this
    /// project must never do.
    pub bottlerocket_version: Option<String>,
    /// Brupop's reported state for this node.
    pub brupop_state: Option<String>,
    /// Brupop's reported version for this node.
    pub brupop_version: Option<String>,
    /// Pods currently assigned here.
    pub pods: Vec<String>,
    /// When this node last changed.
    pub last_change: Option<DateTime<Utc>>,
}

/// A pod's state at a point in the replay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodReplayState {
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
    /// Assigned node, if any.
    pub node: Option<String>,
    /// Phase as last reported.
    pub phase: String,
    /// Owning workload, where resolvable.
    pub workload: Option<String>,
}

/// The most recent preflight result at a point in the replay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightReplayState {
    /// When it ran.
    pub at: DateTime<Utc>,
    /// Nodes analyzed.
    pub node_names: Vec<String>,
    /// Safe or blocked.
    pub status: String,
    /// Predicted evictions.
    pub pods_evicted: u32,
    /// Finding ids returned.
    pub findings: Vec<String>,
    /// Snapshot analyzed.
    pub snapshot_id: String,
}

/// Everything the interface needs at one timeline position.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayState {
    /// Position this state was computed for.
    pub position: usize,
    /// Timestamp of the event at that position.
    pub at: DateTime<Utc>,
    /// Nodes, in stable name order.
    pub nodes: Vec<NodeReplayState>,
    /// Pods currently known to exist, in stable order.
    pub pods: Vec<PodReplayState>,
    /// Most recent preflight, if one has run by now.
    pub last_preflight: Option<PreflightReplayState>,
    /// Most recent snapshot id observed by now.
    pub snapshot_id: Option<String>,
    /// How many events have been applied.
    pub events_applied: usize,
    /// Pods in Pending at this position — the visible symptom of the deadlock.
    pub pending_pods: usize,
    /// Cordoned nodes at this position.
    pub cordoned_nodes: usize,
}

/// The ordered, immutable timeline.
#[derive(Clone, Debug)]
pub struct ReplayTimeline {
    events: Vec<ReplayEvent>,
}

impl ReplayTimeline {
    /// Build from events already sorted by sequence number.
    #[must_use]
    pub fn new(events: Vec<ReplayEvent>) -> Self {
        Self { events }
    }

    /// Every event.
    #[must_use]
    pub fn events(&self) -> &[ReplayEvent] {
        &self.events
    }

    /// Just the events worth stopping on.
    #[must_use]
    pub fn significant(&self) -> Vec<&ReplayEvent> {
        self.events.iter().filter(|e| e.significant).collect()
    }

    /// Number of events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the timeline is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// State after applying events `0..=position`.
    ///
    /// Positions beyond the end clamp to the end, so a scrubber cannot produce
    /// an error by overshooting.
    #[must_use]
    pub fn state_at(&self, position: usize) -> ReplayState {
        let last = self.events.len().saturating_sub(1);
        let position = position.min(last);

        let mut nodes: BTreeMap<String, NodeReplayState> = BTreeMap::new();
        let mut pods: BTreeMap<(String, String), PodReplayState> = BTreeMap::new();
        let mut last_preflight = None;
        let mut snapshot_id = None;
        let mut applied = 0usize;

        for event in self.events.iter().take(position + 1) {
            applied += 1;
            match event.kind.as_str() {
                "node_changed" => {
                    let r = &event.raw;
                    let Some(name) = r.get("name").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let entry = nodes
                        .entry(name.to_owned())
                        .or_insert_with(|| NodeReplayState {
                            name: name.to_owned(),
                            ..Default::default()
                        });
                    entry.ready = r
                        .get("ready")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(entry.ready);
                    entry.unschedulable = r
                        .get("unschedulable")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(entry.unschedulable);
                    if let Some(v) = r.get("bottlerocket_version").and_then(|v| v.as_str()) {
                        entry.bottlerocket_version = Some(v.to_owned());
                    }
                    entry.last_change = Some(event.at);
                }
                "brupop_state_changed" => {
                    let r = &event.raw;
                    let Some(shadow) = r.get("node").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    // Brupop names each shadow `brs-<node>`.
                    let node = shadow.strip_prefix("brs-").unwrap_or(shadow).to_owned();
                    let entry = nodes
                        .entry(node.clone())
                        .or_insert_with(|| NodeReplayState {
                            name: node,
                            ..Default::default()
                        });
                    if let Some(s) = r.get("state").and_then(|v| v.as_str()) {
                        entry.brupop_state = Some(s.to_owned());
                    }
                    if let Some(v) = r.get("current_version").and_then(|v| v.as_str()) {
                        entry.brupop_version = Some(v.to_owned());
                    }
                }
                "pod_changed" => {
                    let r = &event.raw;
                    let (Some(ns), Some(name)) = (
                        r.get("namespace").and_then(|v| v.as_str()),
                        r.get("name").and_then(|v| v.as_str()),
                    ) else {
                        continue;
                    };
                    pods.insert(
                        (ns.to_owned(), name.to_owned()),
                        PodReplayState {
                            namespace: ns.to_owned(),
                            name: name.to_owned(),
                            node: r
                                .get("node")
                                .and_then(|v| v.as_str())
                                .map(ToOwned::to_owned),
                            phase: r
                                .get("phase")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_owned(),
                            workload: r
                                .get("workload")
                                .and_then(|w| w.get("name"))
                                .and_then(|v| v.as_str())
                                .map(ToOwned::to_owned),
                        },
                    );
                }
                "pod_removed" => {
                    let r = &event.raw;
                    if let (Some(ns), Some(name)) = (
                        r.get("namespace").and_then(|v| v.as_str()),
                        r.get("name").and_then(|v| v.as_str()),
                    ) {
                        pods.remove(&(ns.to_owned(), name.to_owned()));
                    }
                }
                "preflight_run" => {
                    let r = &event.raw;
                    last_preflight = Some(PreflightReplayState {
                        at: event.at,
                        node_names: r
                            .get("node_names")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(ToOwned::to_owned))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        status: r
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_owned(),
                        pods_evicted: r
                            .get("pods_evicted")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0) as u32,
                        findings: r
                            .get("findings")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(ToOwned::to_owned))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        snapshot_id: r
                            .get("snapshot_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_owned(),
                    });
                }
                "snapshot_observed" => {
                    if let Some(id) = event.raw.get("snapshot_id").and_then(|v| v.as_str()) {
                        snapshot_id = Some(id.to_owned());
                    }
                }
                _ => {}
            }
        }

        // Attach pods to their nodes, in stable order.
        for pod in pods.values() {
            if let Some(node) = pod.node.as_ref().and_then(|n| nodes.get_mut(n)) {
                node.pods.push(format!("{}/{}", pod.namespace, pod.name));
            }
        }
        for node in nodes.values_mut() {
            node.pods.sort();
        }

        let pending = pods.values().filter(|p| p.phase == "Pending").count();
        let cordoned = nodes.values().filter(|n| n.unschedulable).count();

        ReplayState {
            position,
            at: self.events.get(position).map_or_else(Utc::now, |e| e.at),
            nodes: nodes.into_values().collect(),
            pods: pods.into_values().collect(),
            last_preflight,
            snapshot_id,
            events_applied: applied,
            pending_pods: pending,
            cordoned_nodes: cordoned,
        }
    }
}
