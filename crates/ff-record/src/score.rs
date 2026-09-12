//! Scoring predictions against what actually happened.
//!
//! The point of this module is to publish the misses. A tool that only reports
//! its hits is marking its own homework, and prediction accuracy is the one
//! claim FleetForge can neither assert nor assume — it has to be measured
//! (ADR-0004).
//!
//! Scoring is deliberately conservative about what it counts as "actual". It
//! reads pod removals and eviction events from the log, and it does not attempt
//! to infer causation: a pod that disappeared during the window is counted,
//! whether or not the maintenance caused it. Where that ambiguity exists, the
//! comparison says so rather than resolving it silently.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::event::{LogEntry, Prediction, RecordedEvent, WorkloadKey};

/// How a single prediction compared against reality.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comparison {
    /// When the prediction was made.
    pub predicted_at: DateTime<Utc>,
    /// The snapshot it was computed against.
    pub snapshot_id: String,
    /// The nodes it was about.
    pub node_names: Vec<String>,
    /// What FleetForge said.
    pub prediction: Prediction,

    /// Pods actually observed leaving the selected nodes.
    pub observed_evictions: u32,
    /// Workloads actually observed losing a pod from the selected nodes.
    pub observed_workloads: Vec<WorkloadKey>,
    /// Kubernetes events with an eviction-related reason during the window.
    pub eviction_events: u32,

    /// Workloads predicted and observed.
    pub correctly_predicted: Vec<WorkloadKey>,
    /// Workloads predicted that were never disrupted.
    pub predicted_but_not_observed: Vec<WorkloadKey>,
    /// Workloads disrupted that were never predicted. **The important column.**
    pub observed_but_not_predicted: Vec<WorkloadKey>,

    /// Whether anything actually happened in the window.
    pub window_had_activity: bool,
}

impl Comparison {
    /// Difference between predicted and observed evictions.
    #[must_use]
    pub const fn eviction_delta(&self) -> i64 {
        self.observed_evictions as i64 - self.prediction.pods_evicted as i64
    }

    /// Whether the workload-level prediction was exactly right.
    #[must_use]
    pub fn workloads_exact(&self) -> bool {
        self.predicted_but_not_observed.is_empty() && self.observed_but_not_predicted.is_empty()
    }

    /// A one-line verdict, including when no verdict is possible.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        if !self.window_had_activity {
            // The honest answer when nothing was drained: a prediction that was
            // never tested is not a prediction that was correct.
            return "untested — no disruption occurred in this window";
        }
        if self.workloads_exact() && self.eviction_delta() == 0 {
            "exact"
        } else if self.observed_but_not_predicted.is_empty() {
            "conservative — predicted more disruption than occurred"
        } else {
            "missed — disruption occurred that was not predicted"
        }
    }
}

/// Aggregate accuracy across a run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accuracy {
    /// Predictions found in the log.
    pub predictions: usize,
    /// Predictions followed by observed disruption.
    pub tested: usize,
    /// Exactly right, at workload and pod-count level.
    pub exact: usize,
    /// Over-predicted, but missed nothing.
    pub conservative: usize,
    /// Missed disruption that occurred. **The number that matters.**
    pub missed: usize,
}

impl Accuracy {
    /// Whether any prediction was actually put to the test.
    #[must_use]
    pub const fn is_measured(&self) -> bool {
        self.tested > 0
    }

    /// A plain-language summary that refuses to imply more than was measured.
    #[must_use]
    pub fn summary(&self) -> String {
        if !self.is_measured() {
            return format!(
                "{} prediction(s) recorded, none tested — no disruption occurred during this run, \
                 so no accuracy can be claimed.",
                self.predictions
            );
        }
        format!(
            "{} of {} tested prediction(s) exact, {} conservative, {} missed disruption that \
             occurred.",
            self.exact, self.tested, self.conservative, self.missed
        )
    }
}

/// Reasons that indicate a pod was actually disrupted.
const EVICTION_REASONS: &[&str] = &[
    "Evicted",
    "Killing",
    "Preempted",
    "NodeNotReady",
    "TaintManagerEviction",
];

