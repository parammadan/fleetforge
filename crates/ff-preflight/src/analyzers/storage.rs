//! Node-bound storage analysis.
//!
//! `Certain`, because this is not a capacity question. A pod with a `hostPath`,
//! a `local` PersistentVolume, or an `emptyDir` holding state is tied to one
//! machine's filesystem. Evicting it does not move the data, and the
//! replacement pod either cannot schedule or starts without it.

use ff_core::{Confidence, Finding, Result, Severity, VolumeKind};

use crate::analyzer::{Analyzer, FindingBuilder};
use crate::context::AnalysisContext;

/// Detects pods pinned to a node by their storage.
pub struct NodeBoundStorageAnalyzer;

impl Analyzer for NodeBoundStorageAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-STORAGE"
    }

    fn describes(&self) -> &'static str {
        "pods whose volumes tie them to a specific node"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Pod"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();
        let mut findings = Vec::new();

        for pod in ctx.rescheduling_pods() {
            let binding: Vec<_> = pod
                .volumes
                .iter()
                .filter(|v| v.kind.pins_to_node())
                .collect();
            if binding.is_empty() {
                continue;
            }

            // emptyDir alone is a weaker signal than hostPath or a local PV:
            // it is scratch space for many workloads and irreplaceable state
            // for a few, and cluster state cannot tell which.
            let only_empty_dir = binding.iter().all(|v| v.kind == VolumeKind::EmptyDir);
            let severity = if only_empty_dir {
                Severity::Medium
            } else {
                Severity::Blocker
            };

            let mut builder = FindingBuilder::new(
                if only_empty_dir {
                    "FF-STORAGE-002"
                } else {
                    "FF-STORAGE-001"
                },
                severity,
                Confidence::Certain,
                format!(
                    "Pod {}/{} is bound to node {} by its storage",
                    pod.namespace,
                    pod.name,
                    pod.node_name.as_deref().unwrap_or("(unscheduled)")
                ),
            )
            .affecting([pod.resource_ref()]);

            for volume in &binding {
                builder = builder.evidence(
                    pod.resource_ref(),
                    &format!(".spec.volumes[name={}]", volume.name),
                    format!("{:?}", volume.kind),
                    Some(match volume.kind {
                        VolumeKind::HostPath => "a path on this node's filesystem",
                        VolumeKind::LocalPersistentVolume => {
                            "a local PersistentVolume, tied to this node"
                        }
                        VolumeKind::EmptyDir => "created empty on whichever node the pod lands on",
                        _ => "node-bound",
                    }),
                );
            }

            builder = if only_empty_dir {
                builder
                    .explaining(
                        "This pod uses emptyDir, which is created fresh on whichever node the pod \
                         lands on. The pod can move; its data cannot. Whether that matters depends \
                         on what the volume holds — scratch space is fine, a cache is a warm-up \
                         cost, and anything durable is data loss. Cluster state cannot tell which."
                            .to_owned(),
                    )
                    .remediation(
                        "Confirm with the workload owner that the volume holds nothing durable",
                        None,
                        None,
                    )
                    .limitation(
                        "Cannot distinguish scratch space from durable state. This is flagged for \
                         a human to judge, not as a defect."
                            .to_owned(),
                    )
            } else {
                builder
                    .explaining(format!(
                        "This pod mounts storage that exists only on {}. Evicting it does not move \
                         the data: the replacement either fails to schedule, because the volume's \
                         node affinity pins it to a node being drained, or starts without the data \
                         it expects. Draining is not a safe operation for this pod without first \
                         migrating or accepting the loss.",
                        pod.node_name.as_deref().unwrap_or("this node")
                    ))
                    .remediation(
                        "Migrate the data, or exclude this node from the maintenance",
                        None,
                        Some("migration needs a maintenance window of its own"),
                    )
                    .remediation(
                        "Confirm the data is reproducible and can be discarded",
                        None,
                        Some("irreversible if that judgement is wrong"),
                    )
                    .limitation(
                        "Detects node-bound storage from the pod spec alone. PersistentVolume node \
                         affinity is not read, because FleetForge has no PersistentVolume read \
                         permission at this milestone — so a network PV that is in fact zone- or \
                         node-bound is not flagged here."
                            .to_owned(),
                    )
            };

            builder = builder.limitation(
                "Volume types this version does not recognise are treated as movable. That is the \
                 optimistic direction, and it means this check can miss a binding rather than \
                 invent one."
                    .to_owned(),
            );

            findings.push(builder.build(cluster, snapshot_id, now));
        }

        Ok(findings)
    }
}
