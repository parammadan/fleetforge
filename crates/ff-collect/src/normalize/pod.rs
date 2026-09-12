//! Pod normalization.
//!
//! The pod is where almost every placement constraint lives, so this is the
//! richest normalizer. Anything dropped here is a constraint an analyzer can
//! never see, which is why volume classification in particular is explicit
//! rather than best-effort.

use ff_core::{
    ContainerFact, NodeSelectorOperator, NodeSelectorRequirement, NodeSelectorTerm,
    PodAffinityTerm, PodFact, PodPhase, TaintEffect, Toleration, TolerationOperator,
    TopologySpreadConstraint, UnsatisfiableAction, VolumeFact, VolumeKind,
};
use k8s_openapi::api::core::v1::{Pod, Volume};

use super::{NormalizeContext, label_selector, labels_of, quantity};

/// Normalize a Kubernetes pod.
#[must_use]
pub fn normalize(pod: &Pod, ctx: &NormalizeContext) -> PodFact {
    let meta = &pod.metadata;
    let spec = pod.spec.as_ref();

    PodFact {
        provenance: ctx.provenance("", "v1", "Pod", meta),
        namespace: meta.namespace.clone().unwrap_or_default(),
        name: meta.name.clone().unwrap_or_default(),
        labels: labels_of(meta),
        node_name: spec.and_then(|s| s.node_name.clone()),
        phase: pod.status.as_ref().and_then(|s| s.phase.as_deref()).map_or(
            PodPhase::Unknown,
            |p| match p {
                "Pending" => PodPhase::Pending,
                "Running" => PodPhase::Running,
                "Succeeded" => PodPhase::Succeeded,
                "Failed" => PodPhase::Failed,
                _ => PodPhase::Unknown,
            },
        ),
        // Controller first. An empty chain means nothing will recreate this pod
        // after an eviction, which the unmanaged-pod analyzer cares about.
        owners: meta
            .owner_references
            .as_ref()
            .map(|refs| {
                let mut sorted: Vec<_> = refs.iter().collect();
                sorted.sort_by_key(|r| !r.controller.unwrap_or(false));
                sorted
                    .iter()
                    .map(|r| ff_core::ResourceRef {
                        kind: r.kind.clone(),
                        namespace: meta.namespace.clone(),
                        name: r.name.clone(),
                        uid: Some(r.uid.clone()),
                        resource_version: None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        containers: spec
            .map(|s| {
                // Init containers are deliberately excluded from the steady-state
                // footprint: they have run and exited. Sidecar init containers
                // (restartPolicy: Always) do persist, so those are included.
                let init_sidecars = s
                    .init_containers
                    .iter()
                    .flatten()
                    .filter(|c| c.restart_policy.as_deref() == Some("Always"));
                s.containers
                    .iter()
                    .chain(init_sidecars)
                    .map(|c| {
                        let req = c.resources.as_ref().and_then(|r| r.requests.as_ref());
                        let lim = c.resources.as_ref().and_then(|r| r.limits.as_ref());
                        ContainerFact {
                            name: c.name.clone(),
                            cpu_request: req
                                .and_then(|m| m.get("cpu"))
                                .and_then(|q| quantity::parse_cpu(&q.0).ok()),
                            memory_request: req
                                .and_then(|m| m.get("memory"))
                                .and_then(|q| quantity::parse_bytes(&q.0).ok()),
                            cpu_limit: lim
                                .and_then(|m| m.get("cpu"))
                                .and_then(|q| quantity::parse_cpu(&q.0).ok()),
                            memory_limit: lim
                                .and_then(|m| m.get("memory"))
                                .and_then(|q| quantity::parse_bytes(&q.0).ok()),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        node_selector: spec
            .and_then(|s| s.node_selector.clone())
            .unwrap_or_default(),
        tolerations: spec
            .and_then(|s| s.tolerations.as_ref())
            .map(|tols| {
                tols.iter()
                    .map(|t| Toleration {
                        key: t.key.clone(),
                        operator: match t.operator.as_deref() {
                            Some("Exists") => TolerationOperator::Exists,
                            _ => TolerationOperator::Equal,
                        },
                        value: t.value.clone(),
                        effect: t.effect.as_deref().map(|e| match e {
                            "NoExecute" => TaintEffect::NoExecute,
                            "PreferNoSchedule" => TaintEffect::PreferNoSchedule,
                            _ => TaintEffect::NoSchedule,
                        }),
                        toleration_seconds: t.toleration_seconds,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // Only *required* affinity is modelled. Preferred affinity influences
        // scoring, not admission, so it cannot make a pod unschedulable and
        // must not be reported as if it could.
        required_node_affinity: spec
            .and_then(|s| s.affinity.as_ref())
            .and_then(|a| a.node_affinity.as_ref())
            .and_then(|na| {
                na.required_during_scheduling_ignored_during_execution
                    .as_ref()
            })
            .map(|sel| {
                sel.node_selector_terms
                    .iter()
                    .map(|term| NodeSelectorTerm {
                        match_expressions: term
                            .match_expressions
                            .as_ref()
                            .map(|e| e.iter().map(node_selector_requirement).collect())
                            .unwrap_or_default(),
                        match_fields: term
                            .match_fields
                            .as_ref()
                            .map(|e| e.iter().map(node_selector_requirement).collect())
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        required_pod_anti_affinity: spec
            .and_then(|s| s.affinity.as_ref())
            .and_then(|a| a.pod_anti_affinity.as_ref())
            .and_then(|paa| {
                paa.required_during_scheduling_ignored_during_execution
                    .as_ref()
            })
            .map(|terms| {
                terms
                    .iter()
                    .map(|t| PodAffinityTerm {
                        label_selector: label_selector(t.label_selector.as_ref()),
                        topology_key: t.topology_key.clone(),
                        namespaces: t.namespaces.clone().unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        topology_spread: spec
            .and_then(|s| s.topology_spread_constraints.as_ref())
            .map(|cs| {
                cs.iter()
                    .map(|c| TopologySpreadConstraint {
                        max_skew: c.max_skew,
                        topology_key: c.topology_key.clone(),
                        when_unsatisfiable: match c.when_unsatisfiable.as_str() {
                            "DoNotSchedule" => UnsatisfiableAction::DoNotSchedule,
                            _ => UnsatisfiableAction::ScheduleAnyway,
                        },
                        label_selector: label_selector(c.label_selector.as_ref()),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        volumes: spec
            .and_then(|s| s.volumes.as_ref())
            .map(|vs| vs.iter().map(volume).collect())
            .unwrap_or_default(),
        priority: spec.and_then(|s| s.priority),
        priority_class_name: spec.and_then(|s| s.priority_class_name.clone()),
        termination_grace_period_seconds: spec.and_then(|s| s.termination_grace_period_seconds),
    }
}

fn node_selector_requirement(
    r: &k8s_openapi::api::core::v1::NodeSelectorRequirement,
) -> NodeSelectorRequirement {
    NodeSelectorRequirement {
        key: r.key.clone(),
        operator: match r.operator.as_str() {
            "In" => NodeSelectorOperator::In,
            "NotIn" => NodeSelectorOperator::NotIn,
            "DoesNotExist" => NodeSelectorOperator::DoesNotExist,
            "Gt" => NodeSelectorOperator::Gt,
            "Lt" => NodeSelectorOperator::Lt,
            _ => NodeSelectorOperator::Exists,
        },
        values: r.values.clone().unwrap_or_default(),
    }
}

/// Classify a volume by whether it pins its pod to a node.
///
/// This classification decides whether a pod can move at all, so the unknown
/// case matters: a volume type this function does not recognise is treated as
/// `Ephemeral`, i.e. movable. That is the *optimistic* choice, and it is the
/// wrong direction for safety — so the storage analyzer declares in its
/// limitations that it recognises a fixed set of volume types. Guessing
/// "immovable" for every unrecognised projected volume would make the tool
/// cry wolf on every pod with a ConfigMap mounted.
fn volume(v: &Volume) -> VolumeFact {
    let (kind, claim_name) = if v.host_path.is_some() {
        (VolumeKind::HostPath, None)
    } else if let Some(pvc) = v.persistent_volume_claim.as_ref() {
        // A PVC alone does not say whether the volume is node-bound; that lives
        // on the PersistentVolume. Without PV read permission the honest answer
        // is "network-attached, possibly zone-bound", and the analyzer says so.
        (
            VolumeKind::NetworkPersistentVolume,
            Some(pvc.claim_name.clone()),
        )
    } else if v.empty_dir.is_some() {
        (VolumeKind::EmptyDir, None)
    } else {
        // Everything else is movable. That covers the recognised projected
        // kinds — ConfigMap, Secret, projected, downwardAPI — and also any
        // volume type this function has never heard of. Those two cases are
        // the same answer for different reasons: the first is known, the
        // second is an assumption, and the optimistic direction. The storage
        // analyzer declares that assumption in its `limitations` rather than
        // the model pretending to a certainty it does not have.
        (VolumeKind::Ephemeral, None)
    };

    VolumeFact {
        name: v.name.clone(),
        kind,
        claim_name,
        zones: Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::normalize::Origin;
    use chrono::DateTime;
    use ff_core::{Bytes, CollectionStatus, Millicores};

    fn ctx() -> NormalizeContext {
        let t = DateTime::from_timestamp(1_757_000_000, 0).unwrap();
        NormalizeContext {
            cluster_id: "test".into(),
            observed_at: t,
            collection: CollectionStatus::InSync { synced_at: t },
            origin: Origin::Live,
        }
    }

    fn pod_json(json: serde_json::Value) -> Pod {
        serde_json::from_value(json).expect("valid pod")
    }

    #[test]
    fn container_requests_are_summed() {
        let pod = pod_json(serde_json::json!({
            "metadata": { "name": "p", "namespace": "demo" },
            "spec": { "containers": [
                { "name": "a", "resources": { "requests": { "cpu": "250m", "memory": "256Mi" }}},
                { "name": "b", "resources": { "requests": { "cpu": "0.5",  "memory": "128Mi" }}}
            ]}
        }));
        let fact = normalize(&pod, &ctx());
        assert_eq!(fact.total_cpu_request(), Millicores(750));
        assert_eq!(fact.total_memory_request(), Bytes::from_mib(384));
    }

    #[test]
    fn init_containers_are_excluded_but_sidecars_are_not() {
        // A plain init container has exited and occupies nothing. A sidecar
        // init container (restartPolicy Always) runs for the pod's lifetime.
        let pod = pod_json(serde_json::json!({
            "metadata": { "name": "p", "namespace": "demo" },
            "spec": {
                "initContainers": [
                    { "name": "migrate", "resources": { "requests": { "cpu": "2" }}},
                    { "name": "proxy", "restartPolicy": "Always",
                      "resources": { "requests": { "cpu": "100m" }}}
                ],
                "containers": [
                    { "name": "app", "resources": { "requests": { "cpu": "250m" }}}
                ]
            }
        }));
        let fact = normalize(&pod, &ctx());
        assert_eq!(fact.total_cpu_request(), Millicores(350));
    }

    #[test]
    fn an_unmanaged_pod_has_no_owners() {
        let pod = pod_json(serde_json::json!({
            "metadata": { "name": "p", "namespace": "demo" }
        }));
        assert!(normalize(&pod, &ctx()).is_unmanaged());
    }

    #[test]
    fn the_controller_owner_sorts_first() {
        let pod = pod_json(serde_json::json!({
            "metadata": { "name": "p", "namespace": "demo", "ownerReferences": [
                { "apiVersion": "v1", "kind": "Thing", "name": "t", "uid": "u1" },
                { "apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "rs", "uid": "u2",
                  "controller": true }
            ]}
        }));
        let fact = normalize(&pod, &ctx());
        assert!(!fact.is_unmanaged());
        assert_eq!(fact.controller().unwrap().kind, "ReplicaSet");
    }

    #[test]
    fn node_bound_storage_is_detected() {
        for (vol, expected) in [
            (
                serde_json::json!({ "name": "v", "hostPath": { "path": "/data" }}),
                true,
            ),
            (serde_json::json!({ "name": "v", "emptyDir": {} }), true),
            (
                serde_json::json!({ "name": "v", "configMap": { "name": "c" }}),
                false,
            ),
            (
                serde_json::json!({ "name": "v", "persistentVolumeClaim": { "claimName": "c" }}),
                false,
            ),
        ] {
            let pod = pod_json(serde_json::json!({
                "metadata": { "name": "p", "namespace": "demo" },
                "spec": { "containers": [{ "name": "a" }], "volumes": [vol] }
            }));
            assert_eq!(normalize(&pod, &ctx()).is_node_bound(), expected);
        }
    }

    #[test]
    fn only_required_affinity_is_modelled() {
        // Preferred affinity affects scoring, not admission. Reporting it as a
        // placement restriction would produce findings for pods that can in
        // fact be scheduled anywhere.
        let pod = pod_json(serde_json::json!({
            "metadata": { "name": "p", "namespace": "demo" },
            "spec": { "containers": [{ "name": "a" }], "affinity": { "nodeAffinity": {
                "preferredDuringSchedulingIgnoredDuringExecution": [{
                    "weight": 1,
                    "preference": { "matchExpressions": [
                        { "key": "zone", "operator": "In", "values": ["a"] }]}
                }]
            }}}
        }));
        assert!(normalize(&pod, &ctx()).required_node_affinity.is_empty());
    }

    #[test]
    fn required_affinity_and_topology_spread_are_captured() {
        let pod = pod_json(serde_json::json!({
            "metadata": { "name": "p", "namespace": "demo" },
            "spec": {
                "containers": [{ "name": "a" }],
                "terminationGracePeriodSeconds": 120,
                "affinity": { "nodeAffinity": {
                    "requiredDuringSchedulingIgnoredDuringExecution": {
                        "nodeSelectorTerms": [{ "matchExpressions": [
                            { "key": "disk", "operator": "In", "values": ["ssd"] }]}]
                    }}},
                "topologySpreadConstraints": [{
                    "maxSkew": 1,
                    "topologyKey": "topology.kubernetes.io/zone",
                    "whenUnsatisfiable": "DoNotSchedule",
                    "labelSelector": { "matchLabels": { "app": "web" }}
                }]
            }
        }));
        let fact = normalize(&pod, &ctx());
        assert_eq!(fact.required_node_affinity.len(), 1);
        assert_eq!(fact.topology_spread.len(), 1);
        assert_eq!(
            fact.topology_spread[0].when_unsatisfiable,
            UnsatisfiableAction::DoNotSchedule
        );
        assert_eq!(fact.termination_grace_period_seconds, Some(120));
    }

    #[test]
    fn a_terminated_pod_does_not_occupy_capacity() {
        let pod = pod_json(serde_json::json!({
            "metadata": { "name": "p", "namespace": "demo" },
            "spec": { "containers": [{ "name": "a" }] },
            "status": { "phase": "Succeeded" }
        }));
        assert!(!normalize(&pod, &ctx()).phase.occupies_capacity());
    }
}
