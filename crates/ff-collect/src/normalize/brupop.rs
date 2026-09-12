//! Brupop `BottlerocketShadow` normalization.
//!
//! Read through kube's dynamic API rather than a generated type, because the
//! CRD belongs to Brupop and may be absent, partially installed, or a version
//! FleetForge has not seen. A generated type would turn any of those into a
//! deserialization failure; reading the JSON defensively turns them into
//! absent fields, which is what they are.
//!
//! Field names are tried in several spellings for the same reason. Brupop has
//! used both `currentState` and `current_state` across versions, and guessing
//! wrong should mean "unknown", never a crash or a fabricated value.

use ff_core::BrupopFact;
use kube::api::DynamicObject;

use super::NormalizeContext;

/// The Brupop API group.
pub const GROUP: &str = "brupop.bottlerocket.aws";
/// The API version FleetForge reads.
pub const VERSION: &str = "v2";
/// The kind.
pub const KIND: &str = "BottlerocketShadow";
/// The plural resource name, used when reporting a denial.
pub const PLURAL: &str = "bottlerocketshadows";

/// Pull the first present field from a JSON object, trying several spellings.
fn first_string(value: Option<&serde_json::Value>, keys: &[&str]) -> Option<String> {
    let object = value?;
    keys.iter()
        .find_map(|key| object.get(*key).and_then(|v| v.as_str()))
        .map(ToOwned::to_owned)
}

fn first_i64(value: Option<&serde_json::Value>, keys: &[&str]) -> Option<i64> {
    let object = value?;
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_i64))
}

/// Normalize a `BottlerocketShadow`.
#[must_use]
pub fn normalize(object: &DynamicObject, ctx: &NormalizeContext) -> BrupopFact {
    let spec = object.data.get("spec");
    let status = object.data.get("status");

    BrupopFact {
        provenance: ctx.provenance(GROUP, VERSION, KIND, &object.metadata),
        // Brupop names each shadow after the node it tracks.
        node_name: object.metadata.name.clone().unwrap_or_default(),
        current_state: first_string(status, &["current_state", "currentState", "state"]),
        target_state: first_string(spec, &["state", "target_state", "targetState"]),
        current_version: first_string(status, &["current_version", "currentVersion", "version"]),
        target_version: first_string(spec, &["version", "target_version", "targetVersion"])
            .or_else(|| first_string(status, &["target_version", "targetVersion"])),
        crash_count: first_i64(status, &["crash_count", "crashCount"]),
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

    fn shadow(json: serde_json::Value) -> DynamicObject {
        serde_json::from_value(json).expect("valid dynamic object")
    }

    #[test]
    fn reads_a_shadow_mid_update() {
        let object = shadow(serde_json::json!({
            "apiVersion": "brupop.bottlerocket.aws/v2",
            "kind": "BottlerocketShadow",
            "metadata": { "name": "ip-10-0-1-5", "namespace": "brupop-bottlerocket-aws",
                          "uid": "u1", "resourceVersion": "42" },
            "spec": { "state": "StagedAndPerformedUpdate", "version": "1.20.0" },
            "status": { "current_state": "StagedUpdate", "current_version": "1.19.0",
                        "crash_count": 0 }
        }));
        let fact = normalize(&object, &ctx());
        assert_eq!(fact.node_name, "ip-10-0-1-5");
        assert_eq!(fact.current_state.as_deref(), Some("StagedUpdate"));
        assert_eq!(
            fact.target_state.as_deref(),
            Some("StagedAndPerformedUpdate")
        );
        assert_eq!(fact.current_version.as_deref(), Some("1.19.0"));
        assert_eq!(fact.target_version.as_deref(), Some("1.20.0"));
        assert!(fact.is_in_transition());
    }

    #[test]
    fn a_node_at_rest_is_not_in_transition() {
        let object = shadow(serde_json::json!({
            "apiVersion": "brupop.bottlerocket.aws/v2",
            "kind": "BottlerocketShadow",
            "metadata": { "name": "ip-10-0-1-5" },
            "spec": { "state": "Idle" },
            "status": { "current_state": "Idle", "current_version": "1.20.0" }
        }));
        assert!(!normalize(&object, &ctx()).is_in_transition());
    }

    #[test]
    fn camel_case_spellings_are_accepted() {
        // Brupop has used both conventions across versions. Guessing wrong must
        // mean "unknown", never a wrong value.
        let object = shadow(serde_json::json!({
            "apiVersion": "brupop.bottlerocket.aws/v2",
            "kind": "BottlerocketShadow",
            "metadata": { "name": "n1" },
            "spec": { "state": "Idle" },
            "status": { "currentState": "Idle", "currentVersion": "1.20.0" }
        }));
        let fact = normalize(&object, &ctx());
        assert_eq!(fact.current_state.as_deref(), Some("Idle"));
        assert_eq!(fact.current_version.as_deref(), Some("1.20.0"));
    }

    #[test]
    fn a_shadow_with_no_status_yet_reports_absence_not_a_guess() {
        let object = shadow(serde_json::json!({
            "apiVersion": "brupop.bottlerocket.aws/v2",
            "kind": "BottlerocketShadow",
            "metadata": { "name": "n1" },
            "spec": { "state": "Idle" }
        }));
        let fact = normalize(&object, &ctx());
        assert!(fact.current_state.is_none());
        assert!(fact.current_version.is_none());
        assert!(!fact.is_in_transition(), "unknown is not a transition");
    }

    #[test]
    fn an_unrecognised_shape_does_not_panic() {
        let object = shadow(serde_json::json!({
            "apiVersion": "brupop.bottlerocket.aws/v2",
            "kind": "BottlerocketShadow",
            "metadata": { "name": "n1" },
            "spec": { "somethingNew": { "nested": true } },
            "status": []
        }));
        let fact = normalize(&object, &ctx());
        assert_eq!(fact.node_name, "n1");
        assert!(fact.current_state.is_none());
    }
}
