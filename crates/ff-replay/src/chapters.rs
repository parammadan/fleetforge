//! Chapters: the handful of timeline positions worth jumping to.
//!
//! A 5,068-event log with a 63-minute span is not something anybody scrubs
//! through in a meeting. Chapters are bookmarks — and they are *derived*, not
//! authored. Each one is the position at which a specific, checkable condition
//! first became true in the captured log, so a chapter cannot point at a moment
//! the evidence does not contain.
//!
//! The narration attached to each chapter is editorial and says so, by carrying
//! the same [`ClaimBasis`] labels the claims do. "Two nodes are already
//! cordoned" is `OBSERVED`. "Cordons held the third replica unschedulable" is
//! `HUMAN RCA`, because a person worked that out afterwards — FleetForge did
//! not infer it.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::schema::ClaimBasis;
use crate::state::ReplayTimeline;

/// A position worth jumping to, with what to look at when you get there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chapter {
    /// Stable identifier, used by the interface and by tests.
    pub id: String,
    /// Short title for the chapter rail.
    pub title: String,
    /// Timeline position. Always a real event index.
    pub position: usize,
    /// The timestamp of that event.
    pub at: DateTime<Utc>,
    /// What this moment means, in one or two sentences.
    pub narration: String,
    /// How the narration is justified.
    pub basis: ClaimBasis,
}

/// What the scan tracks. Deliberately much less than [`crate::ReplayState`]:
/// a chapter boundary only needs cordons, versions and pending pods.
#[derive(Default)]
struct Tracker {
    cordoned: BTreeMap<String, bool>,
    version: BTreeMap<String, String>,
    pod_phase: BTreeMap<(String, String), String>,
}

impl Tracker {
    fn pending(&self) -> usize {
        self.pod_phase.values().filter(|p| *p == "Pending").count()
    }

    fn cordoned_count(&self) -> usize {
        self.cordoned.values().filter(|c| **c).count()
    }
}

/// The release every node ended the capture on.
const FINAL_RELEASE: &str = "1.64.0";

