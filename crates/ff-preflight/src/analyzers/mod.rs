//! The analyzer registry.
//!
//! Each analyzer is independent and self-registering here. Adding a check never
//! requires editing an existing one, which is the property that keeps this
//! growing safely.

pub mod capacity;
pub mod pdb;
pub mod placement;
pub mod storage;
pub mod topology;
pub mod workload;

use crate::analyzer::Analyzer;

/// Every analyzer, in a stable order.
///
/// Order does not affect results — findings are sorted by severity before they
/// are returned — but a stable order keeps output diffable between runs.
#[must_use]
pub fn all() -> Vec<Box<dyn Analyzer>> {
    vec![
        Box::new(pdb::PdbAnalyzer),
        Box::new(workload::SingletonAnalyzer),
        Box::new(workload::UnmanagedPodAnalyzer),
        Box::new(capacity::CpuCapacityAnalyzer),
        Box::new(capacity::MemoryCapacityAnalyzer),
        Box::new(placement::NodeSelectorAnalyzer),
        Box::new(placement::TolerationAnalyzer),
        Box::new(placement::NodeAffinityAnalyzer),
        Box::new(storage::NodeBoundStorageAnalyzer),
        Box::new(topology::AvailabilityZoneAnalyzer),
    ]
}
