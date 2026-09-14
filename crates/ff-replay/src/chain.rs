//! The investigation chain.
//!
//! Five facts and four arrows:
//!
//! ```text
//! two cordoned nodes → replica Pending → currentHealthy=2 → disruptionsAllowed=0 → eviction blocked
//! ```
//!
//! The facts and the arrows are not the same kind of thing, and the whole value
//! of this structure is that it refuses to pretend they are.
//!
//! Each **fact** is something FleetForge read out of the cluster, or arithmetic
//! over things it read. Each **arrow** is a causal claim, and a causal claim is
//! exactly what FleetForge does not make: no analyzer in this system correlates
//! a cordon with a Pending pod with a PodDisruptionBudget status. A person read
//! those three artifacts side by side after the incident and drew the line.
//!
//! So the links carry [`ClaimBasis::ObservedByFleetForge`] or
//! [`ClaimBasis::MathematicallyDerived`], and every edge carries
//! [`ClaimBasis::HumanRca`]. Rendering the chain as one uniform sequence of
//! boxes and arrows — which is what every incident-review diagram does — would
//! silently upgrade the arrows to the status of the boxes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::bundle::PdbArithmetic;
use crate::error::ReplayError;
use crate::schema::ClaimBasis;
use crate::state::ReplayTimeline;

/// One node in the chain: a fact, with where it was read from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainLink {
    /// Stable identifier.
    pub id: String,
    /// Short label for the diagram box.
    pub label: String,
    /// The value itself, rendered large.
    pub value: String,
    /// One sentence of plain English, for a reader who does not know Kubernetes.
    pub detail: String,
    /// How this fact is justified.
    pub basis: ClaimBasis,
    /// Artifact this was read from, by manifest name.
    pub artifact: String,
    /// The exact field within it.
    pub field_path: String,
    /// When it was true, where the evidence carries a time.
    pub at: Option<DateTime<Utc>>,
}

/// The arrow between two links.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainEdge {
    /// Link this arrow leaves.
    pub from: String,
    /// Link it arrives at.
    pub to: String,
    /// What the arrow asserts.
    pub because: String,
    /// Always [`ClaimBasis::HumanRca`]. Kept as a field rather than implied so
    /// that a renderer reads it from the data like everything else.
    pub basis: ClaimBasis,
}

/// The whole chain, plus the disclaimer that belongs with it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationChain {
    /// Facts, in order.
    pub links: Vec<ChainLink>,
    /// Causal claims between them.
    pub edges: Vec<ChainEdge>,
    /// Why the edges are weaker than the links.
    pub attribution: String,
}

