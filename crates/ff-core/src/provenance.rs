//! Provenance: where a fact came from, and what it is allowed to claim.
//!
//! FleetForge mixes live cluster reads, calculated what-if outcomes, recorded
//! history, and test fixtures. Confusing them is the most damaging failure the
//! product can have, and the realistic failure mode is not malice — it is an
//! engineer adding a code path and forgetting the label.
//!
//! So the label is not optional, has no default, and is validated twice: once
//! when a [`Provenance`] is constructed, and again when one is deserialized.
//! See ADR-0003.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{FleetForgeError, Result};
use crate::snapshot::SnapshotId;

/// What kind of data this is. There is no `Unknown` variant on purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Read from the current Kubernetes or AWS environment.
    Live,
    /// A calculated future outcome. It has **not** happened.
    WhatIf,
    /// Previously recorded events, played back.
    Replay,
    /// Development or automated-test data.
    Fixture,
}

impl Mode {
    /// The label the interface displays. Uppercase, because it is chrome the
    /// operator should never have to hunt for.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Live => "LIVE",
            Self::WhatIf => "WHAT-IF",
            Self::Replay => "REPLAY",
            Self::Fixture => "FIXTURE",
        }
    }

    /// Whether this data reflects the cluster as it is right now.
    ///
    /// Only [`Mode::Live`] does. A what-if has not happened, a replay already
    /// happened, and a fixture never happened.
    #[must_use]
    pub const fn describes_current_reality(self) -> bool {
        matches!(self, Self::Live)
    }
}

/// Where a fact physically came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Source {
    /// A kube-rs watch on a specific API group, version, and kind.
    KubeWatch {
        /// API group; empty string for the core group.
        group: String,
        /// API version, for example `v1`.
        version: String,
        /// Kind, for example `Node`.
        kind: String,
    },
    /// A recorded snapshot loaded from `fixtures/`.
    Fixture {
        /// Path relative to the fixtures directory.
        file: String,
    },
    /// Derived by FleetForge from a specific snapshot rather than observed.
    Computed {
        /// The snapshot the calculation ran against.
        from: SnapshotId,
    },
    /// Replayed from a recorded event log.
    EventLog {
        /// Identifier of the recorded run.
        run_id: String,
    },
}

impl Source {
    /// The set of modes this source is permitted to claim.
    ///
    /// This is the whole honesty invariant, in one function.
    #[must_use]
    pub fn permits(&self, mode: Mode) -> bool {
        match self {
            // A watch observes the cluster as it is. It cannot produce a
            // calculation, and it is not a recording.
            Self::KubeWatch { .. } => matches!(mode, Mode::Live),
            // Fixture data never happened. It cannot be live, and it cannot be
            // replay — nothing was recorded.
            Self::Fixture { .. } => matches!(mode, Mode::Fixture),
            // A calculation has not happened. Its inputs may have been live or
            // fixture, but the output is a what-if either way.
            Self::Computed { .. } => matches!(mode, Mode::WhatIf),
            // A recording already happened. It is not current reality.
            Self::EventLog { .. } => matches!(mode, Mode::Replay),
        }
    }

    /// A short description used in error messages.
    #[must_use]
    pub const fn describe(&self) -> &'static str {
        match self {
            Self::KubeWatch { .. } => "kubernetes watch",
            Self::Fixture { .. } => "fixture file",
            Self::Computed { .. } => "computed value",
            Self::EventLog { .. } => "recorded event log",
        }
    }
}

