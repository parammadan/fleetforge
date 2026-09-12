//! Normalizing Kubernetes API objects into `ff-core` facts.
//!
//! Every function here is pure: an API object in, a fact out. No network, no
//! clock of its own — the observation time is supplied by the caller, so the
//! same object normalized twice produces the same fact.
//!
//! Provenance is attached here rather than later, because here is where we
//! still know where the object came from. A fact that reaches the snapshot
//! without provenance would not compile, and a fact that reaches it with the
//! *wrong* provenance would be a lie we could not detect.

pub mod brupop;
pub mod event;
pub mod node;
pub mod pdb;
pub mod pod;
pub mod quantity;
pub mod workload;

use chrono::{DateTime, Utc};
use ff_core::{
    CollectionStatus, LabelSelector, LabelSelectorOperator, LabelSelectorRequirement, Provenance,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{
    LabelSelector as K8sLabelSelector, MicroTime, ObjectMeta, Time,
};
use std::collections::BTreeMap;

/// Where the objects being normalized are coming from.
///
/// This is what decides the [`ff_core::Mode`] of every resulting fact, and it
/// is the reason a fixture cannot be normalized into live data: `Fixture`
/// produces `Mode::Fixture` and there is no code path that says otherwise
/// (ADR-0020).
#[derive(Clone, Debug)]
pub enum Origin {
    /// A live Kubernetes watch.
    Live,
    /// A recorded snapshot on disk.
    Fixture {
        /// File name, relative to the fixtures directory.
        file: String,
    },
}

/// Everything a normalizer needs besides the object itself.
#[derive(Clone, Debug)]
pub struct NormalizeContext {
    /// Stable cluster identifier. Never an API server URL.
    pub cluster_id: String,
    /// When these objects were observed.
    pub observed_at: DateTime<Utc>,
    /// How well the owning kind was collected.
    pub collection: CollectionStatus,
    /// Live or fixture.
    pub origin: Origin,
}

impl NormalizeContext {
    /// Build the provenance for one object.
    pub(crate) fn provenance(
        &self,
        group: &str,
        version: &str,
        kind: &str,
        meta: &ObjectMeta,
    ) -> Provenance {
        let base = match &self.origin {
            Origin::Live => {
                Provenance::live(&self.cluster_id, group, version, kind, self.observed_at)
            }
            Origin::Fixture { file } => {
                Provenance::fixture(&self.cluster_id, file.clone(), self.observed_at)
            }
        };
        base.with_object(
            meta.namespace.clone(),
            meta.uid.clone(),
            meta.resource_version.clone(),
        )
        .with_collection(self.collection.clone())
    }
}

/// Convert a Kubernetes timestamp into the model's `chrono` type.
///
/// k8s-openapi 0.28 represents times as `jiff::Timestamp`. `ff-core` uses
/// `chrono`, and deliberately keeps doing so: the domain model should not take
/// a dependency on whichever time library the Kubernetes bindings happen to
/// prefer this year. Converting at the boundary is the cost of that
/// independence, and it is small.
pub(crate) fn to_utc(time: &Time) -> Option<DateTime<Utc>> {
    let ts = time.0;
    DateTime::from_timestamp(
        ts.as_second(),
        u32::try_from(ts.subsec_nanosecond().max(0)).unwrap_or(0),
    )
}

/// Same conversion for the microsecond-precision variant used by `eventTime`.
pub(crate) fn micro_to_utc(time: &MicroTime) -> Option<DateTime<Utc>> {
    let ts = time.0;
    DateTime::from_timestamp(
        ts.as_second(),
        u32::try_from(ts.subsec_nanosecond().max(0)).unwrap_or(0),
    )
}

/// Convert Kubernetes labels, which are `Option<BTreeMap>`, into the map the
/// model always has.
pub(crate) fn labels_of(meta: &ObjectMeta) -> BTreeMap<String, String> {
    meta.labels.clone().unwrap_or_default()
}

/// Convert a Kubernetes label selector.
///
/// An absent selector and an empty selector are **not** the same thing in
/// Kubernetes, but both match everything here, which matches the API server's
/// behaviour for the objects FleetForge reads. `LabelSelector::matches`
/// reproduces that deliberately rather than "fixing" it, because an
/// accidentally cluster-wide PodDisruptionBudget is a real thing operators
/// create and FleetForge needs to see it the way the cluster sees it.
pub(crate) fn label_selector(selector: Option<&K8sLabelSelector>) -> LabelSelector {
    let Some(sel) = selector else {
        return LabelSelector::default();
    };
    LabelSelector {
        match_labels: sel.match_labels.clone().unwrap_or_default(),
        match_expressions: sel
            .match_expressions
            .as_ref()
            .map(|exprs| {
                exprs
                    .iter()
                    .map(|e| LabelSelectorRequirement {
                        key: e.key.clone(),
                        operator: match e.operator.as_str() {
                            "In" => LabelSelectorOperator::In,
                            "NotIn" => LabelSelectorOperator::NotIn,
                            "DoesNotExist" => LabelSelectorOperator::DoesNotExist,
                            // Kubernetes validates this field, so anything else
                            // should be impossible. Treating an unknown
                            // operator as Exists is the conservative choice: it
                            // matches more pods, so a PDB is more likely to be
                            // reported as covering a pod than less.
                            _ => LabelSelectorOperator::Exists,
                        },
                        values: e.values.clone().unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use ff_core::Mode;

    fn ctx(origin: Origin) -> NormalizeContext {
        NormalizeContext {
            cluster_id: "test-cluster".into(),
            observed_at: DateTime::from_timestamp(1_757_000_000, 0).unwrap(),
            collection: CollectionStatus::InSync {
                synced_at: DateTime::from_timestamp(1_757_000_000, 0).unwrap(),
            },
            origin,
        }
    }

    #[test]
    fn live_origin_produces_live_facts() {
        let c = ctx(Origin::Live);
        let p = c.provenance("", "v1", "Node", &ObjectMeta::default());
        assert_eq!(p.mode(), Mode::Live);
    }

    #[test]
    fn fixture_origin_produces_fixture_facts_even_though_captured_live() {
        // ADR-0020: provenance describes where a fact is being read from now,
        // not where it was born.
        let c = ctx(Origin::Fixture {
            file: "nodes.json".into(),
        });
        let p = c.provenance("", "v1", "Node", &ObjectMeta::default());
        assert_eq!(p.mode(), Mode::Fixture);
        assert!(!p.mode().describes_current_reality());
    }

    #[test]
    fn an_absent_selector_matches_everything() {
        let sel = label_selector(None);
        assert!(sel.matches(&BTreeMap::new()));
        assert!(sel.matches(&BTreeMap::from([("app".into(), "web".into())])));
    }

    #[test]
    fn match_labels_are_translated() {
        let k8s = K8sLabelSelector {
            match_labels: Some(BTreeMap::from([("app".to_owned(), "web".to_owned())])),
            match_expressions: None,
        };
        let sel = label_selector(Some(&k8s));
        assert!(sel.matches(&BTreeMap::from([("app".into(), "web".into())])));
        assert!(!sel.matches(&BTreeMap::from([("app".into(), "api".into())])));
    }
}