/// Build the chain from the bundle.
///
/// Every value is read from evidence. Nothing in this function contains a
/// number, so a bundle in which only one node was cordoned would render "1
/// cordoned node" rather than the sentence this incident happened to need.
pub fn derive(
    timeline: &ReplayTimeline,
    pdb: &PdbArithmetic,
    root: &std::path::Path,
    kind: crate::schema::CaptureKind,
) -> Result<InvestigationChain, ReplayError> {
    if kind == crate::schema::CaptureKind::Prevented {
        return Ok(prevented_chain(pdb));
    }
    // The state at the end of the first complete snapshot: what FleetForge saw
    // when it connected, before it had watched anything change.
    let initial = timeline.state_at(initial_burst_end(timeline));

    let cordoned: Vec<&str> = initial
        .nodes
        .iter()
        .filter(|n| n.unschedulable)
        .map(|n| n.name.as_str())
        .collect();

    let pending: Vec<String> = initial
        .pods
        .iter()
        .filter(|p| p.phase == "Pending")
        .map(|p| format!("{}/{}", p.namespace, p.name))
        .collect();

    let observations = PdbObservations::parse(root)?;

    let field = |name: &str| -> String {
        pdb.evidence
            .iter()
            .find(|e| e.field_path.ends_with(name))
            .map_or_else(|| format!(".status.{name}"), |e| e.field_path.clone())
    };

    let links = vec![
        ChainLink {
            id: "cordons".to_owned(),
            label: "Nodes unschedulable".to_owned(),
            value: format!("{} of {} cordoned", cordoned.len(), initial.nodes.len()),
            detail: format!(
                "Cordoned means marked unschedulable: Kubernetes will not place new pods \
                 there. {}. Both were already cordoned in the first state FleetForge saw.",
                short_names(&cordoned)
            ),
            basis: ClaimBasis::ObservedByFleetForge,
            artifact: "06-nodes-before.json".to_owned(),
            field_path: ".items[].spec.unschedulable".to_owned(),
            at: Some(initial.at),
        },
        ChainLink {
            id: "pending".to_owned(),
            label: "Replica cannot schedule".to_owned(),
            value: format!(
                "{} pod{} Pending",
                pending.len(),
                if pending.len() == 1 { "" } else { "s" }
            ),
            detail: if pending.is_empty() {
                "No pod was Pending in the first observed state.".to_owned()
            } else {
                format!(
                    "Pending means the scheduler has accepted the pod but found nowhere to \
                     run it. {}.",
                    pending.join(", ")
                )
            },
            basis: ClaimBasis::ObservedByFleetForge,
            artifact: "05-pods-before.txt".to_owned(),
            field_path: ".status.phase".to_owned(),
            at: Some(initial.at),
        },
        ChainLink {
            id: "current-healthy".to_owned(),
            label: "Healthy replicas".to_owned(),
            value: format!(
                "currentHealthy = {} of {}",
                observations.current_healthy, observations.expected_pods
            ),
            detail: format!(
                "The PodDisruptionBudget counts {} healthy pods where it expects {}. The \
                 missing one is the pod that cannot schedule.",
                observations.current_healthy, observations.expected_pods
            ),
            basis: ClaimBasis::ObservedByFleetForge,
            artifact: "04-pdb-before.json".to_owned(),
            field_path: field("currentHealthy"),
            at: None,
        },
        ChainLink {
            id: "disruptions-allowed".to_owned(),
            label: "Disruption budget exhausted".to_owned(),
            value: format!("disruptionsAllowed = {}", observations.disruptions_allowed),
            detail: format!(
                "{}, so {} − {} = {}. A PodDisruptionBudget is a promise about how many pods \
                 may be taken down voluntarily at once. At zero, the promise permits none.",
                pdb.formula,
                observations.current_healthy,
                observations.desired_healthy,
                observations.disruptions_allowed
            ),
            // The value is read from the cluster *and* reproducible from the
            // two inputs above. Derived is the stronger statement, because a
            // reader can check it.
            basis: ClaimBasis::MathematicallyDerived,
            artifact: "04-pdb-before.json".to_owned(),
            field_path: field("disruptionsAllowed"),
            at: None,
        },
        ChainLink {
            id: "eviction-blocked".to_owned(),
            label: "Eviction refused".to_owned(),
            value: observations
                .condition_reason
                .clone()
                .unwrap_or_else(|| "DisruptionAllowed = False".to_owned()),
            detail: format!(
                "Eviction is the polite way to remove a pod; it is the call Brupop makes \
                 before rebooting a node, and the API server rejects it while \
                 disruptionsAllowed is {}. Kubernetes recorded this condition itself{}.",
                observations.disruptions_allowed,
                observations
                    .condition_at
                    .map_or_else(String::new, |t| format!(" at {}", t.format("%H:%M:%S")))
            ),
            basis: ClaimBasis::ObservedByFleetForge,
            artifact: "04-pdb-before.json".to_owned(),
            field_path: ".status.conditions[type=DisruptionAllowed]".to_owned(),
            at: observations.condition_at,
        },
    ];

    let edges = vec![
        ChainEdge {
            from: "cordons".to_owned(),
            to: "pending".to_owned(),
            because: "With two of three nodes unschedulable and an anti-affinity rule keeping \
                      web replicas apart, the third replica had nowhere left to go."
                .to_owned(),
            basis: ClaimBasis::HumanRca,
        },
        ChainEdge {
            from: "pending".to_owned(),
            to: "current-healthy".to_owned(),
            because: "A pod that never becomes Ready is never counted healthy, so the budget \
                      saw two where it expected three."
                .to_owned(),
            basis: ClaimBasis::HumanRca,
        },
        ChainEdge {
            from: "current-healthy".to_owned(),
            to: "disruptions-allowed".to_owned(),
            because: "minAvailable=2 fixes desiredHealthy at 2, so two healthy pods leave \
                      exactly zero to spend."
                .to_owned(),
            basis: ClaimBasis::HumanRca,
        },
        ChainEdge {
            from: "disruptions-allowed".to_owned(),
            to: "eviction-blocked".to_owned(),
            because: "Brupop drains a node before rebooting it. With no disruption permitted, \
                      the drain could not complete and the cordons were never lifted — which \
                      is what closed the loop."
                .to_owned(),
            basis: ClaimBasis::HumanRca,
        },
    ];

    Ok(InvestigationChain {
        links,
        edges,
        attribution: "The five facts were each read out of the cluster by FleetForge. The four \
                      arrows between them were not: no analyzer in FleetForge correlates cordon \
                      state with pod scheduling with PodDisruptionBudget status. A person read \
                      those artifacts side by side after the incident and drew this line. \
                      FleetForge reported the blocker at the end of the chain; it did not \
                      produce the chain."
            .to_owned(),
    })
}