/// How completely FleetForge was able to observe one Kubernetes kind.
///
/// This is as important as the facts themselves. A snapshot that received `403`
/// on PodDisruptionBudgets must never be analyzed as though the cluster had
/// none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CollectionStatus {
    /// The initial list is still in progress.
    Syncing,
    /// The watch is established and current as of this instant.
    InSync {
        /// When the collector last confirmed it was current.
        ///
        /// Named distinctly rather than `at` because canonical hashing strips
        /// volatile fields by name; see `canonical.rs`.
        synced_at: DateTime<Utc>,
    },
    /// RBAC denied the read. The verb and resource are named so the operator
    /// can fix the role binding, and so the interface never renders this as an
    /// empty result.
    Forbidden {
        /// The verb that was denied, for example `list`.
        verb: String,
        /// The resource that was denied, for example `poddisruptionbudgets`.
        resource: String,
    },
    /// The resource type does not exist in this cluster.
    ///
    /// **This is authoritative.** If the CustomResourceDefinition is not
    /// installed, then "zero of them" is not a guess — it is the only possible
    /// answer, and it is correct. That separates it sharply from
    /// [`Forbidden`](Self::Forbidden), where zero means "we could not look".
    ///
    /// Getting this wrong in the other direction would be costly in its own
    /// way: treating an absent optional CRD as non-authoritative would mark
    /// every cluster without Brupop permanently untrustworthy, and an
    /// incompleteness warning that is always on is one nobody reads.
    NotInstalled {
        /// The resource type that does not exist, for example
        /// `bottlerocketshadows`.
        resource: String,
    },
    /// The watch is failing and the data is no longer being refreshed.
    Degraded {
        /// When it started failing.
        degraded_since: DateTime<Utc>,
        /// A redacted description of the failure.
        error: String,
    },
    /// The watch dropped and the data has aged past its freshness budget.
    Stale {
        /// The last time the data was known current.
        last_current_at: DateTime<Utc>,
        /// How long ago that was, in seconds.
        age_seconds: i64,
    },
}

impl CollectionStatus {
    /// Whether an analyzer may treat this kind's absence as meaningful.
    ///
    /// [`InSync`](Self::InSync) qualifies because the watch is current.
    /// [`NotInstalled`](Self::NotInstalled) qualifies because a resource type
    /// that does not exist has exactly zero instances — that is a fact, not an
    /// absence of information.
    ///
    /// Every other status means "we do not know", and an analyzer that reads
    /// them as "there are none" is wrong.
    #[must_use]
    pub const fn is_authoritative(&self) -> bool {
        matches!(self, Self::InSync { .. } | Self::NotInstalled { .. })
    }

    /// A short human label for the interface.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Syncing => "syncing",
            Self::InSync { .. } => "in sync",
            Self::NotInstalled { .. } => "not installed",
            Self::Forbidden { .. } => "forbidden",
            Self::Degraded { .. } => "degraded",
            Self::Stale { .. } => "stale",
        }
    }
}

/// Per-kind collection coverage for a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindCoverage {
    /// The Kubernetes kind, for example `PodDisruptionBudget`.
    pub kind: String,
    /// How well it was observed.
    pub status: CollectionStatus,
    /// How many objects were collected.
    pub observed_count: usize,
}

/// A reference to a specific Kubernetes object, precise enough to look up.
///
/// `uid` and `resource_version` are what let a reader verify a finding against
/// the live cluster months later, so they travel with every affected resource.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceRef {
    /// Kind, for example `Node` or `PodDisruptionBudget`.
    pub kind: String,
    /// Namespace, absent for cluster-scoped objects.
    pub namespace: Option<String>,
    /// Object name.
    pub name: String,
    /// Object UID.
    pub uid: Option<String>,
    /// The resourceVersion FleetForge observed.
    pub resource_version: Option<String>,
}

impl ResourceRef {
    /// A stable sort and display key: `kind/namespace/name`.
    ///
    /// Used to order facts before hashing, so snapshot identity does not depend
    /// on the order a watch happened to deliver events.
    #[must_use]
    pub fn sort_key(&self) -> String {
        format!(
            "{}/{}/{}",
            self.kind,
            self.namespace.as_deref().unwrap_or(""),
            self.name
        )
    }
}