/// Derive chapters from a timeline.
///
/// Every chapter that cannot be justified from the log is simply absent. An
/// interface showing four chapters instead of eight is telling the truth about
/// a thinner capture; an interface showing eight chapters where four are
/// invented is not.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn derive(
    timeline: &ReplayTimeline,
    brupop_first_seen_at: Option<DateTime<Utc>>,
    kind: crate::schema::CaptureKind,
) -> Vec<Chapter> {
    if kind == crate::schema::CaptureKind::Prevented {
        return derive_prevented(timeline);
    }
    let events = timeline.events();
    if events.is_empty() {
        return Vec::new();
    }

    let mut chapters: Vec<Chapter> = Vec::new();
    let mut t = Tracker::default();
    let mut uncordons = 0_u32;
    let mut seen_blocked_preflight = false;
    let mut seen_cordon = false;
    let mut seen_deadlock = false;
    let mut seen_all_updated = false;

    // Where FleetForge's first complete snapshot ends.
    //
    // A snapshot arrives as a burst: the `snapshot_observed` marker followed by
    // one event per resource in it. Everything inside that first burst is state
    // that already existed when FleetForge connected — it was read, not
    // watched. The burst ends at the next `snapshot_observed`.
    //
    // The very first marker can carry zero resources (the reflector had not
    // populated yet), so the search starts at the first one that actually has
    // nodes in it.
    let initial_burst_end = {
        let has_nodes = |e: &crate::state::ReplayEvent| {
            e.kind == "snapshot_observed"
                && e.raw
                    .get("nodes")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    > 0
        };
        events.iter().position(has_nodes).map_or(0, |start| {
            events
                .iter()
                .skip(start + 1)
                .position(|e| e.kind == "snapshot_observed")
                .map_or(events.len() - 1, |offset| start + offset)
        })
    };

    let push = |chapters: &mut Vec<Chapter>,
                id: &str,
                title: &str,
                position: usize,
                at: DateTime<Utc>,
                narration: &str,
                basis: ClaimBasis| {
        chapters.push(Chapter {
            id: id.to_owned(),
            title: title.to_owned(),
            position,
            at,
            narration: narration.to_owned(),
            basis,
        });
    };

    // Composed from the evidence, not written down. If a future capture starts
    // before Brupop does, this sentence inverts instead of staying wrong.
    let opening = match brupop_first_seen_at {
        Some(brupop) if brupop < events[0].at => {
            let gap = events[0].at.signed_duration_since(brupop);
            format!(
                "FleetForge starts recording at {}. Brupop had already begun updating this \
                 fleet at {} — {} minutes {} seconds earlier — so the first frame of this \
                 replay is not the beginning of the incident. Everything before this point \
                 happened outside the evidence and cannot be replayed.",
                events[0].at.format("%H:%M:%S"),
                brupop.format("%H:%M:%S"),
                gap.num_minutes(),
                gap.num_seconds() % 60,
            )
        }
        Some(brupop) => format!(
            "FleetForge starts recording at {}. The earliest Brupop activity in the bundle \
             is {}, which is not earlier, so this capture does cover the start of the \
             update.",
            events[0].at.format("%H:%M:%S"),
            brupop.format("%H:%M:%S"),
        ),
        // No Brupop events in the snapshot. Say so; do not guess a time.
        None => format!(
            "FleetForge starts recording at {}. When Brupop began updating this fleet is \
             not recoverable from this bundle, so whether the capture covers the start of \
             the incident is UNKNOWN.",
            events[0].at.format("%H:%M:%S"),
        ),
    };
    push(
        &mut chapters,
        "capture-begins",
        "Capture begins",
        0,
        events[0].at,
        &opening,
        ClaimBasis::ObservedByFleetForge,
    );

    for (i, event) in events.iter().enumerate() {
        let r = &event.raw;
        match event.kind.as_str() {
            "node_changed" => {
                let Some(name) = r.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                if let Some(v) = r.get("bottlerocket_version").and_then(|v| v.as_str()) {
                    t.version.insert(name.to_owned(), v.to_owned());
                }
                let was = t.cordoned.get(name).copied();
                let now = r
                    .get("unschedulable")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(was.unwrap_or(false));
                t.cordoned.insert(name.to_owned(), now);

                if now && !seen_cordon {
                    seen_cordon = true;
                    push(
                        &mut chapters,
                        "already-cordoned",
                        "Nodes are already cordoned",
                        i,
                        event.at,
                        "The first node states FleetForge ever saw include cordons that were \
                         already in place. FleetForge did not watch these nodes get cordoned \
                         and therefore did not, and could not, predict what followed.",
                        ClaimBasis::ObservedByFleetForge,
                    );
                }

                // A real uncordon: previously known to be cordoned, now not.
                if was == Some(true) && !now {
                    uncordons += 1;
                    if uncordons <= 2 {
                        let (id, title, narration) = if uncordons == 1 {
                            (
                                "first-uncordon",
                                "First manual uncordon",
                                "A human uncordons the first node. FleetForge did not perform \
                                 this — it has no execution path and never issued a mutation. \
                                 Watch the pending pod count from here.",
                            )
                        } else {
                            (
                                "second-uncordon",
                                "Second manual uncordon",
                                "The second node is uncordoned and Brupop resumes its update \
                                 of the remaining node.",
                            )
                        };
                        push(
                            &mut chapters,
                            id,
                            title,
                            i,
                            event.at,
                            narration,
                            ClaimBasis::ObservedByFleetForge,
                        );
                    }
                }

                if !seen_all_updated
                    && t.version.len() >= 3
                    && t.version.values().all(|v| v == FINAL_RELEASE)
                {
                    seen_all_updated = true;
                    push(
                        &mut chapters,
                        "all-updated",
                        "All nodes on 1.64.0",
                        i,
                        event.at,
                        "Every node in the fleet now reports Bottlerocket 1.64.0. The update \
                         Brupop set out to perform completed.",
                        ClaimBasis::ObservedByFleetForge,
                    );
                }
            }
            "pod_changed" => {
                let (Some(ns), Some(name)) = (
                    r.get("namespace").and_then(|v| v.as_str()),
                    r.get("name").and_then(|v| v.as_str()),
                ) else {
                    continue;
                };
                t.pod_phase.insert(
                    (ns.to_owned(), name.to_owned()),
                    r.get("phase")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_owned(),
                );
            }
            "pod_removed" => {
                if let (Some(ns), Some(name)) = (
                    r.get("namespace").and_then(|v| v.as_str()),
                    r.get("name").and_then(|v| v.as_str()),
                ) {
                    t.pod_phase.remove(&(ns.to_owned(), name.to_owned()));
                }
            }
            "preflight_run" => {
                let blocked = r.get("status").and_then(|v| v.as_str()) == Some("blocked");
                if blocked && !seen_blocked_preflight {
                    seen_blocked_preflight = true;
                    push(
                        &mut chapters,
                        "blocker-detected",
                        "FleetForge reports BLOCKED",
                        i,
                        event.at,
                        "The first preflight returns BLOCKED on FF-PDB-001: the \
                         PodDisruptionBudget permits zero disruptions. This is a detection of \
                         a condition that already existed, not a prediction of the deadlock \
                         that followed.",
                        ClaimBasis::ObservedByFleetForge,
                    );
                }
            }
            _ => {}
        }

        if !seen_deadlock && t.cordoned_count() >= 2 && t.pending() >= 1 {
            seen_deadlock = true;
            // Reached inside the first snapshot means the condition predates the
            // recording. Saying "becomes visible" there would imply FleetForge
            // watched it develop, which it did not.
            let developed = i > initial_burst_end;
            let (title, narration) = if developed {
                (
                    "The loop becomes visible",
                    "Two nodes are cordoned and at least one pod is stuck Pending.",
                )
            } else {
                (
                    "The loop is already in place",
                    "Two nodes cordoned and a pod stuck Pending — in the first state \
                     FleetForge ever saw. The deadlock was not forming; it had already \
                     formed.",
                )
            };
            push(
                &mut chapters,
                "deadlock-visible",
                title,
                i,
                event.at,
                &format!(
                    "{narration} A human later established the causal loop: the cordons \
                     left nowhere to schedule the third web replica, which held \
                     currentHealthy at 2, which held disruptionsAllowed at 0, which \
                     blocked the eviction Brupop needed. FleetForge observed the symptoms; \
                     it did not derive this chain.",
                ),
                ClaimBasis::HumanRca,
            );
        }
    }

    let last = events.len() - 1;
    push(
        &mut chapters,
        "capture-ends",
        "Capture ends",
        last,
        events[last].at,
        "The recording ends during teardown. Node transitions in the final minutes are \
         instances being terminated on purpose, not maintenance.",
        ClaimBasis::ObservedByFleetForge,
    );

    chapters.sort_by_key(|c| c.position);
    chapters
}

