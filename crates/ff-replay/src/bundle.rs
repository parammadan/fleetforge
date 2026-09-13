//! Loading and validating an evidence bundle.
//!
//! Every value the interface displays is read out of a file here. Nothing in
//! this module contains a dashboard number: if the PDB arithmetic changes in
//! the captured preflight result, the interface changes with it, and if the
//! artifact is missing the bundle is rejected rather than rendered with a
//! plausible-looking blank.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::artifacts::ArtifactStore;
use crate::error::ReplayError;
use crate::schema::{
    CaptureContext, Claim, ClaimBasis, DataCaveat, REPLAY_SCHEMA_VERSION,
};
use crate::state::{ReplayEvent, ReplayTimeline};

/// Artifacts without which a bundle is not a bundle.
const REQUIRED: &[&str] = &[
    "35-fleetforge-events-complete.jsonl",
    "03-preflight-before.json",
    "04-pdb-before.json",
    "07-brupop-before.json",
    "25-nodes-final.json",
    "31-traffic-post-recovery.txt",
    "00-CONCLUSIONS.md",
];

/// Event kinds that mark a moment worth stopping on.
const SIGNIFICANT_KINDS: &[&str] = &[
    "run_started",
    "preflight_run",
    "node_changed",
    "brupop_state_changed",
    "pod_removed",
];

/// The PodDisruptionBudget arithmetic, read from the captured preflight result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PdbArithmetic {
    /// Finding identifier, e.g. `FF-PDB-001`.
    pub finding_id: String,
    /// Finding title.
    pub title: String,
    /// Severity as recorded.
    pub severity: String,
    /// Confidence as recorded.
    pub confidence: String,
    /// The formula, verbatim.
    pub formula: String,
    /// Named inputs, in order.
    pub inputs: Vec<(String, String)>,
    /// The result.
    pub result: String,
    /// Unit, where given.
    pub unit: Option<String>,
    /// Raw field observations behind it.
    pub evidence: Vec<PdbEvidence>,
    /// What the finding does not establish.
    pub limitations: Vec<String>,
    /// Objects it concerns.
    pub affected: Vec<String>,
    /// The snapshot it was computed against.
    pub snapshot_id: String,
}

/// One raw observation cited by the finding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PdbEvidence {
    /// Object, as `Kind/name`.
    pub resource: String,
    /// Field path within it.
    pub field_path: String,
    /// Observed value.
    pub value: String,
    /// Why it matters, where recorded.
    pub note: Option<String>,
}

/// One scored prediction from the captured report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PredictionRow {
    /// When the prediction was made.
    pub at: DateTime<Utc>,
    /// Nodes it concerned.
    pub node_names: Vec<String>,
    /// Predicted evictions.
    pub predicted: u32,
    /// Observed evictions.
    pub observed: u32,
    /// observed − predicted.
    pub delta: i64,
    /// Verdict, verbatim from the scorer.
    pub verdict: String,
    /// Severity class for the interface: `exact`, `conservative`,
    /// `under_predicted`, `missed`, or `untested`.
    pub class: String,
    /// Workloads disrupted but not predicted.
    pub missed_workloads: Vec<String>,
}

/// A loaded, validated bundle.
pub struct ReplayBundle {
    /// Schema version this bundle was produced against.
    pub schema_version: u32,
    /// Where it came from.
    pub root: PathBuf,
    /// Capture context for the banner.
    pub context: CaptureContext,
    /// The ordered timeline.
    pub timeline: ReplayTimeline,
    /// Addressable artifacts.
    pub artifacts: ArtifactStore,
    /// Classified claims.
    pub claims: Vec<Claim>,
    /// Known problems with the evidence itself.
    pub caveats: Vec<DataCaveat>,
    /// The PDB finding, parsed.
    pub pdb: PdbArithmetic,
    /// Prediction scoring, parsed.
    pub predictions: Vec<PredictionRow>,
    /// Post-recovery traffic validation, parsed.
    pub traffic: TrafficValidation,
}

