//! The cluster-level placement precondition.
//!
//! Exists because of what a real cluster did. During a Brupop update two of
//! three nodes were cordoned, so selecting the third left **no** schedulable
//! node at all — and every per-pod placement analyzer fired for every pod: 12
//! toleration findings, 6 affinity, 3 selector, all restating the same single
//! fact in 21 different ways.
//!
//! Each of those findings was true. Together they were useless: an operator
//! scrolling 34 findings to discover "there is nowhere to put anything" is
//! worse served than by one sentence.
//!
//! So this runs first, and the per-pod analyzers stand down when it fires.

use ff_core::{Confidence, Finding, Result, Severity};

use crate::analyzer::{Analyzer, FindingBuilder};
use crate::context::AnalysisContext;

/// Detects that no node would remain able to accept a rescheduled pod.
pub struct NoCandidateNodesAnalyzer;

impl Analyzer for NoCandidateNodesAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-NODES"
    }

    fn describes(&self) -> &'static str {
        "whether any node would remain that can accept a rescheduled pod"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Node", "Pod"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();

        if ctx.selected.is_empty() || !ctx.candidate_nodes().is_empty() {
            return Ok(Vec::new());
        }

        let needs_rescheduling = ctx.rescheduling_pods();
        if needs_rescheduling.is_empty() {
            // Nowhere to put anything, but nothing needs putting anywhere.
            return Ok(Vec::new());
        }

        // Why each remaining node is unusable — the actionable part.
        let mut cordoned = Vec::new();
        let mut not_ready = Vec::new();
        for node in &ctx.remaining {
            if node.unschedulable {
                cordoned.push(node.name.clone());
            } else if !node.is_ready() {
                not_ready.push(node.name.clone());
            }
        }

        let mut builder = FindingBuilder::new(
            "FF-NODES-001",
            Severity::Blocker,
            Confidence::Certain,
            "No node would remain that can accept a rescheduled pod",
        )
        .affecting(ctx.selected.iter().map(|n| n.resource_ref()))
        .calculation(
            vec![
                (
                    "nodes in cluster".into(),
                    ctx.snapshot.nodes().len().to_string(),
                ),
                (
                    "selected for maintenance".into(),
                    ctx.selected.len().to_string(),
                ),
                ("remaining".into(), ctx.remaining.len().to_string()),
                ("of those, cordoned".into(), cordoned.len().to_string()),
                ("of those, not Ready".into(), not_ready.len().to_string()),
                ("schedulable remaining".into(), "0".into()),
            ],
            "schedulable = remaining - cordoned - notReady",
            "0 nodes",
            None,
        )
        .explaining(format!(
            "{} pod(s) would need to be rescheduled, and no remaining node can accept them. Of \
             the {} node(s) that would remain, {} are cordoned and {} are not Ready. Every pod \
             would stay Pending.\n\nIf maintenance is already in progress elsewhere — a Brupop \
             update cordons each node it works on — this is the expected answer, and the remedy \
             is to wait for it to finish rather than to change anything.",
            needs_rescheduling.len(),
            ctx.remaining.len(),
            cordoned.len(),
            not_ready.len(),
        ))
        .remediation(
            "Wait for in-flight maintenance to complete and the cordoned nodes to return",
            Some("kubectl get nodes".to_owned()),
            Some("the wait is the fix; nothing needs changing"),
        )
        .remediation(
            "Uncordon a node, if its maintenance is genuinely finished",
            Some("kubectl uncordon <node>".to_owned()),
            Some("uncordoning a node still being updated will schedule pods onto it mid-reboot"),
        )
        .limitation(
            "Reports that nowhere is available, not why each individual pod could not be placed. \
             The per-pod placement checks stand down when this fires, because with zero candidate \
             nodes they would each report the same fact once per pod."
                .to_owned(),
        )
        .limitation(
            "Cluster autoscaling is not modelled. A cluster that would add a node on demand may \
             resolve this without intervention."
                .to_owned(),
        );

        for name in cordoned.iter().chain(not_ready.iter()) {
            if let Some(node) = ctx.remaining.iter().find(|n| &n.name == name) {
                builder = builder.evidence(
                    node.resource_ref(),
                    ".spec.unschedulable",
                    node.unschedulable,
                    Some(if node.unschedulable {
                        "cordoned — accepts no new pods"
                    } else {
                        "not Ready"
                    }),
                );
            }
        }

        Ok(vec![builder.build(cluster, snapshot_id, now)])
    }
}
