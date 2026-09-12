//! Evidence report generation.
//!
//! Generated in one sequential pass over the JSONL log, offline, with no live
//! connection. Every claim in the report traces to a log line with a sequence
//! number, so a reviewer can check it with `jq` instead of trusting the prose.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::RecordError;
use crate::event::{LogEntry, RecordedEvent, RunId};
use crate::score::{Accuracy, Comparison, score};

/// A complete report for one run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    /// The run.
    pub run_id: RunId,
    /// What the run was for.
    pub description: Option<String>,
    /// The cluster.
    pub cluster_id: Option<String>,
    /// First recorded entry.
    pub started_at: Option<DateTime<Utc>>,
    /// Last recorded entry.
    pub ended_at: Option<DateTime<Utc>>,
    /// Entry count.
    pub entries: usize,
    /// Distinct snapshots observed.
    pub snapshots_observed: usize,
    /// Predictions and how they fared.
    pub comparisons: Vec<Comparison>,
    /// Aggregate accuracy.
    pub accuracy: Accuracy,
    /// Eviction-related events, in order.
    pub timeline: Vec<TimelineItem>,
    /// Anything that makes the report less than complete.
    pub caveats: Vec<String>,
}

/// One notable moment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineItem {
    /// Log sequence number — the anchor a reviewer greps for.
    pub seq: u64,
    /// When it was recorded.
    pub at: DateTime<Utc>,
    /// Short category.
    pub kind: String,
    /// Human-readable line.
    pub detail: String,
}

/// Build a report from a log.
///
/// # Errors
///
/// Returns an error if the log has no entries.
pub fn build(entries: &[LogEntry]) -> Result<Report, RecordError> {
    let Some(first) = entries.first() else {
        return Err(RecordError::EmptyRun {
            run_id: "(unknown)".to_owned(),
        });
    };

    let (comparisons, accuracy) = score(entries);

    let mut description = None;
    let mut cluster_id = None;
    let mut snapshots = std::collections::BTreeSet::new();
    let mut timeline = Vec::new();

    for entry in entries {
        match &entry.event {
            RecordedEvent::RunStarted {
                description: d,
                cluster_id: c,
            } => {
                description = Some(d.clone());
                cluster_id = Some(c.clone());
                timeline.push(TimelineItem {
                    seq: entry.seq,
                    at: entry.recorded_at,
                    kind: "run".into(),
                    detail: format!("run started: {d}"),
                });
            }
            RecordedEvent::SnapshotObserved {
                snapshot_id,
                authoritative,
                ..
            } => {
                snapshots.insert(snapshot_id.as_str().to_owned());
                if !authoritative {
                    timeline.push(TimelineItem {
                        seq: entry.seq,
                        at: entry.recorded_at,
                        kind: "coverage".into(),
                        detail: format!("snapshot {} was not fully collected", snapshot_id.short()),
                    });
                }
            }
            RecordedEvent::PreflightRun(prediction) => timeline.push(TimelineItem {
                seq: entry.seq,
                at: entry.recorded_at,
                kind: "preflight".into(),
                detail: format!(
                    "preflight on [{}] at concurrency {} → {:?} ({} pods predicted evicted)",
                    prediction.node_names.join(", "),
                    prediction.concurrency,
                    prediction.status,
                    prediction.pods_evicted
                ),
            }),
            RecordedEvent::PodRemoved {
                namespace,
                name,
                node,
                ..
            } => timeline.push(TimelineItem {
                seq: entry.seq,
                at: entry.recorded_at,
                kind: "eviction".into(),
                detail: format!(
                    "pod {namespace}/{name} left {}",
                    node.as_deref().unwrap_or("(unknown node)")
                ),
            }),
            RecordedEvent::NodeChanged {
                name,
                ready,
                unschedulable,
                ..
            } => {
                if *unschedulable || !*ready {
                    timeline.push(TimelineItem {
                        seq: entry.seq,
                        at: entry.recorded_at,
                        kind: "node".into(),
                        detail: format!("node {name}: ready={ready} cordoned={unschedulable}"),
                    });
                }
            }
            RecordedEvent::BrupopStateChanged {
                node,
                state,
                target_version,
                ..
            } => timeline.push(TimelineItem {
                seq: entry.seq,
                at: entry.recorded_at,
                kind: "brupop".into(),
                detail: format!(
                    "brupop: {node} → {state}{}",
                    target_version
                        .as_ref()
                        .map(|v| format!(" (target {v})"))
                        .unwrap_or_default()
                ),
            }),
            RecordedEvent::KubernetesEvent {
                kind,
                name,
                event_type,
                reason,
                message,
                ..
            } => {
                if event_type == "Warning" {
                    timeline.push(TimelineItem {
                        seq: entry.seq,
                        at: entry.recorded_at,
                        kind: "warning".into(),
                        detail: format!("{kind}/{name}: {reason} — {message}"),
                    });
                }
            }
            RecordedEvent::RunEnded { reason } => timeline.push(TimelineItem {
                seq: entry.seq,
                at: entry.recorded_at,
                kind: "run".into(),
                detail: format!("run ended: {reason}"),
            }),
            RecordedEvent::PodChanged { .. } => {}
        }
    }

    let mut caveats = Vec::new();
    if !accuracy.is_measured() {
        caveats.push(
            "No disruption occurred during this run, so no prediction was tested. Accuracy is \
             unmeasured, not perfect."
                .to_owned(),
        );
    }
    if entries.iter().any(|e| {
        matches!(
            &e.event,
            RecordedEvent::SnapshotObserved {
                authoritative: false,
                ..
            }
        )
    }) {
        caveats.push(
            "At least one snapshot was incompletely collected, so some analyses did not run and \
             some cluster state is missing from this report."
                .to_owned(),
        );
    }
    if !entries
        .iter()
        .any(|e| matches!(&e.event, RecordedEvent::BrupopStateChanged { .. }))
    {
        caveats.push(
            "No Brupop state was observed. Either Brupop is not installed, or no Bottlerocket \
             update occurred during this run."
                .to_owned(),
        );
    }

    Ok(Report {
        run_id: first.run_id.clone(),
        description,
        cluster_id,
        started_at: Some(first.recorded_at),
        ended_at: entries.last().map(|e| e.recorded_at),
        entries: entries.len(),
        snapshots_observed: snapshots.len(),
        comparisons,
        accuracy,
        timeline,
        caveats,
    })
}

