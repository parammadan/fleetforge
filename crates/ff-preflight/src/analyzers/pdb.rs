//! PodDisruptionBudget analysis.
//!
//! The centrepiece check, and the only one whose conclusion is `Certain`: the
//! eviction API itself will refuse when `disruptionsAllowed` is zero, so this
//! is not a prediction about the scheduler — it is a statement about what the
//! API server will do.

use std::collections::BTreeMap;

use ff_core::{Confidence, Finding, PdbFact, PodFact, Result, Severity};

use crate::analyzer::{Analyzer, FindingBuilder};
use crate::context::AnalysisContext;

/// Detects PodDisruptionBudgets that forbid the proposed disruption.
pub struct PdbAnalyzer;

impl Analyzer for PdbAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-PDB"
    }

    fn describes(&self) -> &'static str {
        "PodDisruptionBudgets that prevent voluntary disruption of the selected nodes"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Pod", "PodDisruptionBudget"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();
        let evicted = ctx.evicted_pods();
        let mut findings = Vec::new();

        // Group evicted pods by the PDB covering them. A pod can be covered by
        // more than one, and each is evaluated on its own terms.
        let mut by_pdb: BTreeMap<(String, String), (&PdbFact, Vec<&PodFact>)> = BTreeMap::new();
        for pod in &evicted {
            for pdb in ctx.pdbs_for(pod) {
                by_pdb
                    .entry((pdb.namespace.clone(), pdb.name.clone()))
                    .or_insert_with(|| (pdb, Vec::new()))
                    .1
                    .push(pod);
            }
        }

        for ((namespace, name), (pdb, covered)) in by_pdb {
            // How many covered pods sit on each selected node. A drain takes a
            // whole node at once, so the worst single node is what decides
            // whether the operation can even begin.
            let mut per_node: BTreeMap<&str, usize> = BTreeMap::new();
            for pod in &covered {
                if let Some(node) = pod.node_name.as_deref() {
                    *per_node.entry(node).or_insert(0) += 1;
                }
            }
            let worst_node = per_node.iter().max_by_key(|(_, count)| **count);

            let mut affected: Vec<_> = vec![pdb.resource_ref()];
            affected.extend(covered.iter().map(|p| p.resource_ref()));

            if pdb.blocks_disruption() {
                findings.push(
                    FindingBuilder::new(
                        "FF-PDB-001",
                        Severity::Blocker,
                        Confidence::Certain,
                        format!("PodDisruptionBudget {namespace}/{name} permits no disruption"),
                    )
                    .affecting(affected)
                    .evidence(
                        pdb.resource_ref(),
                        ".status.disruptionsAllowed",
                        pdb.disruptions_allowed,
                        Some("the eviction API will reject every eviction while this is 0"),
                    )
                    .evidence(
                        pdb.resource_ref(),
                        ".status.currentHealthy",
                        pdb.current_healthy,
                        None,
                    )
                    .evidence(
                        pdb.resource_ref(),
                        ".status.desiredHealthy",
                        pdb.desired_healthy,
                        None,
                    )
                    .evidence(
                        pdb.resource_ref(),
                        ".spec.minAvailable",
                        pdb.min_available.as_deref().unwrap_or("(unset)"),
                        Some("as declared by the operator"),
                    )
                    .calculation(
                        vec![
                            ("currentHealthy".into(), pdb.current_healthy.to_string()),
                            ("desiredHealthy".into(), pdb.desired_healthy.to_string()),
                        ],
                        "disruptionsAllowed = currentHealthy - desiredHealthy",
                        pdb.disruptions_allowed,
                        Some("pods"),
                    )
                    .explaining(format!(
                        "{} of this budget's pods sit on the selected nodes, but the API server \
                         currently allows 0 voluntary disruptions. currentHealthy ({}) equals \
                         desiredHealthy ({}), so evicting any covered pod would drop the workload \
                         below its declared minimum. Every eviction will be rejected with 429 \
                         until this changes.",
                        covered.len(),
                        pdb.current_healthy,
                        pdb.desired_healthy,
                    ))
                    .remediation(
                        "Raise the replica count so healthy pods exceed the minimum",
                        Some(format!(
                            "kubectl scale deploy/<name> -n {namespace} --replicas={}",
                            pdb.desired_healthy.saturating_add(1)
                        )),
                        Some("costs additional capacity for the duration of the maintenance"),
                    )
                    .remediation(
                        "Relax the budget",
                        Some(format!(
                            "kubectl patch pdb {name} -n {namespace} --type=merge \
                             -p '{{\"spec\":{{\"minAvailable\":{}}}}}'",
                            pdb.desired_healthy.saturating_sub(1).max(0)
                        )),
                        Some(
                            "reduces the availability guarantee the workload owner asked for; \
                             their call, not the maintenance operator's",
                        ),
                    )
                    .limitation(
                        "Reflects disruptionsAllowed at the moment of the snapshot. A rollout \
                         finishing or a pod becoming ready can change it without any action here."
                            .to_owned(),
                    )
                    .limitation(
                        "Covers voluntary disruption via the eviction API only. A node that fails \
                         or is deleted outright bypasses PodDisruptionBudgets entirely."
                            .to_owned(),
                    )
                    .build(cluster, snapshot_id, now),
                );
                continue;
            }

            // The budget allows some disruption. It can still block if one node
            // holds more covered pods than the budget permits to go at once.
            if let Some((node, count)) = worst_node {
                let count = i32::try_from(*count).unwrap_or(i32::MAX);
                if count > pdb.disruptions_allowed {
                    findings.push(
                        FindingBuilder::new(
                            "FF-PDB-002",
                            Severity::Blocker,
                            Confidence::Certain,
                            format!(
                                "Node {node} holds more {namespace}/{name} pods than the budget \
                                 allows to be disrupted at once"
                            ),
                        )
                        .affecting(affected)
                        .evidence(
                            pdb.resource_ref(),
                            ".status.disruptionsAllowed",
                            pdb.disruptions_allowed,
                            None,
                        )
                        .calculation(
                            vec![
                                ("pods covered on this node".into(), count.to_string()),
                                (
                                    "disruptionsAllowed".into(),
                                    pdb.disruptions_allowed.to_string(),
                                ),
                            ],
                            "blocked when podsOnNode > disruptionsAllowed",
                            format!("{count} > {}", pdb.disruptions_allowed),
                            Some("pods"),
                        )
                        .explaining(format!(
                            "Draining {node} evicts {count} pods covered by {namespace}/{name}, \
                             but the budget permits only {} voluntary disruption(s) at a time. A \
                             drain evicts a node's pods together, so it will stall partway \
                             through rather than fail cleanly at the start.",
                            pdb.disruptions_allowed
                        ))
                        .remediation(
                            "Spread the workload so no single node holds more covered pods than \
                             the budget allows",
                            None,
                            Some("may require a topology-spread constraint or anti-affinity"),
                        )
                        .limitation(
                            "Assumes the drain evicts this node's pods concurrently, which is \
                             kubectl's behaviour. A tool evicting strictly one at a time and \
                             waiting would eventually succeed."
                                .to_owned(),
                        )
                        .limitation(
                            "disruptionsAllowed can recover between evictions as replacement pods \
                             become ready; this does not model that recovery."
                                .to_owned(),
                        )
                        .build(cluster, snapshot_id, now),
                    );
                    continue;
                }
            }

            // Not blocking, but the margin is worth stating: it is what limits
            // how many nodes may be taken at once.
            findings.push(
                FindingBuilder::new(
                    "FF-PDB-003",
                    Severity::Info,
                    Confidence::Certain,
                    format!("PodDisruptionBudget {namespace}/{name} permits this disruption"),
                )
                .affecting(affected)
                .evidence(
                    pdb.resource_ref(),
                    ".status.disruptionsAllowed",
                    pdb.disruptions_allowed,
                    Some("headroom for voluntary disruption"),
                )
                .calculation(
                    vec![
                        (
                            "covered pods on selected nodes".into(),
                            covered.len().to_string(),
                        ),
                        (
                            "disruptionsAllowed".into(),
                            pdb.disruptions_allowed.to_string(),
                        ),
                        (
                            "worst single node".into(),
                            worst_node.map_or("0".to_owned(), |(n, c)| format!("{n} holds {c}")),
                        ),
                    ],
                    "permitted while podsOnAnyNode <= disruptionsAllowed",
                    "permitted",
                    None,
                )
                .explaining(format!(
                    "{} covered pod(s) sit on the selected nodes and the budget allows {} \
                     voluntary disruption(s). This is the constraint that limits how many nodes \
                     may be taken at once.",
                    covered.len(),
                    pdb.disruptions_allowed
                ))
                .limitation(
                    "A snapshot-time reading. Headroom shrinks if other maintenance or a rollout \
                     consumes it before this operation runs."
                        .to_owned(),
                )
                .build(cluster, snapshot_id, now),
            );
        }

        Ok(findings)
    }
}
