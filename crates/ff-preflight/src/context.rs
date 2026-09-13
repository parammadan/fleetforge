//! The precomputed view every analyzer works from.
//!
//! Built once per preflight run so ten analyzers do not each re-derive "which
//! pods are on the selected nodes". More importantly, it puts the *definitions*
//! in one place: what counts as evicted, what counts as reschedulable, and what
//! capacity remains are each decided once rather than ten slightly different
//! ways.

use ff_core::{
    Bytes, ClusterSnapshot, MaintenanceRequest, Millicores, NodeFact, PdbFact, PodFact,
    WorkloadFact, WorkloadKind,
};

/// Everything an analyzer needs about a proposed maintenance operation.
///
/// # What "selected" means
///
/// An operator names a set of nodes and a concurrency. Those are different
/// questions: *which* nodes eventually get maintained, and *how many* are out
/// of service at the same moment. Only the second determines whether anything
/// breaks — taking six nodes one at a time is a different operation from taking
/// six at once.
///
/// So preflight analyzes a **wave**: the `concurrency` nodes from the request
/// that would hurt most if taken together. [`requested`](Self::requested) holds
/// everything the operator named; [`selected`](Self::selected) holds the wave
/// under analysis; [`remaining`](Self::remaining) is everything else, including
/// requested nodes scheduled for a later wave.
///
/// This is what makes the interface's concurrency control meaningful rather
/// than decorative: the same node selection can be blocked at concurrency 3 and
/// safe at concurrency 1, and that is a true statement about the cluster.
pub struct AnalysisContext<'a> {
    /// The snapshot being analyzed.
    pub snapshot: &'a ClusterSnapshot,
    /// What was asked.
    pub request: &'a MaintenanceRequest,
    /// Every node the operator named that exists in the snapshot.
    pub requested: Vec<&'a NodeFact>,
    /// The wave under analysis: nodes assumed out of service simultaneously.
    pub selected: Vec<&'a NodeFact>,
    /// Node names that were requested but are not in the snapshot.
    ///
    /// Not silently dropped: a request naming a node FleetForge cannot see is a
    /// finding in itself, not an empty set to analyze around.
    pub unknown_nodes: Vec<String>,
    /// Nodes in service during this wave.
    pub remaining: Vec<&'a NodeFact>,
}

impl<'a> AnalysisContext<'a> {
    /// Build the context for the request's own concurrency.
    #[must_use]
    pub fn new(snapshot: &'a ClusterSnapshot, request: &'a MaintenanceRequest) -> Self {
        Self::with_concurrency(snapshot, request, request.desired_concurrency)
    }

    /// Build the context for a specific wave size.
    ///
    /// Used by the engine to search for the largest safe concurrency.
    #[must_use]
    pub fn with_concurrency(
        snapshot: &'a ClusterSnapshot,
        request: &'a MaintenanceRequest,
        concurrency: u32,
    ) -> Self {
        let mut requested = Vec::new();
        let mut unknown_nodes = Vec::new();
        for name in &request.node_names {
            match snapshot.nodes().iter().find(|n| &n.name == name) {
                Some(node) => requested.push(node),
                None => unknown_nodes.push(name.clone()),
            }
        }

        // Rank by how much work a node's removal creates, so a smaller wave is
        // still analyzed against its worst case rather than a lucky subset.
        // Name is the tie-break, which is what keeps the result deterministic
        // for a given snapshot (ADR-0016).
        let mut ranked = requested.clone();
        ranked.sort_by(|a, b| {
            let impact = |node: &NodeFact| {
                snapshot
                    .pods()
                    .iter()
                    .filter(|p| {
                        p.phase.occupies_capacity()
                            && p.node_name.as_deref() == Some(node.name.as_str())
                    })
                    .count()
            };
            impact(b).cmp(&impact(a)).then_with(|| a.name.cmp(&b.name))
        });

        let wave_size = if concurrency == 0 {
            ranked.len()
        } else {
            (concurrency as usize).min(ranked.len())
        };
        let selected: Vec<&'a NodeFact> = ranked.into_iter().take(wave_size).collect();
        let selected_names: Vec<&str> = selected.iter().map(|n| n.name.as_str()).collect();

        let remaining = snapshot
            .nodes()
            .iter()
            .filter(|n| !selected_names.contains(&n.name.as_str()))
            .collect();

        Self {
            snapshot,
            request,
            requested,
            selected,
            unknown_nodes,
            remaining,
        }
    }