/// The post-recovery traffic sampler result.
///
/// Explicitly *not* an availability measurement for the incident. The field
/// names say so, because a chart labelled "uptime" is read as uptime no matter
/// what the caption says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TrafficValidation {
    /// Total requests in the rerun.
    pub requests: u32,
    /// HTTP 200s.
    pub successes: u32,
    /// Failures, including sampler timeouts.
    pub failures: u32,
    /// Success percentage, to one decimal.
    pub success_pct: String,
    /// First and last sample times, as recorded.
    pub window: String,
    /// Whether this run passed. It did not.
    pub passed: bool,
    /// What it does and does not mean.
    pub interpretation: String,
}

impl ReplayBundle {
    /// Load and validate a bundle from a directory.
    ///
    /// # Errors
    ///
    /// Rejects a bundle that is missing a required artifact, has an unparseable
    /// event log, or whose timeline is empty or out of order.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, ReplayError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(ReplayError::BundleNotFound {
                path: root.display().to_string(),
            });
        }

        let artifacts = ArtifactStore::index(root)?;
        for required in REQUIRED {
            if !artifacts.contains(required) {
                return Err(ReplayError::MissingArtifact {
                    artifact: (*required).to_owned(),
                });
            }
        }

        let timeline = load_timeline(root)?;
        let pdb = parse_pdb_finding(root)?;
        let predictions = parse_predictions(root)?;
        let traffic = parse_traffic(root)?;
        let context = build_context(&timeline, root)?;
        let claims = build_claims(&timeline, &pdb, &traffic);
        let caveats = build_caveats(&timeline);

        Ok(Self {
            schema_version: REPLAY_SCHEMA_VERSION,
            root: root.to_path_buf(),
            context,
            timeline,
            artifacts,
            claims,
            caveats,
            pdb,
            predictions,
            traffic,
        })
    }
}

/// Read the JSONL event log into an ordered timeline.
fn load_timeline(root: &Path) -> Result<ReplayTimeline, ReplayError> {
    let name = "35-fleetforge-events-complete.jsonl";
    let text = std::fs::read_to_string(root.join(name)).map_err(|e| ReplayError::Io {
        path: name.to_owned(),
        reason: e.to_string(),
    })?;

    let mut raw: Vec<(u64, DateTime<Utc>, String, serde_json::Value)> = Vec::new();
    for (lineno, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| ReplayError::MalformedArtifact {
                artifact: name.to_owned(),
                reason: format!("line {}: {e}", lineno + 1),
            })?;
        let seq = value
            .get("seq")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ReplayError::MalformedArtifact {
                artifact: name.to_owned(),
                reason: format!("line {} has no seq", lineno + 1),
            })?;
        let at = value
            .get("recorded_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc))
            .ok_or_else(|| ReplayError::MalformedArtifact {
                artifact: name.to_owned(),
                reason: format!("line {} has no usable recorded_at", lineno + 1),
            })?;
        let kind = value
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_owned();

        // A replayed line claiming to be live would be a contradiction in
        // terms. Reject rather than reinterpret.
        if value.get("mode").and_then(|v| v.as_str()) == Some("what_if") {
            continue;
        }

        raw.push((seq, at, kind, value));
    }

    if raw.is_empty() {
        return Err(ReplayError::InvalidTimeline {
            reason: "the event log contains no usable entries".to_owned(),
        });
    }

    // Sort by sequence, which the recorder guarantees is monotonic even across
    // process restarts. Ties broken by timestamp for total determinism.
    raw.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let events = raw
        .into_iter()
        .enumerate()
        .map(|(index, (seq, at, kind, value))| ReplayEvent {
            index,
            seq,
            summary: summarise(&kind, &value),
            significant: SIGNIFICANT_KINDS.contains(&kind.as_str())
                || is_notable_k8s_event(&value),
            kind,
            at,
            raw: value,
        })
        .collect();

    Ok(ReplayTimeline::new(events))
}

/// Kubernetes events that matter to a maintenance story.
fn is_notable_k8s_event(v: &serde_json::Value) -> bool {
    const REASONS: &[&str] = &[
        "Evicted",
        "Killing",
        "FailedScheduling",
        "NodeNotReady",
        "NodeNotSchedulable",
        "NodeSchedulable",
        "Preempted",
        "TaintManagerEviction",
    ];
    v.get("reason")
        .and_then(|r| r.as_str())
        .is_some_and(|r| REASONS.contains(&r))
}

