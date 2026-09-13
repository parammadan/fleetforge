//! Node normalization.

use ff_core::{Bytes, ConditionStatus, Millicores, NodeCondition, NodeFact, Taint, TaintEffect};
use k8s_openapi::api::core::v1::Node;

use super::{NormalizeContext, labels_of, quantity};

/// Standard topology label carrying the availability zone.
const ZONE_LABEL: &str = "topology.kubernetes.io/zone";
/// Standard topology label carrying the instance type.
const INSTANCE_TYPE_LABEL: &str = "node.kubernetes.io/instance-type";
/// Brupop's agent requires this label to schedule. Its value is the *updater
/// interface* version — `2.0.0` — and has nothing to do with which Bottlerocket
/// release the node is running.
///
/// Named explicitly because the original code used it as the OS version and
/// reported every Bottlerocket node in a live EKS cluster as "2.0.0". Two
/// different things are called "version" here and they are three orders of
/// magnitude apart.
const BRUPOP_INTERFACE_LABEL: &str = "bottlerocket.aws/updater-interface-version";

/// Extract the Bottlerocket release from the kubelet's OS image string.
///
/// The format is `Bottlerocket OS 1.64.0 (aws-k8s-1.36)`. Returns `None` for
/// anything that is not Bottlerocket, rather than guessing — a node whose
/// version cannot be determined must report absence, not a plausible number.
fn bottlerocket_release(os_image: &str) -> Option<String> {
    let rest = os_image.strip_prefix("Bottlerocket OS ")?;
    let version = rest.split_whitespace().next()?;
    // Guard against a future format change quietly yielding nonsense.
    if version.is_empty() || !version.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    Some(version.to_owned())
}