    /// Pods that would be evicted: everything occupying capacity on a selected
    /// node.
    #[must_use]
    pub fn evicted_pods(&self) -> Vec<&'a PodFact> {
        self.snapshot
            .pods()
            .iter()
            .filter(|p| {
                p.phase.occupies_capacity()
                    && p.node_name
                        .as_ref()
                        .is_some_and(|n| self.selected.iter().any(|s| &s.name == n))
            })
            .collect()
    }

    /// Evicted pods that need to land somewhere else.
    ///
    /// DaemonSet pods are excluded. They leave with the node and return with
    /// it; counting them as needing capacity elsewhere would overstate what the
    /// drain requires and produce capacity findings that are simply wrong.
    #[must_use]
    pub fn rescheduling_pods(&self) -> Vec<&'a PodFact> {
        self.evicted_pods()
            .into_iter()
            .filter(|p| !self.is_daemonset_pod(p))
            .collect()
    }

    /// Whether a pod belongs to a DaemonSet.
    #[must_use]
    pub fn is_daemonset_pod(&self, pod: &PodFact) -> bool {
        pod.owners.iter().any(|owner| owner.kind == "DaemonSet")
            || self
                .workload_for(pod)
                .is_some_and(|w| w.kind == WorkloadKind::DaemonSet)
    }

    /// The workload controlling a pod, resolving a ReplicaSet to its Deployment
    /// where possible.
    ///
    /// Matching is by label selector within the namespace, because a pod's
    /// owner chain stops at the ReplicaSet and the operator thinks in
    /// Deployments.
    #[must_use]
    pub fn workload_for(&self, pod: &PodFact) -> Option<&'a WorkloadFact> {
        // Prefer an exact owner match.
        let direct = pod.controller().and_then(|controller| {
            self.snapshot.workloads().iter().find(|w| {
                w.namespace == pod.namespace
                    && w.name == controller.name
                    && format!("{:?}", w.kind) == controller.kind
            })
        });
        if let Some(direct) = direct {
            // A ReplicaSet is an implementation detail of a Deployment, and the
            // operator thinks in Deployments.
            if direct.kind == WorkloadKind::ReplicaSet {
                let deployment = self.snapshot.workloads().iter().find(|w| {
                    w.kind == WorkloadKind::Deployment
                        && w.namespace == pod.namespace
                        && w.selector.matches(&pod.labels)
                });
                if let Some(deployment) = deployment {
                    return Some(deployment);
                }
            }
            return Some(direct);
        }
        self.snapshot
            .workloads()
            .iter()
            .find(|w| w.namespace == pod.namespace && w.selector.matches(&pod.labels))
    }

    /// Nodes that would remain in service and can accept new pods.
    ///
    /// Cordoned and not-Ready nodes are excluded: they keep their existing pods
    /// but accept no new ones, so counting them would overstate where evicted
    /// pods could land.
    #[must_use]
    pub fn candidate_nodes(&self) -> Vec<&'a NodeFact> {
        self.remaining
            .iter()
            .filter(|n| !n.unschedulable && n.is_ready())
            .copied()
            .collect()
    }

    /// PodDisruptionBudgets covering a pod.
    #[must_use]
    pub fn pdbs_for(&self, pod: &PodFact) -> Vec<&'a PdbFact> {
        self.snapshot
            .pdbs()
            .iter()
            .filter(|pdb| pdb.namespace == pod.namespace && pdb.selector.matches(&pod.labels))
            .collect()
    }

    /// Total CPU requested by pods that must be rescheduled.
    #[must_use]
    pub fn cpu_to_reschedule(&self) -> Millicores {
        self.rescheduling_pods()
            .iter()
            .fold(Millicores::ZERO, |acc, p| {
                acc.saturating_add(p.total_cpu_request())
            })
    }

    /// Total memory requested by pods that must be rescheduled.
    #[must_use]
    pub fn memory_to_reschedule(&self) -> Bytes {
        self.rescheduling_pods().iter().fold(Bytes::ZERO, |acc, p| {
            acc.saturating_add(p.total_memory_request())
        })
    }

    /// Allocatable CPU on the nodes that remain, excluding cordoned nodes.
    ///
    /// A cordoned node keeps its pods but accepts no new ones, so counting its
    /// allocatable capacity as available headroom would overstate what the
    /// cluster can absorb.
    #[must_use]
    pub fn remaining_allocatable_cpu(&self) -> Millicores {
        self.remaining
            .iter()
            .filter(|n| !n.unschedulable && n.is_ready())
            .fold(Millicores::ZERO, |acc, n| {
                acc.saturating_add(n.allocatable_cpu)
            })
    }

    /// Allocatable memory on the nodes that remain, excluding cordoned nodes.
    #[must_use]
    pub fn remaining_allocatable_memory(&self) -> Bytes {
        self.remaining
            .iter()
            .filter(|n| !n.unschedulable && n.is_ready())
            .fold(Bytes::ZERO, |acc, n| {
                acc.saturating_add(n.allocatable_memory)
            })
    }

    /// CPU already requested by pods staying on the remaining nodes.
    #[must_use]
    pub fn cpu_already_used_on_remaining(&self) -> Millicores {
        self.snapshot
            .pods()
            .iter()
            .filter(|p| {
                p.phase.occupies_capacity()
                    && p.node_name
                        .as_ref()
                        .is_some_and(|n| self.remaining.iter().any(|r| &r.name == n))
            })
            .fold(Millicores::ZERO, |acc, p| {
                acc.saturating_add(p.total_cpu_request())
            })
    }

    /// Memory already requested by pods staying on the remaining nodes.
    #[must_use]
    pub fn memory_already_used_on_remaining(&self) -> Bytes {
        self.snapshot
            .pods()
            .iter()
            .filter(|p| {
                p.phase.occupies_capacity()
                    && p.node_name
                        .as_ref()
                        .is_some_and(|n| self.remaining.iter().any(|r| &r.name == n))
            })
            .fold(Bytes::ZERO, |acc, p| {
                acc.saturating_add(p.total_memory_request())
            })
    }
}