/// Whether an event is evidence that Brupop exists in the cluster.
///
/// Namespace-based, because it catches the operator's pods appearing — the
/// earliest observable moment — rather than the first state it publishes.
fn is_brupop_activity(e: &crate::state::ReplayEvent) -> bool {
    const NS: &str = "brupop-bottlerocket-aws";
    e.kind == "brupop_state_changed" || e.raw.get("namespace").and_then(|v| v.as_str()) == Some(NS)
}

/// Chapters for a prevented run.
///
/// The incident's chapters are a descent: cordons, deadlock, blocked. These are
/// the opposite shape — a check, a fix, and an update that simply works — and
/// the first one is the load-bearing claim, because everything else only means
/// something if BLOCKED came before the executor existed.
#[must_use]
fn derive_prevented(timeline: &ReplayTimeline) -> Vec<Chapter> {
    let events = timeline.events();
    if events.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Chapter> = Vec::new();
    let push = |out: &mut Vec<Chapter>,
                id: &str,
                title: &str,
                i: usize,
                narration: String,
                basis: ClaimBasis| {
        out.push(Chapter {
            id: id.to_owned(),
            title: title.to_owned(),
            position: i,
            at: events[i].at,
            narration,
            basis,
        });
    };

    push(
        &mut out,
        "recording-begins",
        "Recording begins",
        0,
        "FleetForge starts watching before anything is installed. Nothing is updating, \
         nothing is cordoned, and the executor does not exist yet — which is the only \
         position from which prevention can be claimed at all."
            .to_owned(),
        ClaimBasis::ObservedByFleetForge,
    );

    // Preflights, in order: the first is BLOCKED, the second SAFE.
    let preflights: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == "preflight_run")
        .map(|(i, _)| i)
        .collect();

    for (n, &i) in preflights.iter().enumerate() {
        let blocked = events[i].raw.get("status").and_then(|v| v.as_str()) == Some("blocked");
        if blocked && n == 0 {
            push(
                &mut out,
                "blocked",
                "FleetForge says BLOCKED",
                i,
                "A PodDisruptionBudget permitting zero disruptions. Had the update started \
                 here it would have stalled on the first eviction — which is exactly what \
                 happened in the earlier capture. Nothing has been installed yet."
                    .to_owned(),
                ClaimBasis::ObservedByFleetForge,
            );
        } else if !blocked {
            push(
                &mut out,
                "safe",
                "FleetForge says SAFE",
                i,
                "After one field changed, a second preflight against a different snapshot \
                 returns SAFE with no blocking findings. Only now is it reasonable to start \
                 the update."
                    .to_owned(),
                ClaimBasis::ObservedByFleetForge,
            );
            break;
        }
    }

    // The earliest trace of the executor existing at all, not the first shadow
    // state change. `brupop_state_changed` only fires once FleetForge's custom
    // resource watch catches up, which here was four minutes after Brupop's
    // pods appeared — late enough to place this chapter *after* the first node
    // reboot, which would have read as nonsense. It also makes the prevention
    // claim harder to satisfy, which is the direction an honest marker should
    // err in.
    if let Some(i) = events.iter().position(is_brupop_activity) {
        push(
            &mut out,
            "executor-arrives",
            "Brupop begins",
            i,
            "The executor appears in the log for the first time — after both preflights. \
             FleetForge did not perform this update and holds no mutating client; it \
             watched."
                .to_owned(),
            ClaimBasis::ObservedByFleetForge,
        );
    }

    // First and last node reboot, as the visible shape of the real update.
    let reboots: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.kind == "node_changed"
                && e.raw.get("ready").and_then(serde_json::Value::as_bool) == Some(false)
        })
        .map(|(i, _)| i)
        .collect();
    if let Some(&i) = reboots.first() {
        push(
            &mut out,
            "first-reboot",
            "First node reboots",
            i,
            "Cordon, drain, update, reboot. The node leaves service and comes back on the \
             new release — and, unlike the earlier capture, it is uncordoned again without \
             anyone intervening."
                .to_owned(),
            ClaimBasis::ObservedByFleetForge,
        );
    }

    let last = events.len() - 1;
    push(
        &mut out,
        "complete",
        "Update complete",
        last,
        "All three nodes reached Bottlerocket 1.64.0. No deadlock, no manual uncordon, no \
         human intervention after the correction."
            .to_owned(),
        ClaimBasis::ObservedByFleetForge,
    );

    out.sort_by_key(|c| c.position);
    out.dedup_by(|a, b| a.position == b.position);
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_timeline_has_no_chapters() {
        for kind in [
            crate::schema::CaptureKind::Incident,
            crate::schema::CaptureKind::Prevented,
        ] {
            assert!(derive(&ReplayTimeline::new(Vec::new()), None, kind).is_empty());
        }
    }
}
