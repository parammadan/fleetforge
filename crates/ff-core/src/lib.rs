//! FleetForge domain model.
//!
//! This crate holds types and nothing else. It performs no I/O, constructs no
//! Kubernetes client, and spawns no tasks. That restriction is what lets the
//! analysis engine be tested exhaustively without a cluster, and it is enforced
//! by the dependency graph rather than by convention (see ADR-0008).
//!
//! # The honesty guarantee
//!
//! Every fact carries [`Provenance`], and every [`Provenance`] carries a
//! [`Mode`]. Neither has a default value and neither has an "unknown" variant.
//! A fact that cannot say where it came from does not compile.
//!
//! [`Provenance`] is additionally validated at construction *and* at
//! deserialization: fixture-sourced data can never carry [`Mode::Live`], and
//! computed data can never claim to have been observed. See ADR-0003.
//!
//! # No floating point
//!
//! CPU is [`Millicores`] and memory is [`Bytes`], both integers. A snapshot
//! identifier is a content hash that must be byte-identical on `aarch64` and
//! `x86_64`; floating point would put that at risk for no benefit, so
//! `clippy::float_arithmetic` is denied workspace-wide.

mod canonical;
mod error;
mod finding;
mod provenance;
mod quantity;
mod recommendation;
mod snapshot;

pub use error::{FleetForgeError, Result};
pub use finding::{Calculation, Confidence, Evidence, Finding, FindingId, Remediation, Severity};
pub use provenance::{CollectionStatus, KindCoverage, Mode, Provenance, ResourceRef, Source};
pub use quantity::{Bytes, Millicores};
pub use recommendation::{
    ConstraintRef, MaintenanceRequest, MaintenanceStatus, PredictedImpact, PreflightResult,
    RecommendationSummary, SummaryInputs, WorkloadImpact,
};
pub use snapshot::{
    BrupopFact, ClusterSnapshot, ConditionStatus, ContainerFact, EventFact, LabelSelector,
    LabelSelectorOperator, LabelSelectorRequirement, NodeCondition, NodeFact, NodeSelectorOperator,
    NodeSelectorRequirement, NodeSelectorTerm, PdbFact, PodAffinityTerm, PodFact, PodPhase,
    SnapshotFacts, SnapshotId, Taint, TaintEffect, Toleration, TolerationOperator,
    TopologySpreadConstraint, UnsatisfiableAction, VolumeFact, VolumeKind, WorkloadFact,
    WorkloadKind,
};
