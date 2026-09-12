//! Availability-zone and multi-node analysis.
//!
//! `Heuristic`. This reasons about where a workload's copies sit, which is real
//! and checkable — but whether concentration matters depends on what the
//! workload is, which cluster state does not record.

use std::collections::{BTreeMap, BTreeSet};

use ff_core::{Confidence, Finding, Result, Severity};

use crate::analyzer::{Analyzer, FindingBuilder};
use crate::context::AnalysisContext;

/// Detects maintenance that removes a whole availability zone, or all copies of
/// a workload at once.
pub struct AvailabilityZoneAnalyzer;

impl Analyzer for AvailabilityZoneAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-AZ"
    }

    fn describes(&self) -> &'static str {
        "availability-zone concentration and simultaneous removal of every replica"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Node", "Pod", "Deployment", "StatefulSet"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();
        let mut findings = Vec::new();

        if ctx.selected.is_empty() {
            return Ok(findings);
        }

        // --- Whole-zone removal ---------------------------------------------
        let zone_of = |name: &str| {
            ctx.snapshot
                .nodes()
                .iter()
                .find(|n| n.name == name)
                .and_then(|n| n.availability_zone.clone())
        };

        let mut nodes_per_zone: BTreeMap<String, usize> = BTreeMap::new();
        for node in ctx.snapshot.nodes() {
            if let Some(zone) = &node.availability_zone {
                *nodes_per_zone.entry(zone.clone()).or_insert(0) += 1;
            }
        }
        let mut selected_per_zone: BTreeMap<String, usize> = BTreeMap::new();
        for node in &ctx.selected {
            if let Some(zone) = &node.availability_zone {
                *selected_per_zone.entry(zone.clone()).or_insert(0) += 1;
            }
        }

        for (zone, selected_count) in &selected_per_zone {
            let total = nodes_per_zone.get(zone).copied().unwrap_or(0);
            if total > 0 && *selected_count == total && nodes_per_zone.len() > 1 {
                findings.push(
                    FindingBuilder::new(
                        "FF-AZ-001",
                        Severity::High,
                        Confidence::Heuristic,
                        format!("This maintenance removes every node in zone {zone}"),
                    )
                    .affecting(ctx.selected.iter().map(|n| n.resource_ref()))
                    .calculation(
                        vec![
                            (format!("nodes in {zone}"), total.to_string()),
                            ("of those selected".into(), selected_count.to_string()),
                        ],
                        "flagged when selectedInZone == totalInZone and other zones exist",
                        format!("{selected_count}/{total}"),
                        Some("nodes"),
                    )
                    .explaining(format!(
                        "All {total} node(s) in zone {zone} are selected, so the zone leaves \
                         service entirely for the duration. Any workload whose replicas all sit in \
                         this zone loses every copy at once, and topology-spread constraints that \
                         require a pod per zone become unsatisfiable."
                    ))
                    .remediation(
                        "Take the zone's nodes in more than one wave",
                        None,
                        Some("the maintenance takes longer"),
                    )
                    .limitation(
                        "Whether losing a zone matters depends on the workloads in it. This check \
                         reports the topology fact, not its consequence."
                            .to_owned(),
                    )
                    .limitation(
                        "Zone is read from the standard topology label. A node without that label \
                         is invisible to this check."
                            .to_owned(),
                    )
                    .build(cluster, snapshot_id, now),
                );
            }
        }

        // --- Every replica of a workload at once -----------------------------
        let mut workload_pods: BTreeMap<(String, String), (usize, usize, BTreeSet<String>)> =
            BTreeMap::new();
        for pod in ctx
            .snapshot
            .pods()
            .iter()
            .filter(|p| p.phase.occupies_capacity())
        {
            let Some(workload) = ctx.workload_for(pod) else {
                continue;
            };
            if workload.kind == ff_core::WorkloadKind::DaemonSet {
                continue;
            }
            let key = (workload.namespace.clone(), workload.name.clone());
            let entry = workload_pods.entry(key).or_insert((0, 0, BTreeSet::new()));
            entry.0 += 1;
            if let Some(node) = pod.node_name.as_deref() {
                if ctx.selected.iter().any(|s| s.name == node) {
                    entry.1 += 1;
                }
                if let Some(zone) = zone_of(node) {
                    entry.2.insert(zone);
                }
            }
        }

        for ((namespace, name), (total, affected, zones)) in workload_pods {
            if total == 0 || affected < total || total == 1 {
                // Singletons are reported by their own analyzer, with a better
                // explanation than "all replicas affected" would give.
                continue;
            }
            findings.push(
                FindingBuilder::new(
                    "FF-AZ-002",
                    Severity::Blocker,
                    Confidence::Heuristic,
                    format!("Every replica of {namespace}/{name} is on a selected node"),
                )
                .calculation(
                    vec![
                        ("running replicas".into(), total.to_string()),
                        ("on selected nodes".into(), affected.to_string()),
                        (
                            "zones spanned".into(),
                            if zones.is_empty() {
                                "(unlabelled)".to_owned()
                            } else {
                                zones.iter().cloned().collect::<Vec<_>>().join(", ")
                            },
                        ),
                    ],
                    "blocked when affectedReplicas == totalReplicas",
                    format!("{affected}/{total}"),
                    Some("replicas"),
                )
                .explaining(format!(
                    "All {total} running replicas of this workload sit on nodes selected for \
                     maintenance. Taking those nodes together means the workload has no serving \
                     copy until replacements schedule and become ready, regardless of what its \
                     PodDisruptionBudget allows over time."
                ))
                .remediation(
                    "Split the nodes across waves so some replicas keep serving",
                    None,
                    Some("the maintenance takes longer"),
                )
                .limitation(
                    "Counts replicas by label-selector match, so a workload sharing labels with \
                     another may be counted imprecisely."
                        .to_owned(),
                )
                .limitation(
                    "Does not model a rolling drain. Taking the nodes one at a time, waiting for \
                     replacements between each, may avoid this entirely — wave planning is out of \
                     scope at this milestone (ADR-0011)."
                        .to_owned(),
                )
                .build(cluster, snapshot_id, now),
            );
        }

        Ok(findings)
    }
}