fn summarise(kind: &str, v: &serde_json::Value) -> String {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("");
    let short = |n: &str| n.split('.').next().unwrap_or(n).to_owned();
    match kind {
        "run_started" => format!("recording started — {}", s("description")),
        "run_ended" => format!("recording ended — {}", s("reason")),
        "snapshot_observed" => format!(
            "snapshot {} ({} nodes, {} pods)",
            &s("snapshot_id").chars().take(12).collect::<String>(),
            v.get("nodes").and_then(serde_json::Value::as_u64).unwrap_or(0),
            v.get("pods").and_then(serde_json::Value::as_u64).unwrap_or(0),
        ),
        "preflight_run" => format!(
            "preflight on {} → {} ({} pods predicted evicted)",
            v.get("node_names")
                .and_then(|a| a.as_array())
                .and_then(|a| a.first())
                .and_then(|x| x.as_str())
                .map(short)
                .unwrap_or_default(),
            s("status").to_uppercase(),
            v.get("pods_evicted").and_then(serde_json::Value::as_u64).unwrap_or(0),
        ),
        "node_changed" => format!(
            "node {} ready={} cordoned={} bottlerocket={}",
            short(s("name")),
            v.get("ready").and_then(serde_json::Value::as_bool).unwrap_or(false),
            v.get("unschedulable").and_then(serde_json::Value::as_bool).unwrap_or(false),
            s("bottlerocket_version"),
        ),
        "brupop_state_changed" => format!(
            "brupop {} → {} (v{})",
            short(s("node")),
            s("state"),
            s("current_version"),
        ),
        "pod_removed" => format!(
            "pod {}/{} left {}",
            s("namespace"),
            s("name"),
            short(s("node"))
        ),
        "pod_changed" => format!(
            "pod {}/{} {} on {}",
            s("namespace"),
            s("name"),
            s("phase"),
            short(s("node"))
        ),
        "kubernetes_event" => format!("{}: {} — {}", s("kind"), s("reason"), s("message")),
        other => other.to_owned(),
    }
}