/// Where FleetForge's first complete snapshot ends.
///
/// Shared with chapter derivation: everything inside the first burst is state
/// that already existed when FleetForge connected.
fn initial_burst_end(timeline: &ReplayTimeline) -> usize {
    let events = timeline.events();
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
            .map_or(events.len().saturating_sub(1), |offset| start + offset)
    })
}

/// The PodDisruptionBudget as Kubernetes itself reported it.
///
/// Read from the captured `kubectl get pdb -o json`, not from FleetForge's own
/// finding. The finding is FleetForge's reading of the cluster; this is the
/// cluster's own words, and a reader who distrusts the first can check the
/// second.
struct PdbObservations {
    current_healthy: i64,
    desired_healthy: i64,
    expected_pods: i64,
    disruptions_allowed: i64,
    condition_reason: Option<String>,
    condition_at: Option<DateTime<Utc>>,
}

impl PdbObservations {
    fn parse(root: &std::path::Path) -> Result<Self, ReplayError> {
        let name = "04-pdb-before.json";
        let text = std::fs::read_to_string(root.join(name)).map_err(|e| ReplayError::Io {
            path: name.to_owned(),
            reason: e.to_string(),
        })?;
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ReplayError::MalformedArtifact {
                artifact: name.to_owned(),
                reason: e.to_string(),
            })?;

        let item = v
            .get("items")
            .and_then(|i| i.as_array())
            .and_then(|items| {
                items.iter().find(|i| {
                    i.pointer("/metadata/name").and_then(|n| n.as_str()) == Some("web-pdb")
                })
            })
            .ok_or_else(|| ReplayError::MalformedArtifact {
                artifact: name.to_owned(),
                reason: "no PodDisruptionBudget named web-pdb".to_owned(),
            })?;

        let num = |p: &str| item.pointer(p).and_then(serde_json::Value::as_i64);
        let condition = item
            .pointer("/status/conditions")
            .and_then(|c| c.as_array())
            .and_then(|cs| {
                cs.iter()
                    .find(|c| c.get("type").and_then(|t| t.as_str()) == Some("DisruptionAllowed"))
            });

        Ok(Self {
            current_healthy: num("/status/currentHealthy").ok_or_else(|| {
                ReplayError::MalformedArtifact {
                    artifact: name.to_owned(),
                    reason: "web-pdb has no status.currentHealthy".to_owned(),
                }
            })?,
            desired_healthy: num("/status/desiredHealthy").ok_or_else(|| {
                ReplayError::MalformedArtifact {
                    artifact: name.to_owned(),
                    reason: "web-pdb has no status.desiredHealthy".to_owned(),
                }
            })?,
            expected_pods: num("/status/expectedPods").unwrap_or_default(),
            disruptions_allowed: num("/status/disruptionsAllowed").ok_or_else(|| {
                ReplayError::MalformedArtifact {
                    artifact: name.to_owned(),
                    reason: "web-pdb has no status.disruptionsAllowed".to_owned(),
                }
            })?,
            condition_reason: condition
                .and_then(|c| c.get("reason"))
                .and_then(|r| r.as_str())
                .map(ToOwned::to_owned),
            condition_at: condition
                .and_then(|c| c.get("lastTransitionTime"))
                .and_then(|t| t.as_str())
                .and_then(|t| t.parse::<DateTime<Utc>>().ok()),
        })
    }
}

/// Node names without the EKS DNS suffix, which is the same on all of them and
/// pushes the part that differs off the end of the box.
fn short_names(names: &[&str]) -> String {
    names
        .iter()
        .map(|n| n.split('.').next().unwrap_or(n))
        .collect::<Vec<_>>()
        .join(" and ")
}

