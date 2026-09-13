//! The replay schema.
//!
//! Versioned, because a bundle captured today will be read by code written
//! later, and silently reinterpreting old evidence is how a report starts
//! saying something the evidence never said.
//!
//! # Claims
//!
//! The central type here is [`Claim`]. A leadership interface makes assertions,
//! and the difference between "FleetForge observed this", "this follows
//! arithmetically", "a human worked this out", and "this is a guess nobody has
//! checked" is the difference between evidence and storytelling. Every claim
//! carries its [`ClaimBasis`], and the interface renders them differently.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Schema version of the replay format.
///
/// Bumped when the meaning of a field changes. A bundle declaring a version
/// this build does not understand is rejected rather than guessed at.
pub const REPLAY_SCHEMA_VERSION: u32 = 1;

/// How much weight a statement can bear.
///
/// Ordered from strongest to weakest, and that ordering is deliberate: the
/// interface sorts by it, so the things FleetForge actually saw appear above
/// the things a human inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimBasis {
    /// FleetForge recorded this directly from the Kubernetes API. The strongest
    /// kind of statement available: a fact with a resourceVersion attached.
    ObservedByFleetForge,
    /// Arithmetic over observed values, reproducible by hand.
    MathematicallyDerived,
    /// A human read several observations together and drew a conclusion.
    /// FleetForge did not infer this and does not implement the inference.
    HumanRca,
    /// A plausible explanation that has not been tested. Stated so it can be
    /// argued with, not relied on.
    UnverifiedHypothesis,
    /// Something the evidence cannot answer. Recorded explicitly, because an
    /// absent field reads as "fine" and an explicit unknown does not.
    Unavailable,
}

impl ClaimBasis {
    /// A short label for the interface.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ObservedByFleetForge => "OBSERVED",
            Self::MathematicallyDerived => "DERIVED",
            Self::HumanRca => "HUMAN RCA",
            Self::UnverifiedHypothesis => "UNVERIFIED",
            // "NO EVIDENCE" rather than "UNKNOWN": the tag says why a thing
            // cannot be known, and sits next to values that are themselves
            // rendered UNKNOWN. Two UNKNOWNs in one tile read as a bug.
            Self::Unavailable => "NO EVIDENCE",
        }
    }

    /// Whether this basis is strong enough to state as fact without qualifying
    /// language. Only direct observation and reproducible arithmetic are.
    #[must_use]
    pub const fn is_evidence(self) -> bool {
        matches!(
            self,
            Self::ObservedByFleetForge | Self::MathematicallyDerived
        )
    }
}

/// One assertion the interface may display, with its support and its basis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// Stable identifier, so the interface can link to a specific claim.
    pub id: String,
    /// The assertion, in plain language.
    pub statement: String,
    /// How much weight it can bear.
    pub basis: ClaimBasis,
    /// Artifact names backing it. Each must exist in the bundle manifest.
    pub evidence: Vec<String>,
    /// What this claim explicitly does not establish.
    pub limitations: Vec<String>,
    /// When the underlying observation happened, where that is meaningful.
    pub at: Option<DateTime<Utc>>,
}

/// What kind of cluster produced this bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureContext {
    /// Stable cluster identifier from the capture.
    pub cluster_id: String,
    /// Human description of the environment.
    pub cluster_kind: String,
    /// Kubernetes server version reported at capture time.
    pub kubernetes_version: Option<String>,
    /// The Kubernetes API version the client bindings targeted.
    pub client_target_version: String,
    /// Bottlerocket releases seen during the capture, ascending.
    pub bottlerocket_versions: Vec<String>,
    /// When the capture began and ended.
    pub captured_from: DateTime<Utc>,
    /// When the capture ended.
    pub captured_to: DateTime<Utc>,
    /// Node names in the captured fleet.
    pub nodes: Vec<String>,
    /// The earliest Brupop activity visible anywhere in the bundle.
    ///
    /// Derived from Kubernetes events in the pre-recovery snapshot, not from a
    /// constant. It is the single most important number in the replay: it
    /// predates [`captured_from`](Self::captured_from), which is what makes
    /// "FleetForge did not predict this" a fact rather than modesty.
    pub brupop_first_seen_at: Option<DateTime<Utc>>,
}

/// A file in the bundle that a claim can point at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// The name a caller uses to request it. Never a path.
    pub name: String,
    /// What it contains, for a human.
    pub description: String,
    /// `json`, `jsonl`, `markdown`, or `text`.
    pub kind: ArtifactKind,
    /// Size on disk.
    pub bytes: u64,
    /// SHA-256 of the file as captured. Lets a reader prove the artifact they
    /// are reading is the one the claim was made against.
    pub sha256: String,
}

/// How an artifact should be rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// A single JSON document.
    Json,
    /// One JSON object per line.
    Jsonl,
    /// Markdown prose.
    Markdown,
    /// Plain text, usually a log.
    Text,
}

/// A data-quality caveat about the bundle itself.
///
/// Distinct from a claim limitation: this is about the evidence being imperfect,
/// not about what the evidence proves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataCaveat {
    /// Short identifier.
    pub id: String,
    /// What is wrong with the data.
    pub statement: String,
    /// Which artifacts or event ranges are affected.
    pub affects: Vec<String>,
}
