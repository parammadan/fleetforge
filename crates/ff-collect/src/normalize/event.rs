//! Kubernetes event normalization.

use ff_core::EventFact;
use k8s_openapi::api::core::v1::Event;

use super::NormalizeContext;

/// Normalize a Kubernetes event.
#[must_use]
pub fn normalize(event: &Event, ctx: &NormalizeContext) -> EventFact {
    let meta = &event.metadata;
    EventFact {
        provenance: ctx.provenance("", "v1", "Event", meta),
        namespace: meta.namespace.clone(),
        involved_object: ff_core::ResourceRef {
            kind: event.involved_object.kind.clone().unwrap_or_default(),
            namespace: event.involved_object.namespace.clone(),
            name: event.involved_object.name.clone().unwrap_or_default(),
            uid: event.involved_object.uid.clone(),
            resource_version: event.involved_object.resource_version.clone(),
        },
        event_type: event.type_.clone().unwrap_or_default(),
        reason: event.reason.clone().unwrap_or_default(),
        message: event.message.clone().unwrap_or_default(),
        // Modern Kubernetes populates eventTime and series rather than the
        // legacy first/lastTimestamp on many events, so fall back rather than
        // reporting an event with no time at all.
        first_seen_at: event
            .first_timestamp
            .as_ref()
            .and_then(super::to_utc)
            .or_else(|| event.event_time.as_ref().and_then(super::micro_to_utc)),
        last_seen_at: event
            .last_timestamp
            .as_ref()
            .and_then(super::to_utc)
            .or_else(|| {
                event
                    .series
                    .as_ref()
                    .and_then(|s| s.last_observed_time.as_ref())
                    .and_then(super::micro_to_utc)
            })
            .or_else(|| event.event_time.as_ref().and_then(super::micro_to_utc)),
        count: event
            .count
            .or_else(|| event.series.as_ref().and_then(|s| s.count))
            .unwrap_or(1),
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
    fn a_legacy_event_is_normalized() {
        let e: Event = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "e1", "namespace": "demo" },
            "involvedObject": { "kind": "Pod", "namespace": "demo", "name": "web-1", "uid": "u1" },
            "type": "Normal", "reason": "Scheduled", "message": "Successfully assigned",
            "firstTimestamp": "2026-09-12T10:00:00Z",
            "lastTimestamp": "2026-09-12T10:00:05Z",
            "count": 2
        }))
        .unwrap();
        let fact = normalize(&e, &ctx());
        assert_eq!(fact.reason, "Scheduled");
        assert_eq!(fact.involved_object.name, "web-1");
        assert_eq!(fact.count, 2);
        assert!(fact.first_seen_at.is_some());
    }

    #[test]
    fn a_modern_event_with_only_event_time_still_has_a_timestamp() {
        let e: Event = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "e1", "namespace": "demo" },
            "involvedObject": { "kind": "Pod", "name": "web-1" },
            "type": "Warning", "reason": "Evicted", "message": "evicted",
            "eventTime": "2026-09-12T10:00:00.000000Z"
        }))
        .unwrap();
        let fact = normalize(&e, &ctx());
        assert!(
            fact.first_seen_at.is_some(),
            "must not lose the time entirely"
        );
        assert_eq!(fact.count, 1);
    }
}