/// Render a report as Markdown.
#[must_use]
pub fn to_markdown(report: &Report) -> String {
    let mut out = String::new();
    let line = |out: &mut String, s: &str| {
        out.push_str(s);
        out.push('\n');
    };

    line(
        &mut out,
        &format!("# FleetForge evidence report — {}", report.run_id),
    );
    line(&mut out, "");
    if let Some(d) = &report.description {
        line(&mut out, &format!("**Run:** {d}  "));
    }
    if let Some(c) = &report.cluster_id {
        line(&mut out, &format!("**Cluster:** `{c}`  "));
    }
    line(
        &mut out,
        &format!(
            "**Window:** {} → {}  ",
            report
                .started_at
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
            report.ended_at.map(|t| t.to_rfc3339()).unwrap_or_default()
        ),
    );
    line(
        &mut out,
        &format!(
            "**Recorded:** {} log entries, {} distinct snapshots",
            report.entries, report.snapshots_observed
        ),
    );
    line(&mut out, "");
    line(
        &mut out,
        "Every line below cites a `seq` from the event log. Check any of them with:",
    );
    line(&mut out, "");
    line(&mut out, "```bash");
    line(&mut out, "jq 'select(.seq == N)' events.jsonl");
    line(&mut out, "```");
    line(&mut out, "");

    line(&mut out, "## Prediction versus actual");
    line(&mut out, "");
    line(&mut out, &report.accuracy.summary());
    line(&mut out, "");

    if report.comparisons.is_empty() {
        line(
            &mut out,
            "_No preflight analysis was recorded in this run._",
        );
    } else {
        line(
            &mut out,
            "| Predicted at | Nodes | Predicted evictions | Observed | Δ | Verdict |",
        );
        line(&mut out, "| --- | --- | --- | --- | --- | --- |");
        for c in &report.comparisons {
            line(
                &mut out,
                &format!(
                    "| {} | {} | {} | {} | {:+} | {} |",
                    c.predicted_at.format("%H:%M:%S"),
                    c.node_names.join(", "),
                    c.prediction.pods_evicted,
                    c.observed_evictions,
                    c.eviction_delta(),
                    c.verdict()
                ),
            );
        }
        line(&mut out, "");

        // The misses get their own section rather than a column nobody reads.
        let misses: Vec<&Comparison> = report
            .comparisons
            .iter()
            .filter(|c| !c.observed_but_not_predicted.is_empty())
            .collect();
        line(&mut out, "### What FleetForge got wrong");
        line(&mut out, "");
        if misses.is_empty() {
            line(
                &mut out,
                "No workload was disrupted without being predicted.",
            );
            if !report.accuracy.is_measured() {
                line(&mut out, "");
                line(
                    &mut out,
                    "**This is not a result.** Nothing was disrupted at all, so nothing was tested.",
                );
            }
        } else {
            for c in misses {
                line(
                    &mut out,
                    &format!(
                        "- Prediction at {}: disrupted but not predicted — {}",
                        c.predicted_at.format("%H:%M:%S"),
                        c.observed_but_not_predicted
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
            }
        }
        line(&mut out, "");

        line(&mut out, "### Over-prediction");
        line(&mut out, "");
        let over: Vec<&Comparison> = report
            .comparisons
            .iter()
            .filter(|c| c.window_had_activity && !c.predicted_but_not_observed.is_empty())
            .collect();
        if over.is_empty() {
            line(&mut out, "None.");
        } else {
            for c in over {
                line(
                    &mut out,
                    &format!(
                        "- Prediction at {}: predicted but not disrupted — {}",
                        c.predicted_at.format("%H:%M:%S"),
                        c.predicted_but_not_observed
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
            }
        }
        line(&mut out, "");
    }

    line(&mut out, "## Timeline");
    line(&mut out, "");
    line(&mut out, "| seq | time | kind | detail |");
    line(&mut out, "| --- | --- | --- | --- |");
    for item in &report.timeline {
        line(
            &mut out,
            &format!(
                "| {} | {} | {} | {} |",
                item.seq,
                item.at.format("%H:%M:%S"),
                item.kind,
                item.detail.replace('|', "\\|")
            ),
        );
    }
    line(&mut out, "");

    line(&mut out, "## Caveats");
    line(&mut out, "");
    if report.caveats.is_empty() {
        line(&mut out, "None recorded.");
    } else {
        for caveat in &report.caveats {
            line(&mut out, &format!("- {caveat}"));
        }
    }
    line(&mut out, "");
    line(
        &mut out,
        "_FleetForge observed this run. It did not perform any of the changes described: it holds \
         no mutating Kubernetes client (ADR-0012)._",
    );

    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use ff_core::Mode;

    fn entry(seq: u64, event: RecordedEvent) -> LogEntry {
        LogEntry {
            seq,
            recorded_at: Utc
                .timestamp_opt(1_757_000_000 + seq as i64, 0)
                .single()
                .unwrap(),
            run_id: RunId::new("run-test"),
            mode: Mode::Live,
            event,
        }
    }

    #[test]
    fn an_empty_log_is_an_error_not_an_empty_report() {
        assert!(build(&[]).is_err());
    }

    #[test]
    fn a_run_with_no_disruption_says_accuracy_is_unmeasured() {
        let entries = vec![entry(
            1,
            RecordedEvent::RunStarted {
                description: "quiet".into(),
                cluster_id: "c1".into(),
            },
        )];
        let report = build(&entries).unwrap();
        let md = to_markdown(&report);
        assert!(md.contains("no prediction was tested") || md.contains("unmeasured"));
        assert!(report.caveats.iter().any(|c| c.contains("not perfect")));
    }

    #[test]
    fn the_report_tells_a_reader_how_to_check_it() {
        let entries = vec![entry(
            1,
            RecordedEvent::RunStarted {
                description: "demo".into(),
                cluster_id: "c1".into(),
            },
        )];
        let md = to_markdown(&build(&entries).unwrap());
        assert!(md.contains("jq 'select(.seq == N)'"));
        assert!(md.contains("| seq |"));
    }

    #[test]
    fn absent_brupop_state_is_a_stated_caveat() {
        let entries = vec![entry(
            1,
            RecordedEvent::RunStarted {
                description: "demo".into(),
                cluster_id: "c1".into(),
            },
        )];
        let report = build(&entries).unwrap();
        assert!(report.caveats.iter().any(|c| c.contains("Brupop")));
    }

    #[test]
    fn the_report_states_that_fleetforge_changed_nothing() {
        let entries = vec![entry(
            1,
            RecordedEvent::RunStarted {
                description: "demo".into(),
                cluster_id: "c1".into(),
            },
        )];
        let md = to_markdown(&build(&entries).unwrap());
        assert!(md.contains("did not perform any of the changes"));
    }
}
