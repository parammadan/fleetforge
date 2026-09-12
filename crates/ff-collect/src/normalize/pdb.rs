//! PodDisruptionBudget normalization.
//!
//! `disruptions_allowed` is read from `status`, not computed from `spec`. The
//! API server maintains it, and it is the number the eviction API will actually
//! enforce. Recomputing it from `minAvailable` and a pod count would produce a
//! second opinion that disagrees with the cluster at exactly the moments that
//! matter — during a rollout, when pods are not yet ready.

use ff_core::PdbFact;
use k8s_openapi::api::policy::v1::PodDisruptionBudget;

use super::{NormalizeContext, label_selector};

/// Normalize a PodDisruptionBudget.
#[must_use]
pub fn normalize(pdb: &PodDisruptionBudget, ctx: &NormalizeContext) -> PdbFact {
    let spec = pdb.spec.as_ref();
    let status = pdb.status.as_ref();

    PdbFact {
        provenance: ctx.provenance("policy", "v1", "PodDisruptionBudget", &pdb.metadata),
        namespace: pdb.metadata.namespace.clone().unwrap_or_default(),
        name: pdb.metadata.name.clone().unwrap_or_default(),
        selector: label_selector(spec.and_then(|s| s.selector.as_ref())),
        // Kept as declared strings: `minAvailable` may be "2" or "50%", and an
        // evidence report should quote what the operator wrote rather than an
        // interpretation of it.
        min_available: spec
            .and_then(|s| s.min_available.as_ref())
            .map(intstr_to_string),
        max_unavailable: spec
            .and_then(|s| s.max_unavailable.as_ref())
            .map(intstr_to_string),
        current_healthy: status.and_then(|s| s.current_healthy).unwrap_or(0),
        desired_healthy: status.and_then(|s| s.desired_healthy).unwrap_or(0),
        expected_pods: status.and_then(|s| s.expected_pods).unwrap_or(0),
        // Absent status means the controller has not computed it yet. Zero is
        // the safe reading: it means "no disruption permitted", so FleetForge
        // reports blocked rather than waving a drain through on missing data.
        disruptions_allowed: status.and_then(|s| s.disruptions_allowed).unwrap_or(0),
    }
}

fn intstr_to_string(v: &k8s_openapi::apimachinery::pkg::util::intstr::IntOrString) -> String {
    match v {
        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(i) => i.to_string(),
        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(s) => s.clone(),
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

    fn pdb_json(v: serde_json::Value) -> PodDisruptionBudget {
        serde_json::from_value(v).expect("valid pdb")
    }

    #[test]
    fn a_permissive_pdb_allows_disruption() {
        let pdb = pdb_json(serde_json::json!({
            "metadata": { "name": "web-pdb", "namespace": "demo" },
            "spec": { "minAvailable": 2, "selector": { "matchLabels": { "app": "web" }}},
            "status": { "currentHealthy": 3, "desiredHealthy": 2, "expectedPods": 3,
                        "disruptionsAllowed": 1, "observedGeneration": 1 }
        }));
        let fact = normalize(&pdb, &ctx());
        assert!(!fact.blocks_disruption());
        assert_eq!(fact.min_available.as_deref(), Some("2"));
    }

    #[test]
    fn a_restrictive_pdb_blocks_disruption() {
        let pdb = pdb_json(serde_json::json!({
            "metadata": { "name": "web-pdb", "namespace": "demo" },
            "spec": { "minAvailable": 3, "selector": { "matchLabels": { "app": "web" }}},
            "status": { "currentHealthy": 3, "desiredHealthy": 3, "expectedPods": 3,
                        "disruptionsAllowed": 0, "observedGeneration": 1 }
        }));
        assert!(normalize(&pdb, &ctx()).blocks_disruption());
    }

    #[test]
    fn a_percentage_min_available_is_preserved_verbatim() {
        // An evidence report must quote what the operator wrote.
        let pdb = pdb_json(serde_json::json!({
            "metadata": { "name": "p", "namespace": "demo" },
            "spec": { "minAvailable": "50%", "selector": { "matchLabels": { "app": "web" }}},
            "status": { "currentHealthy": 2, "desiredHealthy": 1, "expectedPods": 3,
                        "disruptionsAllowed": 1, "observedGeneration": 1 }
        }));
        assert_eq!(
            normalize(&pdb, &ctx()).min_available.as_deref(),
            Some("50%")
        );
    }

    #[test]
    fn a_pdb_without_status_is_treated_as_blocking() {
        // The controller has not computed it yet. Reporting "disruption
        // allowed" on missing data is how a tool waves through an outage.
        let pdb = pdb_json(serde_json::json!({
            "metadata": { "name": "p", "namespace": "demo" },
            "spec": { "minAvailable": 1, "selector": { "matchLabels": { "app": "web" }}}
        }));
        assert!(normalize(&pdb, &ctx()).blocks_disruption());
    }
}