/// Normalize a Kubernetes node.
#[must_use]
pub fn normalize(node: &Node, ctx: &NormalizeContext) -> NodeFact {
    let meta = &node.metadata;
    let labels = labels_of(meta);
    let status = node.status.as_ref();

    // Allocatable is what the scheduler may hand out; capacity is the machine's
    // total. Using capacity for headroom arithmetic would overstate what is
    // available by whatever the kubelet reserves, which is the wrong direction
    // to be wrong in.
    let (alloc_cpu, alloc_mem) =
        status
            .and_then(|s| s.allocatable.as_ref())
            .map_or((Millicores::ZERO, Bytes::ZERO), |m| {
                (
                    m.get("cpu")
                        .and_then(|q| quantity::parse_cpu(&q.0).ok())
                        .unwrap_or(Millicores::ZERO),
                    m.get("memory")
                        .and_then(|q| quantity::parse_bytes(&q.0).ok())
                        .unwrap_or(Bytes::ZERO),
                )
            });
    let (cap_cpu, cap_mem) =
        status
            .and_then(|s| s.capacity.as_ref())
            .map_or((alloc_cpu, alloc_mem), |m| {
                (
                    m.get("cpu")
                        .and_then(|q| quantity::parse_cpu(&q.0).ok())
                        .unwrap_or(alloc_cpu),
                    m.get("memory")
                        .and_then(|q| quantity::parse_bytes(&q.0).ok())
                        .unwrap_or(alloc_mem),
                )
            });

    NodeFact {
        provenance: ctx.provenance("", "v1", "Node", meta),
        name: meta.name.clone().unwrap_or_default(),
        taints: node
            .spec
            .as_ref()
            .and_then(|s| s.taints.as_ref())
            .map(|taints| {
                taints
                    .iter()
                    .map(|t| Taint {
                        key: t.key.clone(),
                        value: t.value.clone(),
                        effect: match t.effect.as_str() {
                            "NoExecute" => TaintEffect::NoExecute,
                            "PreferNoSchedule" => TaintEffect::PreferNoSchedule,
                            // NoSchedule is the conservative default: it means
                            // a pod needs a toleration, so we err towards
                            // reporting a placement restriction rather than
                            // missing one.
                            _ => TaintEffect::NoSchedule,
                        },
                    })
                    .collect()
            })
            .unwrap_or_default(),
        conditions: status
            .and_then(|s| s.conditions.as_ref())
            .map(|conds| {
                conds
                    .iter()
                    .map(|c| NodeCondition {
                        condition_type: c.type_.clone(),
                        status: match c.status.as_str() {
                            "True" => ConditionStatus::True,
                            "False" => ConditionStatus::False,
                            _ => ConditionStatus::Unknown,
                        },
                        reason: c.reason.clone(),
                        last_transition_at: c.last_transition_time.as_ref().and_then(super::to_utc),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        capacity_cpu: cap_cpu,
        capacity_memory: cap_mem,
        allocatable_cpu: alloc_cpu,
        allocatable_memory: alloc_mem,
        availability_zone: labels.get(ZONE_LABEL).cloned(),
        instance_type: labels.get(INSTANCE_TYPE_LABEL).cloned(),
        unschedulable: node
            .spec
            .as_ref()
            .and_then(|s| s.unschedulable)
            .unwrap_or(false),
        // The OS image string is the only place the running release appears.
        // Brupop's BottlerocketShadow also reports it, but that is a separate
        // fact from a separate source and is collected separately.
        bottlerocket_version: status
            .and_then(|s| s.node_info.as_ref())
            .and_then(|i| bottlerocket_release(&i.os_image)),
        kubelet_version: status
            .and_then(|s| s.node_info.as_ref())
            .map(|i| i.kubelet_version.clone()),
        labels,
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

    fn node_json(json: serde_json::Value) -> Node {
        serde_json::from_value(json).expect("valid node")
    }

    #[test]
    fn allocatable_and_capacity_are_parsed_separately() {
        let node = node_json(serde_json::json!({
            "metadata": { "name": "n1", "uid": "u1", "resourceVersion": "100" },
            "status": {
                "capacity":    { "cpu": "4",     "memory": "8Gi" },
                "allocatable": { "cpu": "3800m", "memory": "7Gi" }
            }
        }));
        let fact = normalize(&node, &ctx());
        assert_eq!(fact.capacity_cpu, Millicores(4000));
        assert_eq!(fact.allocatable_cpu, Millicores(3800));
        assert_eq!(fact.capacity_memory, Bytes::from_gib(8));
        assert_eq!(fact.allocatable_memory, Bytes::from_gib(7));
    }

    #[test]
    fn a_node_missing_status_does_not_panic() {
        // Objects arrive mid-creation. A collector that panics on a partial
        // object takes the whole watch down.
        let node = node_json(serde_json::json!({ "metadata": { "name": "n1" } }));
        let fact = normalize(&node, &ctx());
        assert_eq!(fact.allocatable_cpu, Millicores::ZERO);
        assert!(!fact.is_ready());
    }

    #[test]
    fn cordon_state_is_captured() {
        let node = node_json(serde_json::json!({
            "metadata": { "name": "n1" },
            "spec": { "unschedulable": true }
        }));
        assert!(normalize(&node, &ctx()).unschedulable);
    }

    #[test]
    fn ready_condition_is_recognised() {
        let node = node_json(serde_json::json!({
            "metadata": { "name": "n1" },
            "status": { "conditions": [
                { "type": "MemoryPressure", "status": "False", "lastTransitionTime": "2026-09-12T10:00:00Z" },
                { "type": "Ready", "status": "True", "lastTransitionTime": "2026-09-12T10:00:00Z" }
            ]}
        }));
        assert!(normalize(&node, &ctx()).is_ready());
    }

    #[test]
    fn taints_and_topology_labels_are_carried() {
        let node = node_json(serde_json::json!({
            "metadata": { "name": "n1", "labels": {
                "topology.kubernetes.io/zone": "us-east-2a",
                "node.kubernetes.io/instance-type": "m6g.large"
            }},
            "spec": { "taints": [
                { "key": "dedicated", "value": "gpu", "effect": "NoSchedule" },
                { "key": "evict-me", "effect": "NoExecute" }
            ]}
        }));
        let fact = normalize(&node, &ctx());
        assert_eq!(fact.availability_zone.as_deref(), Some("us-east-2a"));
        assert_eq!(fact.instance_type.as_deref(), Some("m6g.large"));
        assert_eq!(fact.taints.len(), 2);
        assert_eq!(fact.taints[1].effect, TaintEffect::NoExecute);
    }

    #[test]
    fn the_brupop_interface_label_is_not_the_os_version() {
        // Observed on a real EKS cluster: every Bottlerocket node reported
        // version "2.0.0", which is the updater-interface-version label. The
        // actual release was 1.64.0.
        let node = node_json(serde_json::json!({
            "metadata": { "name": "n1", "labels": {
                "bottlerocket.aws/updater-interface-version": "2.0.0"
            }},
            "status": { "nodeInfo": {
                "architecture": "arm64", "bootID": "b", "containerRuntimeVersion": "containerd://2.2.5+bottlerocket",
                "kernelVersion": "6.1", "kubeProxyVersion": "v1.36.1", "kubeletVersion": "v1.36.1",
                "machineID": "m", "operatingSystem": "linux",
                "osImage": "Bottlerocket OS 1.64.0 (aws-k8s-1.36)", "systemUUID": "s"
            }}
        }));
        let fact = normalize(&node, &ctx());
        assert_eq!(fact.bottlerocket_version.as_deref(), Some("1.64.0"));
        assert_ne!(fact.bottlerocket_version.as_deref(), Some("2.0.0"));
        // The label is still present and still carried in labels.
        assert_eq!(
            fact.labels
                .get("bottlerocket.aws/updater-interface-version")
                .map(String::as_str),
            Some("2.0.0")
        );
    }

    #[test]
    fn an_unparseable_os_image_reports_absence_rather_than_a_guess() {
        for os in [
            "Bottlerocket OS",
            "Bottlerocket OS (aws-k8s-1.36)",
            "Amazon Linux 2",
        ] {
            let node = node_json(serde_json::json!({
                "metadata": { "name": "n1" },
                "status": { "nodeInfo": {
                    "architecture": "arm64", "bootID": "b", "containerRuntimeVersion": "containerd://2.0.0",
                    "kernelVersion": "6.1", "kubeProxyVersion": "v1.36.0", "kubeletVersion": "v1.36.0",
                    "machineID": "m", "operatingSystem": "linux", "osImage": os, "systemUUID": "s"
                }}
            }));
            assert!(
                normalize(&node, &ctx()).bottlerocket_version.is_none(),
                "os={os}"
            );
        }
    }

    #[test]
    fn bottlerocket_is_detected_from_the_os_image() {
        let node = node_json(serde_json::json!({
            "metadata": { "name": "n1" },
            "status": { "nodeInfo": {
                "architecture": "arm64", "bootID": "b", "containerRuntimeVersion": "containerd://2.0.0",
                "kernelVersion": "6.1", "kubeProxyVersion": "v1.30.0", "kubeletVersion": "v1.30.0",
                "machineID": "m", "operatingSystem": "linux",
                "osImage": "Bottlerocket OS 1.19.0 (aws-k8s-1.30)", "systemUUID": "s"
            }}
        }));
        let fact = normalize(&node, &ctx());
        assert_eq!(fact.bottlerocket_version.as_deref(), Some("1.19.0"));
        assert_eq!(fact.kubelet_version.as_deref(), Some("v1.30.0"));
    }

    #[test]
    fn a_non_bottlerocket_node_reports_no_bottlerocket_version() {
        let node = node_json(serde_json::json!({
            "metadata": { "name": "n1" },
            "status": { "nodeInfo": {
                "architecture": "arm64", "bootID": "b", "containerRuntimeVersion": "containerd://2.0.0",
                "kernelVersion": "6.1", "kubeProxyVersion": "v1.37.0", "kubeletVersion": "v1.37.0",
                "machineID": "m", "operatingSystem": "linux",
                "osImage": "Debian GNU/Linux 13 (trixie)", "systemUUID": "s"
            }}
        }));
        assert!(normalize(&node, &ctx()).bottlerocket_version.is_none());
    }
}