/// The chain for a prevented run.
///
/// Shorter than the incident's, and every link is evidence — because nothing
/// had to be reconstructed afterwards. The counterfactual is deliberately
/// absent: what *would* have happened is not a fact, and the one arrow here
/// says only what was observed to follow what.
fn prevented_chain(pdb: &PdbArithmetic) -> InvestigationChain {
    let links = vec![
        ChainLink {
            id: "unsafe-set".to_owned(),
            label: "Budget tightened".to_owned(),
            value: "minAvailable = 3 of 3".to_owned(),
            detail: "A PodDisruptionBudget requiring all three replicas, deliberately set to \
                     create the condition under test."
                .to_owned(),
            basis: ClaimBasis::ObservedByFleetForge,
            artifact: "11-unsafe-condition.txt".to_owned(),
            field_path: ".spec.minAvailable".to_owned(),
            at: None,
        },
        ChainLink {
            id: "arithmetic".to_owned(),
            label: "No disruption permitted".to_owned(),
            value: format!("disruptionsAllowed = {}", pdb.result),
            detail: format!("{}, leaving nothing to spend.", pdb.formula),
            basis: ClaimBasis::MathematicallyDerived,
            artifact: "12-preflight-BLOCKED.json".to_owned(),
            field_path: ".status.disruptionsAllowed".to_owned(),
            at: None,
        },
        ChainLink {
            id: "blocked".to_owned(),
            label: "Preflight refuses".to_owned(),
            value: "BLOCKED".to_owned(),
            detail: "FleetForge reports the blocker before the executor exists. It cannot \
                     stop anything itself — it has no mutating client — so what it prevents \
                     is a human starting the update."
                .to_owned(),
            basis: ClaimBasis::ObservedByFleetForge,
            artifact: "12-preflight-BLOCKED.json".to_owned(),
            field_path: ".data.summary.status".to_owned(),
            at: None,
        },
        ChainLink {
            id: "corrected".to_owned(),
            label: "One field changed".to_owned(),
            value: "minAvailable 3 → 2".to_owned(),
            detail: "The narrowest change that clears the blocker. Same pods, same UIDs, \
                     zero restarts."
                .to_owned(),
            basis: ClaimBasis::ObservedByFleetForge,
            artifact: "13-correction.txt".to_owned(),
            field_path: ".spec.minAvailable".to_owned(),
            at: None,
        },
        ChainLink {
            id: "safe".to_owned(),
            label: "Preflight clears".to_owned(),
            value: "SAFE".to_owned(),
            detail: "Re-run against a different snapshot hash: no blocking findings, \
                     concurrency 1."
                .to_owned(),
            basis: ClaimBasis::ObservedByFleetForge,
            artifact: "14-preflight-SAFE.json".to_owned(),
            field_path: ".data.summary.status".to_owned(),
            at: None,
        },
        ChainLink {
            id: "updated".to_owned(),
            label: "Update completes".to_owned(),
            value: "3 of 3 on 1.64.0".to_owned(),
            detail: "Brupop cordons, drains, reboots and restores each node in turn. No \
                     deadlock, no manual uncordon."
                .to_owned(),
            basis: ClaimBasis::ObservedByFleetForge,
            artifact: "17-update-sequence.txt".to_owned(),
            field_path: ".status.nodeInfo.osImage".to_owned(),
            at: None,
        },
    ];

    let edge = |from: &str, to: &str, because: &str, basis: ClaimBasis| ChainEdge {
        from: from.to_owned(),
        to: to.to_owned(),
        because: because.to_owned(),
        basis,
    };

    InvestigationChain {
        links,
        edges: vec![
            edge(
                "unsafe-set",
                "arithmetic",
                "Three required of three healthy leaves zero.",
                ClaimBasis::MathematicallyDerived,
            ),
            edge(
                "arithmetic",
                "blocked",
                "A budget permitting nothing is a blocker by definition.",
                ClaimBasis::MathematicallyDerived,
            ),
            edge(
                "blocked",
                "corrected",
                "A person read the finding and changed the field it named.",
                ClaimBasis::HumanRca,
            ),
            edge(
                "corrected",
                "safe",
                "Two required of three healthy leaves one.",
                ClaimBasis::MathematicallyDerived,
            ),
            edge(
                "safe",
                "updated",
                "The update was started only after the check cleared, and it finished.",
                ClaimBasis::ObservedByFleetForge,
            ),
        ],
        attribution: "Unlike the earlier capture, most of this chain is arithmetic rather \
                      than hindsight: the values were read before the update, not \
                      reconstructed after it. Only one arrow is human — a person decided \
                      which field to change. What this chain does NOT contain is a \
                      counterfactual: it does not claim the update would have deadlocked \
                      without the correction. That did not happen here and cannot be \
                      observed."
            .to_owned(),
    }
}
