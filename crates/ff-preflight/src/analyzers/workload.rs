//! Workload-shape analysis: singletons and unmanaged pods.
//!
//! Both are `Certain` because neither is a prediction. A one-replica workload
//! has no second copy; an ownerless pod has no controller. Those are facts
//! about the objects, not guesses about the scheduler.

use std::collections::BTreeSet;

use ff_core::{Confidence, Finding, Result, Severity};

use crate::analyzer::{Analyzer, FindingBuilder};
use crate::context::AnalysisContext;

/// Detects workloads with a single replica on the selected nodes.
pub struct SingletonAnalyzer;

impl Analyzer for SingletonAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-SINGLETON"
    }

    fn describes(&self) -> &'static str {
        "workloads with one replica, which have no redundancy during eviction"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Pod", "Deployment", "StatefulSet", "ReplicaSet"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();
        let mut seen = BTreeSet::new();
        let mut findings = Vec::new();

        for pod in ctx.rescheduling_pods() {
            let Some(workload) = ctx.workload_for(pod) else {
                continue;
            };
            if !workload.is_singleton() {
                continue;
            }
            if !seen.insert((workload.namespace.clone(), workload.name.clone())) {
                continue;
            }

            let covered_by_pdb = !ctx.pdbs_for(pod).is_empty();

            findings.push(
                FindingBuilder::new(
                    "FF-SINGLETON-001",
                    Severity::High,
                    Confidence::Certain,
                    format!(
                        "{:?} {}/{} has a single replica",
                        workload.kind, workload.namespace, workload.name
                    ),
                )
                .affecting([workload.resource_ref(), pod.resource_ref()])
                .evidence(
                    workload.resource_ref(),
                    ".spec.replicas",
                    workload.desired_replicas.unwrap_or(0),
                    Some("no second copy exists to serve during the eviction"),
                )
                .evidence(
                    pod.resource_ref(),
                    ".spec.nodeName",
                    pod.node_name.as_deref().unwrap_or("(unscheduled)"),
                    Some("on a node selected for maintenance"),
                )
                .evidence(
                    pod.resource_ref(),
                    ".spec.terminationGracePeriodSeconds",
                    pod.termination_grace_period_seconds.unwrap_or(30),
                    Some("minimum outage before a replacement can even begin"),
                )
                .explaining(format!(
                    "Evicting this pod takes the workload to zero replicas until a replacement is \
                     scheduled and becomes ready. Termination alone allows up to {}s before the \
                     pod stops, and startup time is additional. A PodDisruptionBudget cannot \
                     prevent this — with one replica, any budget that permits a disruption \
                     permits a full outage.{}",
                    pod.termination_grace_period_seconds.unwrap_or(30),
                    if covered_by_pdb {
                        " A budget does cover this pod, which will gate the eviction but cannot \
                         create redundancy that does not exist."
                    } else {
                        " No PodDisruptionBudget covers this pod, so nothing will gate the \
                         eviction at all."
                    }
                ))
                .remediation(
                    "Scale to at least two replicas before the maintenance",
                    Some(format!(
                        "kubectl scale {}/{} -n {} --replicas=2",
                        format!("{:?}", workload.kind).to_lowercase(),
                        workload.name,
                        workload.namespace
                    )),
                    Some("requires the workload to tolerate running more than one copy"),
                )
                .remediation(
                    "Accept the outage and schedule the maintenance for a low-traffic window",
                    None,
                    Some("the outage still happens; it is only better timed"),
                )
                .limitation(
                    "Does not measure how long rescheduling actually takes. The outage length \
                     depends on image pull, startup, and readiness probes, none of which are \
                     visible in cluster state."
                        .to_owned(),
                )
                .limitation(
                    "A single replica may be intentional and harmless — a batch worker, a \
                     leader-elected component with a passive standby elsewhere. This check sees \
                     the replica count, not the intent."
                        .to_owned(),
                )
                .build(cluster, snapshot_id, now),
            );
        }

        Ok(findings)
    }
}

/// Detects pods with no controller.
pub struct UnmanagedPodAnalyzer;

impl Analyzer for UnmanagedPodAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-UNMANAGED"
    }

    fn describes(&self) -> &'static str {
        "pods with no owner, which nothing will recreate after eviction"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Pod"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();
        let mut findings = Vec::new();

        for pod in ctx.evicted_pods() {
            if !pod.is_unmanaged() {
                continue;
            }
            // Static pods are mirror pods for control-plane components. They
            // are recreated by the kubelet from a manifest on disk, so they are
            // ownerless without being lost — reporting them as unmanaged would
            // be a false alarm on every control-plane node.
            let is_static = pod.labels.get("component").is_some_and(|c| {
                matches!(
                    c.as_str(),
                    "etcd" | "kube-apiserver" | "kube-controller-manager" | "kube-scheduler"
                )
            }) || pod.namespace == "kube-system"
                && pod.name.ends_with(pod.node_name.as_deref().unwrap_or("\0"));

            if is_static {
                continue;
            }

            findings.push(
                FindingBuilder::new(
                    "FF-UNMANAGED-001",
                    Severity::Blocker,
                    Confidence::Certain,
                    format!(
                        "Pod {}/{} has no controller and will not be recreated",
                        pod.namespace, pod.name
                    ),
                )
                .affecting([pod.resource_ref()])
                .evidence(
                    pod.resource_ref(),
                    ".metadata.ownerReferences",
                    "(empty)",
                    Some("no Deployment, StatefulSet, DaemonSet, or Job owns this pod"),
                )
                .evidence(
                    pod.resource_ref(),
                    ".spec.nodeName",
                    pod.node_name.as_deref().unwrap_or("(unscheduled)"),
                    Some("on a node selected for maintenance"),
                )
                .explaining(
                    "This pod was created directly rather than by a controller. Evicting it \
                     deletes it permanently — nothing exists to schedule a replacement. Whatever \
                     it was doing stops, and the only way back is for a human to recreate it."
                        .to_owned(),
                )
                .remediation(
                    "Confirm with the owner whether this pod is still needed",
                    None,
                    None,
                )
                .remediation(
                    "Recreate it under a controller before the maintenance",
                    None,
                    Some("requires knowing the pod's intended lifecycle"),
                )
                .limitation(
                    "Static pods managed by the kubelet from on-disk manifests are excluded by \
                     label and naming convention. A static pod not matching those conventions \
                     would be reported here incorrectly."
                        .to_owned(),
                )
                .limitation(
                    "A deliberately one-shot debugging pod is indistinguishable from a lost \
                     production workload in cluster state alone."
                        .to_owned(),
                )
                .build(cluster, snapshot_id, now),
            );
        }

        Ok(findings)
    }
}