/// Score every prediction in a log.
///
/// A prediction's window runs from when it was made to the next prediction, or
/// to the end of the log. That is a simplification worth naming: two preflight
/// runs in quick succession will attribute the disruption to the later one.
#[must_use]
pub fn score(entries: &[LogEntry]) -> (Vec<Comparison>, Accuracy) {
    let prediction_positions: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.prediction().map(|_| i))
        .collect();

    let mut comparisons = Vec::new();

    for (nth, &start) in prediction_positions.iter().enumerate() {
        let Some(entry) = entries.get(start) else {
            continue;
        };
        let Some(prediction) = entry.prediction() else {
            continue;
        };
        let end = prediction_positions
            .get(nth + 1)
            .copied()
            .unwrap_or(entries.len());

        let window = entries.get(start + 1..end).unwrap_or(&[]);
        let selected: BTreeSet<&str> = prediction.node_names.iter().map(String::as_str).collect();

        let mut observed_evictions = 0u32;
        let mut observed_workloads: BTreeSet<WorkloadKey> = BTreeSet::new();
        let mut eviction_events = 0u32;
        let mut any_activity = false;

        for item in window {
            match &item.event {
                RecordedEvent::PodRemoved { node, workload, .. } => {
                    any_activity = true;
                    if node.as_deref().is_some_and(|n| selected.contains(n)) {
                        observed_evictions = observed_evictions.saturating_add(1);
                        if let Some(w) = workload {
                            observed_workloads.insert(w.clone());
                        }
                    }
                }
                RecordedEvent::KubernetesEvent { reason, .. } => {
                    if EVICTION_REASONS.contains(&reason.as_str()) {
                        any_activity = true;
                        eviction_events = eviction_events.saturating_add(1);
                    }
                }
                // A cordon on a selected node is the clearest signal that the
                // maintenance actually began.
                RecordedEvent::NodeChanged {
                    name,
                    unschedulable: true,
                    ..
                } if selected.contains(name.as_str()) => {
                    any_activity = true;
                }
                _ => {}
            }
        }

        let predicted: BTreeSet<WorkloadKey> =
            prediction.affected_workloads.iter().cloned().collect();

        comparisons.push(Comparison {
            predicted_at: entry.recorded_at,
            snapshot_id: prediction.snapshot_id.as_str().to_owned(),
            node_names: prediction.node_names.clone(),
            prediction: prediction.clone(),
            observed_evictions,
            observed_workloads: observed_workloads.iter().cloned().collect(),
            eviction_events,
            correctly_predicted: predicted
                .intersection(&observed_workloads)
                .cloned()
                .collect(),
            predicted_but_not_observed: predicted
                .difference(&observed_workloads)
                .cloned()
                .collect(),
            observed_but_not_predicted: observed_workloads
                .difference(&predicted)
                .cloned()
                .collect(),
            window_had_activity: any_activity,
        });
    }

    let mut accuracy = Accuracy {
        predictions: comparisons.len(),
        ..Accuracy::default()
    };
    for comparison in &comparisons {
        if !comparison.window_had_activity {
            continue;
        }
        accuracy.tested += 1;
        match comparison.verdict() {
            "exact" => accuracy.exact += 1,
            "missed — disruption occurred that was not predicted" => accuracy.missed += 1,
            _ => accuracy.conservative += 1,
        }
    }

    (comparisons, accuracy)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use ff_core::{MaintenanceStatus, Mode, SnapshotId};

    use crate::event::RunId;

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).single().unwrap()
    }

    fn entry(seq: u64, event: RecordedEvent) -> LogEntry {
        LogEntry {
            seq,
            recorded_at: at(1_000 + seq as i64),
            run_id: RunId::new("r"),
            mode: Mode::Live,
            event,
        }
    }

    fn workload(name: &str) -> WorkloadKey {
        WorkloadKey {
            namespace: "demo".into(),
            name: name.into(),
        }
    }

    fn prediction_entry(seq: u64, pods: u32, workloads: &[&str]) -> LogEntry {
        entry(
            seq,
            RecordedEvent::PreflightRun(Box::new(Prediction {
                snapshot_id: SnapshotId::from_hex_unchecked("abc"),
                node_names: vec!["n1".into()],
                concurrency: 1,
                status: MaintenanceStatus::Safe,
                pods_evicted: pods,
                pods_not_rescheduled: 0,
                affected_workloads: workloads.iter().map(|w| workload(w)).collect(),
                findings: vec![],
                authoritative: true,
            })),
        )
    }

    fn removal(seq: u64, node: &str, workload_name: &str) -> LogEntry {
        entry(
            seq,
            RecordedEvent::PodRemoved {
                namespace: "demo".into(),
                name: format!("{workload_name}-x"),
                node: Some(node.into()),
                workload: Some(workload(workload_name)),
            },
        )
    }

    #[test]
    fn an_untested_prediction_is_not_a_correct_one() {
        // The trap this avoids: nothing was drained, nothing was disrupted, so
        // every prediction "matched" and accuracy reads 100%.
        let entries = vec![prediction_entry(1, 2, &["web"])];
        let (comparisons, accuracy) = score(&entries);
        assert_eq!(comparisons.len(), 1);
        assert!(!comparisons[0].window_had_activity);
        assert!(comparisons[0].verdict().starts_with("untested"));
        assert!(!accuracy.is_measured());
        assert!(accuracy.summary().contains("no accuracy can be claimed"));
    }

    #[test]
    fn an_exact_prediction_is_recognised() {
        let entries = vec![
            prediction_entry(1, 2, &["web"]),
            removal(2, "n1", "web"),
            removal(3, "n1", "web"),
        ];
        let (comparisons, accuracy) = score(&entries);
        assert_eq!(comparisons[0].observed_evictions, 2);
        assert_eq!(comparisons[0].eviction_delta(), 0);
        assert_eq!(comparisons[0].verdict(), "exact");
        assert_eq!(accuracy.exact, 1);
        assert_eq!(accuracy.missed, 0);
    }

    #[test]
    fn a_missed_workload_is_reported_as_a_miss() {
        // The column that matters: something was disrupted that FleetForge did
        // not predict.
        let entries = vec![
            prediction_entry(1, 1, &["web"]),
            removal(2, "n1", "web"),
            removal(3, "n1", "database"),
        ];
        let (comparisons, accuracy) = score(&entries);
        assert_eq!(
            comparisons[0].observed_but_not_predicted,
            vec![workload("database")]
        );
        assert!(comparisons[0].verdict().starts_with("missed"));
        assert_eq!(accuracy.missed, 1);
        assert!(accuracy.summary().contains("1 missed"));
    }

    #[test]
    fn over_prediction_is_conservative_not_exact() {
        let entries = vec![
            prediction_entry(1, 5, &["web", "api"]),
            removal(2, "n1", "web"),
        ];
        let (comparisons, accuracy) = score(&entries);
        assert_eq!(
            comparisons[0].predicted_but_not_observed,
            vec![workload("api")]
        );
        assert!(comparisons[0].verdict().starts_with("conservative"));
        assert_eq!(accuracy.conservative, 1);
        assert_eq!(accuracy.exact, 0);
    }

    #[test]
    fn removals_on_other_nodes_are_not_attributed_to_this_maintenance() {
        let entries = vec![
            prediction_entry(1, 1, &["web"]),
            removal(2, "n1", "web"),
            removal(3, "n9", "unrelated"),
        ];
        let (comparisons, _) = score(&entries);
        assert_eq!(comparisons[0].observed_evictions, 1);
        assert!(comparisons[0].observed_but_not_predicted.is_empty());
    }

    #[test]
    fn a_later_prediction_closes_the_earlier_window() {
        let entries = vec![
            prediction_entry(1, 1, &["web"]),
            removal(2, "n1", "web"),
            prediction_entry(3, 1, &["api"]),
            removal(4, "n1", "api"),
        ];
        let (comparisons, _) = score(&entries);
        assert_eq!(comparisons.len(), 2);
        assert_eq!(comparisons[0].observed_evictions, 1);
        assert_eq!(comparisons[1].observed_evictions, 1);
        assert_eq!(comparisons[1].observed_workloads, vec![workload("api")]);
    }
}
