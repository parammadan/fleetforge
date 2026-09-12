//! Evidence-based preflight analysis.
//!
//! Pure functions over a [`ClusterSnapshot`](ff_core::ClusterSnapshot): no I/O,
//! no Kubernetes client, no clock beyond stamping provenance. That is what
//! makes the analysis deterministic, exhaustively testable against fixtures,
//! and developable without a cluster on an 8 GB machine.
//!
//! # What this is not
//!
//! Version one does not simulate the Kubernetes scheduler and does not claim
//! to. Each analyzer declares what it proves and what it does not, and every
//! finding it emits carries those limitations (ADR-0004). An aggregate capacity
//! check proves insufficiency and never proves sufficiency, and it says so in
//! its own output.
//!
//! # Refusing to guess
//!
//! An analyzer whose inputs were not collected authoritatively does not run,
//! and its absence becomes a blocker. A check that did not run has cleared
//! nothing — and for PodDisruptionBudgets in particular, an unseen list reads
//! as "no blockers", which reads as safe to drain.

pub mod analyzer;
pub mod analyzers;
pub mod context;
pub mod engine;

pub use analyzer::{Analyzer, FindingBuilder};
pub use context::AnalysisContext;
pub use engine::run;