/// Where a single fact came from and what it may claim.
///
/// Fields are private. The only ways to build one are the validating
/// constructors below and validated deserialization, which is what makes
/// "fixture data cannot be emitted as live" a property of the type rather than
/// a rule someone has to remember.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ProvenanceRepr")]
pub struct Provenance {
    mode: Mode,
    cluster_id: String,
    namespace: Option<String>,
    uid: Option<String>,
    resource_version: Option<String>,
    observed_at: DateTime<Utc>,
    source: Source,
    collection: CollectionStatus,
}

/// Wire representation, used only so deserialization can be validated.
#[derive(Deserialize)]
struct ProvenanceRepr {
    mode: Mode,
    cluster_id: String,
    namespace: Option<String>,
    uid: Option<String>,
    resource_version: Option<String>,
    observed_at: DateTime<Utc>,
    source: Source,
    collection: CollectionStatus,
}

impl TryFrom<ProvenanceRepr> for Provenance {
    type Error = FleetForgeError;

    fn try_from(repr: ProvenanceRepr) -> Result<Self> {
        Self::new(
            repr.mode,
            repr.cluster_id,
            repr.namespace,
            repr.uid,
            repr.resource_version,
            repr.observed_at,
            repr.source,
            repr.collection,
        )
    }
}

impl Provenance {
    /// The validating constructor every other constructor routes through.
    ///
    /// # Errors
    ///
    /// Returns [`FleetForgeError::InconsistentProvenance`] if the source does
    /// not permit the claimed mode.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: Mode,
        cluster_id: String,
        namespace: Option<String>,
        uid: Option<String>,
        resource_version: Option<String>,
        observed_at: DateTime<Utc>,
        source: Source,
        collection: CollectionStatus,
    ) -> Result<Self> {
        if !source.permits(mode) {
            return Err(FleetForgeError::InconsistentProvenance {
                reason: format!(
                    "a {} cannot be presented as {}",
                    source.describe(),
                    mode.label()
                ),
            });
        }
        Ok(Self {
            mode,
            cluster_id,
            namespace,
            uid,
            resource_version,
            observed_at,
            source,
            collection,
        })
    }

    /// A fact observed live from a Kubernetes watch.
    #[must_use]
    pub fn live(
        cluster_id: impl Into<String>,
        group: impl Into<String>,
        version: impl Into<String>,
        kind: impl Into<String>,
        observed_at: DateTime<Utc>,
    ) -> Self {
        // A KubeWatch source always permits Live, so this cannot fail. The
        // fallible constructor stays the single validation point regardless.
        Self {
            mode: Mode::Live,
            cluster_id: cluster_id.into(),
            namespace: None,
            uid: None,
            resource_version: None,
            observed_at,
            source: Source::KubeWatch {
                group: group.into(),
                version: version.into(),
                kind: kind.into(),
            },
            collection: CollectionStatus::InSync {
                synced_at: observed_at,
            },
        }
    }

    /// A fact loaded from a fixture file. Always [`Mode::Fixture`].
    #[must_use]
    pub fn fixture(
        cluster_id: impl Into<String>,
        file: impl Into<String>,
        observed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            mode: Mode::Fixture,
            cluster_id: cluster_id.into(),
            namespace: None,
            uid: None,
            resource_version: None,
            observed_at,
            source: Source::Fixture { file: file.into() },
            collection: CollectionStatus::InSync {
                synced_at: observed_at,
            },
        }
    }

    /// A value FleetForge calculated. Always [`Mode::WhatIf`].
    ///
    /// This is the constructor every preflight result uses, which is why a
    /// preflight result cannot be presented as something that occurred.
    #[must_use]
    pub fn computed(
        cluster_id: impl Into<String>,
        from: SnapshotId,
        computed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            mode: Mode::WhatIf,
            cluster_id: cluster_id.into(),
            namespace: None,
            uid: None,
            resource_version: None,
            observed_at: computed_at,
            source: Source::Computed { from },
            collection: CollectionStatus::InSync {
                synced_at: computed_at,
            },
        }
    }

    /// Attach object identity to a fact's provenance.
    #[must_use]
    pub fn with_object(
        mut self,
        namespace: Option<String>,
        uid: Option<String>,
        resource_version: Option<String>,
    ) -> Self {
        self.namespace = namespace;
        self.uid = uid;
        self.resource_version = resource_version;
        self
    }

    /// Record how well the owning kind was collected.
    #[must_use]
    pub fn with_collection(mut self, collection: CollectionStatus) -> Self {
        self.collection = collection;
        self
    }

    /// What kind of data this is.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// The stable cluster identifier. Never an API server URL.
    #[must_use]
    pub fn cluster_id(&self) -> &str {
        &self.cluster_id
    }

    /// The object's namespace, if it has one.
    #[must_use]
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    /// The object's UID, if known.
    #[must_use]
    pub fn uid(&self) -> Option<&str> {
        self.uid.as_deref()
    }

    /// The observed resourceVersion, if known.
    #[must_use]
    pub fn resource_version(&self) -> Option<&str> {
        self.resource_version.as_deref()
    }

    /// When FleetForge saw this.
    #[must_use]
    pub const fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }

    /// Where it came from.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// How well the owning kind was collected.
    #[must_use]
    pub const fn collection(&self) -> &CollectionStatus {
        &self.collection
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn t() -> DateTime<Utc> {
        DateTime::from_timestamp(1_757_000_000, 0).unwrap()
    }

    #[test]
    fn fixture_source_cannot_claim_live() {
        let err = Provenance::new(
            Mode::Live,
            "c1".into(),
            None,
            None,
            None,
            t(),
            Source::Fixture {
                file: "two-node.json".into(),
            },
            CollectionStatus::InSync { synced_at: t() },
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FleetForgeError::InconsistentProvenance { .. }
        ));
    }

    #[test]
    fn computed_values_are_always_what_if() {
        let p = Provenance::computed("c1", SnapshotId::from_hex_unchecked("abc"), t());
        assert_eq!(p.mode(), Mode::WhatIf);
        assert!(!p.mode().describes_current_reality());
    }

    #[test]
    fn a_watch_cannot_claim_to_be_a_calculation() {
        assert!(
            !Source::KubeWatch {
                group: String::new(),
                version: "v1".into(),
                kind: "Node".into(),
            }
            .permits(Mode::WhatIf)
        );
    }

    #[test]
    fn deserialization_is_validated_not_trusted() {
        // A hand-edited event log claiming fixture data is live must be
        // rejected on the way in, not carried into the interface.
        let json = r#"{
            "mode": "live",
            "cluster_id": "c1",
            "namespace": null,
            "uid": null,
            "resource_version": null,
            "observed_at": "2026-09-04T12:53:20Z",
            "source": { "type": "fixture", "file": "two-node.json" },
            "collection": { "state": "in_sync", "synced_at": "2026-09-04T12:53:20Z" }
        }"#;
        let result: std::result::Result<Provenance, _> = serde_json::from_str(json);
        assert!(result.is_err(), "fixture data must not deserialize as live");
    }

    #[test]
    fn round_trip_preserves_a_valid_provenance() {
        let p = Provenance::live("c1", "", "v1", "Node", t()).with_object(
            None,
            Some("uid-1".into()),
            Some("456".into()),
        );
        let json = serde_json::to_string(&p).unwrap();
        let back: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn only_in_sync_is_authoritative() {
        assert!(CollectionStatus::InSync { synced_at: t() }.is_authoritative());
        assert!(
            !CollectionStatus::Forbidden {
                verb: "list".into(),
                resource: "poddisruptionbudgets".into(),
            }
            .is_authoritative()
        );
        assert!(!CollectionStatus::Syncing.is_authoritative());
    }
}