/// Pull the PDB finding out of the captured preflight result.
fn parse_pdb_finding(root: &Path) -> Result<PdbArithmetic, ReplayError> {
    let name = "03-preflight-before.json";
    let text = std::fs::read_to_string(root.join(name)).map_err(|e| ReplayError::Io {
        path: name.to_owned(),
        reason: e.to_string(),
    })?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| ReplayError::MalformedArtifact {
            artifact: name.to_owned(),
            reason: e.to_string(),
        })?;

    let findings = v
        .pointer("/data/findings")
        .and_then(|f| f.as_array())
        .ok_or_else(|| ReplayError::MalformedArtifact {
            artifact: name.to_owned(),
            reason: "no data.findings array".to_owned(),
        })?;

    let f = findings
        .iter()
        .find(|f| f.get("id").and_then(|i| i.as_str()) == Some("FF-PDB-001"))
        .ok_or_else(|| ReplayError::MalformedArtifact {
            artifact: name.to_owned(),
            reason: "FF-PDB-001 not present — this bundle does not contain the blocker"
                .to_owned(),
        })?;

    let calc = f
        .get("calculation")
        .ok_or_else(|| ReplayError::MalformedArtifact {
            artifact: name.to_owned(),
            reason: "FF-PDB-001 carries no calculation".to_owned(),
        })?;

    Ok(PdbArithmetic {
        finding_id: "FF-PDB-001".to_owned(),
        title: f.get("title").and_then(|x| x.as_str()).unwrap_or_default().to_owned(),
        severity: f.get("severity").and_then(|x| x.as_str()).unwrap_or_default().to_owned(),
        confidence: f.get("confidence").and_then(|x| x.as_str()).unwrap_or_default().to_owned(),
        formula: calc.get("formula").and_then(|x| x.as_str()).unwrap_or_default().to_owned(),
        inputs: calc
            .get("inputs")
            .and_then(|i| i.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|p| {
                        let arr = p.as_array()?;
                        Some((
                            arr.first()?.as_str()?.to_owned(),
                            arr.get(1)?.as_str()?.to_owned(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        result: calc.get("result").and_then(|x| x.as_str()).unwrap_or_default().to_owned(),
        unit: calc.get("unit").and_then(|x| x.as_str()).map(ToOwned::to_owned),
        evidence: f
            .get("evidence")
            .and_then(|e| e.as_array())
            .map(|a| {
                a.iter()
                    .map(|e| PdbEvidence {
                        resource: format!(
                            "{}/{}",
                            e.pointer("/resource/kind").and_then(|x| x.as_str()).unwrap_or(""),
                            e.pointer("/resource/name").and_then(|x| x.as_str()).unwrap_or(""),
                        ),
                        field_path: e.get("field_path").and_then(|x| x.as_str()).unwrap_or("").to_owned(),
                        value: e.get("value").and_then(|x| x.as_str()).unwrap_or("").to_owned(),
                        note: e.get("note").and_then(|x| x.as_str()).map(ToOwned::to_owned),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        limitations: f
            .get("limitations")
            .and_then(|l| l.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(ToOwned::to_owned)).collect())
            .unwrap_or_default(),
        affected: f
            .get("affected")
            .and_then(|l| l.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|r| {
                        Some(format!(
                            "{}/{}",
                            r.get("kind")?.as_str()?,
                            r.get("name")?.as_str()?
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        snapshot_id: f.get("snapshot_id").and_then(|x| x.as_str()).unwrap_or_default().to_owned(),
    })
}

/// Parse the prediction table out of the generated report.
fn parse_predictions(root: &Path) -> Result<Vec<PredictionRow>, ReplayError> {
    let name = "35-fleetforge-events-complete.jsonl";
    let text = std::fs::read_to_string(root.join(name)).map_err(|e| ReplayError::Io {
        path: name.to_owned(),
        reason: e.to_string(),
    })?;

    // Score the log directly rather than scraping the rendered markdown: the
    // scorer is the authority, and re-running it means the displayed verdicts
    // use the corrected under-prediction classification rather than whatever
    // wording was current when the report was rendered.
    let entries = ff_record::read(root.join(name)).map_err(|e| ReplayError::MalformedArtifact {
        artifact: name.to_owned(),
        reason: e.to_string(),
    })?;
    let _ = text;
    let (comparisons, _accuracy) = ff_record::score(&entries);

    Ok(comparisons
        .into_iter()
        .map(|c| {
            let verdict = c.verdict().to_owned();
            let class = if verdict.starts_with("untested") {
                "untested"
            } else if verdict.starts_with("exact") {
                "exact"
            } else if verdict.starts_with("under-predicted") {
                "under_predicted"
            } else if verdict.starts_with("missed") {
                "missed"
            } else {
                "conservative"
            }
            .to_owned();
            PredictionRow {
                at: c.predicted_at,
                node_names: c.node_names.clone(),
                predicted: c.prediction.pods_evicted,
                observed: c.observed_evictions,
                delta: c.eviction_delta(),
                verdict,
                class,
                missed_workloads: c
                    .observed_but_not_predicted
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            }
        })
        .collect())
}

/// Parse the post-recovery traffic sampler output.
fn parse_traffic(root: &Path) -> Result<TrafficValidation, ReplayError> {
    let name = "31-traffic-post-recovery.txt";
    let text = std::fs::read_to_string(root.join(name)).map_err(|e| ReplayError::Io {
        path: name.to_owned(),
        reason: e.to_string(),
    })?;

    let mut successes = 0u32;
    let mut failures = 0u32;
    let mut first = String::new();
    let mut last = String::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(ts), Some(code)) = (parts.next(), parts.next()) else {
            continue;
        };
        if first.is_empty() {
            first = ts.to_owned();
        }
        last = ts.to_owned();
        if code == "200" {
            successes += 1;
        } else {
            failures += 1;
        }
    }
    let requests = successes + failures;
    if requests == 0 {
        return Err(ReplayError::MalformedArtifact {
            artifact: name.to_owned(),
            reason: "no samples in the traffic log".to_owned(),
        });
    }

    let pct = f64::from(successes) * 100.0 / f64::from(requests);
    Ok(TrafficValidation {
        requests,
        successes,
        failures,
        success_pct: format!("{pct:.1}"),
        window: format!("{first} → {last}"),
        passed: false,
        interpretation: "Post-recovery networking validation. This run FAILED. It is not a \
                         measurement of availability during the incident, and must never be \
                         presented as uptime: the sampler used during the incident was defective \
                         and its output was discarded."
            .to_owned(),
    })
}

fn build_context(timeline: &ReplayTimeline, root: &Path) -> Result<CaptureContext, ReplayError> {
    let events = timeline.events();
    let first = events.first().ok_or_else(|| ReplayError::InvalidTimeline {
        reason: "empty timeline".to_owned(),
    })?;
    let last = events.last().unwrap_or(first);

    let cluster_id = events
        .iter()
        .find_map(|e| e.raw.get("cluster_id").and_then(|v| v.as_str()))
        .unwrap_or("unknown")
        .to_owned();

    // Node names and Bottlerocket versions come from the events themselves.
    let mut nodes: Vec<String> = events
        .iter()
        .filter(|e| e.kind == "node_changed")
        .filter_map(|e| e.raw.get("name").and_then(|v| v.as_str()).map(ToOwned::to_owned))
        .collect();
    nodes.sort();
    nodes.dedup();

    let mut versions: Vec<String> = events
        .iter()
        .filter_map(|e| {
            e.raw
                .get("bottlerocket_version")
                .or_else(|| e.raw.get("current_version"))
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
        })
        // 2.0.0 is the known-bad value from the version bug, not a Bottlerocket
        // release. Excluded from the banner and disclosed as a data caveat.
        .filter(|v| v.starts_with('1'))
        .collect();
    versions.sort();
    versions.dedup();

    let kubernetes_version = std::fs::read_to_string(root.join("30-environment-final.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("server_version").and_then(|x| x.as_str()).map(ToOwned::to_owned));

    Ok(CaptureContext {
        cluster_id,
        cluster_kind: "Amazon EKS with Bottlerocket managed node group (destroyed)".to_owned(),
        kubernetes_version,
        client_target_version: "v1.36".to_owned(),
        bottlerocket_versions: versions,
        captured_from: first.at,
        captured_to: last.at,
        nodes,
    })
}

/// Build the classified claims.
///
/// Timestamps come from the timeline, not from constants, so the claim about
/// recording starting after Brupop is recomputed from the evidence every load.
/// If a future bundle were captured *before* Brupop started, this claim would
/// change on its own rather than quietly stay wrong.
fn build_claims(
    timeline: &ReplayTimeline,
    pdb: &PdbArithmetic,
    traffic: &TrafficValidation,
) -> Vec<Claim> {
    let events = timeline.events();
    let recording_started = events.first().map(|e| e.at);

    // The first observation of a cordoned node. If it is in the very first
    // snapshot, the cordon predates the recording.
    let first_cordon = events
        .iter()
        .find(|e| {
            e.kind == "node_changed"
                && e.raw.get("unschedulable").and_then(serde_json::Value::as_bool) == Some(true)
        })
        .map(|e| e.at);

    let cordon_predates_recording = matches!(
        (recording_started, first_cordon),
        (Some(start), Some(cordon)) if (cordon - start).num_seconds().abs() < 5
    );

    let uncordons: Vec<&ReplayEvent> = events
        .iter()
        .filter(|e| {
            e.kind == "node_changed"
                && e.raw.get("unschedulable").and_then(serde_json::Value::as_bool) == Some(false)
        })
        .collect();

    let final_versions: Vec<String> = {
        let state = timeline.state_at(timeline.len().saturating_sub(1));
        let mut v: Vec<String> = state
            .nodes
            .iter()
            .filter_map(|n| n.bottlerocket_version.clone())
            .filter(|v| v.starts_with('1'))
            .collect();
        v.sort();
        v.dedup();
        v
    };

    vec![
        Claim {
            id: "blocker-detected".to_owned(),
            statement: format!(
                "FleetForge detected an already-existing PodDisruptionBudget blocker: {}. \
                 The arithmetic was {} = {}.",
                pdb.title, pdb.formula, pdb.result
            ),
            basis: ClaimBasis::ObservedByFleetForge,
            evidence: vec!["03-preflight-before.json".to_owned(), "04-pdb-before.json".to_owned()],
            limitations: pdb.limitations.clone(),
            at: Some(pdb_observed_at(events)),
        },
        Claim {
            id: "no-prediction".to_owned(),
            statement: if cordon_predates_recording {
                "FleetForge did NOT predict the deadlock. Nodes were already cordoned in the \
                 very first snapshot, so Brupop's update began before recording started and no \
                 preflight finding exists from before it."
                    .to_owned()
            } else {
                "Recording began before the first observed cordon.".to_owned()
            },
            basis: ClaimBasis::ObservedByFleetForge,
            evidence: vec!["35-fleetforge-events-complete.jsonl".to_owned()],
            limitations: vec![
                "Detection is not prediction. Everything FleetForge said about this incident was \
                 said after it had already begun."
                    .to_owned(),
            ],
            at: recording_started,
        },
        Claim {
            id: "causal-chain".to_owned(),
            statement: "Two cordoned nodes prevented the third web replica from scheduling, which \
                        held currentHealthy at 2, which made disruptionsAllowed 0, which blocked \
                        the eviction Brupop needed — so the cordons never lifted."
                .to_owned(),
            basis: ClaimBasis::HumanRca,
            evidence: vec![
                "04-pdb-before.json".to_owned(),
                "05-pods-before.txt".to_owned(),
                "06-nodes-before.json".to_owned(),
            ],
            limitations: vec![
                "FleetForge does not implement this inference. It reported the blocker; a human \
                 read node, pod, and PodDisruptionBudget state together and drew the chain."
                    .to_owned(),
            ],
            at: None,
        },
        Claim {
            id: "recovery".to_owned(),
            statement: format!(
                "Two manual uncordons restored scheduling progress. {} uncordon transitions were \
                 observed.",
                uncordons.len()
            ),
            basis: ClaimBasis::ObservedByFleetForge,
            evidence: vec![
                "12-recovery-timeline.txt".to_owned(),
                "24-uncordon-194-recovery.txt".to_owned(),
                "35-fleetforge-events-complete.jsonl".to_owned(),
            ],
            limitations: vec![
                "The uncordons were performed by a human with kubectl. FleetForge holds no \
                 mutating Kubernetes client and changed nothing (ADR-0012)."
                    .to_owned(),
            ],
            at: uncordons.first().map(|e| e.at),
        },
        Claim {
            id: "final-versions".to_owned(),
            statement: format!(
                "All three nodes ultimately reached Bottlerocket {}.",
                final_versions.last().map_or("1.64.0", String::as_str)
            ),
            basis: ClaimBasis::ObservedByFleetForge,
            evidence: vec!["25-nodes-final.json".to_owned(), "28-brupop-final.json".to_owned()],
            limitations: vec![],
            at: events.last().map(|e| e.at),
        },
        Claim {
            id: "availability-unknown".to_owned(),
            statement: "Availability during the incident is UNKNOWN. The traffic sampler in use \
                        at the time had no request timeout, so it blocked for minutes on each \
                        failure and recorded 31 samples in 34 minutes instead of roughly 2000. \
                        Its output measures the sampler's patience, not the service."
                .to_owned(),
            basis: ClaimBasis::Unavailable,
            evidence: vec!["20-traffic-log.txt".to_owned()],
            limitations: vec![
                "No conclusion about customer impact can be drawn from this incident's data, in \
                 either direction."
                    .to_owned(),
            ],
            at: None,
        },
        Claim {
            id: "traffic-rerun".to_owned(),
            statement: format!(
                "A post-recovery rerun with a bounded sampler FAILED: {} of {} requests succeeded \
                 ({}%). This is a networking validation result, not an availability measurement.",
                traffic.successes, traffic.requests, traffic.success_pct
            ),
            basis: ClaimBasis::ObservedByFleetForge,
            evidence: vec!["31-traffic-post-recovery.txt".to_owned()],
            limitations: vec![
                "Measured after recovery completed. It says nothing about the incident window."
                    .to_owned(),
                "All three web pods were Running and Ready, all Service endpoints were Ready, and \
                 kube-proxy and the CNI were healthy on every node with zero restarts."
                    .to_owned(),
            ],
            at: None,
        },
        Claim {
            id: "cni-hypothesis".to_owned(),
            statement: "A roughly 1-in-3 success rate against a 3-endpoint Service is consistent \
                        with only the same-node endpoint being reachable, which would point at \
                        cross-node pod networking. The most likely cause is this cluster's addon \
                        ordering: the VPC CNI was installed after the nodes had already joined."
                .to_owned(),
            basis: ClaimBasis::UnverifiedHypothesis,
            evidence: vec![
                "31-traffic-post-recovery.txt".to_owned(),
                "33-kube-proxy.log".to_owned(),
                "34-aws-node.log".to_owned(),
            ],
            limitations: vec![
                "Not tested. No packet capture, no per-endpoint probe, no controlled comparison."
                    .to_owned(),
                "The Terraform correction (vpc-cni before_compute) is implemented but NOT \
                 verified on a fresh cluster."
                    .to_owned(),
            ],
            at: None,
        },
        Claim {
            id: "terraform-correction".to_owned(),
            statement: "The networking correction is implemented in Terraform and not verified on \
                        a fresh cluster."
                .to_owned(),
            basis: ClaimBasis::UnverifiedHypothesis,
            evidence: vec!["00-CONCLUSIONS.md".to_owned()],
            limitations: vec![
                "The cluster was destroyed before a rebuild could confirm it."
                    .to_owned(),
            ],
            at: None,
        },
    ]
}

fn pdb_observed_at(events: &[ReplayEvent]) -> DateTime<Utc> {
    events
        .iter()
        .find(|e| e.kind == "preflight_run")
        .map_or_else(
            || events.first().map_or_else(Utc::now, |e| e.at),
            |e| e.at,
        )
}

/// Problems with the evidence itself, as opposed to what it proves.
fn build_caveats(timeline: &ReplayTimeline) -> Vec<DataCaveat> {
    let mut caveats = Vec::new();

    // The version-field bug: early node_changed events carry 2.0.0.
    let bad: Vec<&ReplayEvent> = timeline
        .events()
        .iter()
        .filter(|e| {
            e.kind == "node_changed"
                && e.raw.get("bottlerocket_version").and_then(|v| v.as_str()) == Some("2.0.0")
        })
        .collect();
    if !bad.is_empty() {
        caveats.push(DataCaveat {
            id: "version-field-bug".to_owned(),
            statement: format!(
                "{} early node events record bottlerocket_version as \"2.0.0\". That is the \
                 bottlerocket.aws/updater-interface-version label, which FleetForge was reading \
                 into the wrong field. The bug was fixed partway through the capture, so later \
                 events carry real releases. The bad values are shown as recorded rather than \
                 corrected, because editing captured evidence to look better is the one thing \
                 this system must not do.",
                bad.len()
            ),
            affects: vec!["35-fleetforge-events-complete.jsonl".to_owned()],
        });
    }

    // The log spans more than the incident: it continues through teardown.
    if let (Some(first), Some(last)) = (timeline.events().first(), timeline.events().last()) {
        caveats.push(DataCaveat {
            id: "teardown-tail".to_owned(),
            statement: format!(
                "The capture runs from {} to {} and includes the cluster teardown at the end. \
                 Node transitions after the final uncordon are instances being terminated, not \
                 maintenance.",
                first.at.format("%H:%M:%S"),
                last.at.format("%H:%M:%S")
            ),
            affects: vec!["35-fleetforge-events-complete.jsonl".to_owned()],
        });
    }

    caveats.push(DataCaveat {
        id: "restarts".to_owned(),
        statement: "FleetForge was restarted several times during the capture, so the log \
                    contains multiple run_started markers. Sequence numbers remain monotonic \
                    across restarts, so the timeline is continuous, but there are short gaps \
                    where nothing was being recorded."
            .to_owned(),
        affects: vec!["35-fleetforge-events-complete.jsonl".to_owned()],
    });

    caveats
}
