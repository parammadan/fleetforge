//! Workload controller normalization.
//!
//! Deployments, StatefulSets, DaemonSets, and ReplicaSets collapse into one
//! `WorkloadFact` distinguished by `WorkloadKind`, because analyzers care about
//! two things: how many replicas there are, and whether evicting a pod causes
//! it to be rescheduled somewhere else. A DaemonSet answers "no" to the second,
//! and that single distinction changes the capacity arithmetic for a drain.

use ff_core::{WorkloadFact, WorkloadKind};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};

use super::{NormalizeContext, label_selector};

/// Normalize a Deployment.
#[must_use]
pub fn deployment(d: &Deployment, ctx: &NormalizeContext) -> WorkloadFact {
    WorkloadFact {
        provenance: ctx.provenance("apps", "v1", "Deployment", &d.metadata),
        kind: WorkloadKind::Deployment,
        namespace: d.metadata.namespace.clone().unwrap_or_default(),
        name: d.metadata.name.clone().unwrap_or_default(),
        desired_replicas: d.spec.as_ref().and_then(|s| s.replicas),
        ready_replicas: d.status.as_ref().and_then(|s| s.ready_replicas),
        selector: d
            .spec
            .as_ref()
            .map(|s| label_selector(Some(&s.selector)))
            .unwrap_or_default(),
    }
}

/// Normalize a StatefulSet.
#[must_use]
pub fn stateful_set(s: &StatefulSet, ctx: &NormalizeContext) -> WorkloadFact {
    WorkloadFact {
        provenance: ctx.provenance("apps", "v1", "StatefulSet", &s.metadata),
        kind: WorkloadKind::StatefulSet,
        namespace: s.metadata.namespace.clone().unwrap_or_default(),
        name: s.metadata.name.clone().unwrap_or_default(),
        desired_replicas: s.spec.as_ref().and_then(|sp| sp.replicas),
        ready_replicas: s.status.as_ref().and_then(|st| st.ready_replicas),
        selector: s
            .spec
            .as_ref()
            .map(|sp| label_selector(Some(&sp.selector)))
            .unwrap_or_default(),
    }
}

/// Normalize a DaemonSet.
///
/// `desired_replicas` comes from `desiredNumberScheduled` — the count the
/// controller wants given current nodes and tolerations, not a spec field.
/// Using the node count instead would be wrong for any DaemonSet with a node
/// selector.
#[must_use]
pub fn daemon_set(d: &DaemonSet, ctx: &NormalizeContext) -> WorkloadFact {
    WorkloadFact {
        provenance: ctx.provenance("apps", "v1", "DaemonSet", &d.metadata),
        kind: WorkloadKind::DaemonSet,
        namespace: d.metadata.namespace.clone().unwrap_or_default(),
        name: d.metadata.name.clone().unwrap_or_default(),
        desired_replicas: d.status.as_ref().map(|s| s.desired_number_scheduled),
        ready_replicas: d.status.as_ref().map(|s| s.number_ready),
        selector: d
            .spec
            .as_ref()
            .map(|s| label_selector(Some(&s.selector)))
            .unwrap_or_default(),
    }
}

/// Normalize a ReplicaSet.
#[must_use]
pub fn replica_set(r: &ReplicaSet, ctx: &NormalizeContext) -> WorkloadFact {
    WorkloadFact {
        provenance: ctx.provenance("apps", "v1", "ReplicaSet", &r.metadata),
        kind: WorkloadKind::ReplicaSet,
        namespace: r.metadata.namespace.clone().unwrap_or_default(),
        name: r.metadata.name.clone().unwrap_or_default(),
        desired_replicas: r.spec.as_ref().and_then(|s| s.replicas),
        ready_replicas: r.status.as_ref().and_then(|s| s.ready_replicas),
        selector: r
            .spec
            .as_ref()
            .and_then(|s| s.selector.match_labels.as_ref().map(|_| &s.selector))
            .map(|s| label_selector(Some(s)))
            .unwrap_or_default(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::normalize::Origin;
    use chrono::DateTime;
    use ff_core::CollectionStatus;

    fn ctx() -> NormalizeContext {
        let t = DateTime::from_timestamp(1_757_000_000, 0).unwrap();
        NormalizeContext {
            cluster_id: "test".into(),
            observed_at: t,
            collection: CollectionStatus::InSync { synced_at: t },
            origin: Origin::Live,
        }
    }

    #[test]
    fn a_one_replica_deployment_is_a_singleton() {
        let d: Deployment = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "api", "namespace": "demo" },
            "spec": { "replicas": 1, "selector": { "matchLabels": { "app": "api" }},
                      "template": { "metadata": { "labels": { "app": "api" }},
                                    "spec": { "containers": [{ "name": "a" }]}}}
        }))
        .unwrap();
        let fact = deployment(&d, &ctx());
        assert!(fact.is_singleton());
        assert!(fact.kind.reschedules_elsewhere());
    }

    #[test]
    fn daemonset_replicas_come_from_desired_number_scheduled() {
        // A DaemonSet has no spec.replicas. Reading the node count instead
        // would be wrong for any DaemonSet with a node selector.
        let d: DaemonSet = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "agent", "namespace": "demo" },
            "spec": { "selector": { "matchLabels": { "app": "agent" }},
                      "template": { "metadata": { "labels": { "app": "agent" }},
                                    "spec": { "containers": [{ "name": "a" }]}}},
            "status": { "currentNumberScheduled": 2, "desiredNumberScheduled": 2,
                        "numberMisscheduled": 0, "numberReady": 2 }
        }))
        .unwrap();
        let fact = daemon_set(&d, &ctx());
        assert_eq!(fact.desired_replicas, Some(2));
        assert!(
            !fact.kind.reschedules_elsewhere(),
            "DaemonSet pods do not move"
        );
    }
}
