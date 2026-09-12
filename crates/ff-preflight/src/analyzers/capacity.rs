//! Aggregate capacity analysis.
//!
//! These are the checks most likely to be misread, so the limitations are
//! stated at length and on every finding.
//!
//! Aggregate capacity is a **necessary but not sufficient** condition. If the
//! sum of remaining allocatable is less than the sum of what must be
//! rescheduled, the pods definitely will not all fit — that direction is sound.
//! The converse is not: enough total capacity says nothing about whether any
//! *individual* node can hold any *individual* pod. Ten nodes with 1 core free
//! each cannot run one pod requesting 4 cores, and this check cannot see that
//! (ADR-0004).

use ff_core::{Bytes, Confidence, Finding, Millicores, Result, Severity};

use crate::analyzer::{Analyzer, FindingBuilder};
use crate::context::AnalysisContext;

/// The bin-packing limitation, written once and attached to every finding here.
const PACKING_LIMITATION: &str = "This is an aggregate sum, not a scheduling decision. Sufficient total capacity does not \
     prove any individual pod fits on any individual node: ten nodes with 1 core free each \
     cannot place a pod requesting 4 cores. Only the scheduler can answer that.";

const PREDICATE_LIMITATION: &str = "Scheduler predicates are not evaluated. Node selectors, affinity, taints, topology spread, \
     and volume topology can each prevent placement regardless of free capacity. Those are \
     covered by separate analyzers, which have their own limitations.";

const REQUEST_LIMITATION: &str = "Uses resource *requests*, which is what the scheduler uses. A pod with no request counts as \
     zero here and can still consume a node's actual CPU and memory at runtime.";

/// Checks aggregate CPU after the selected nodes are removed.
pub struct CpuCapacityAnalyzer;

impl Analyzer for CpuCapacityAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-CPU"
    }

    fn describes(&self) -> &'static str {
        "whether remaining nodes have enough aggregate CPU to absorb the evicted pods"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Node", "Pod", "DaemonSet"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();

        if ctx.selected.is_empty() {
            return Ok(Vec::new());
        }

        let needed = ctx.cpu_to_reschedule();
        let allocatable = ctx.remaining_allocatable_cpu();
        let already_used = ctx.cpu_already_used_on_remaining();
        let free = allocatable.saturating_sub(already_used);
        let shortfall = needed.saturating_sub(free);

        let inputs = vec![
            (
                "allocatable on remaining schedulable nodes".into(),
                allocatable.to_string(),
            ),
            ("already requested there".into(), already_used.to_string()),
            ("free".into(), free.to_string()),
            ("needed by evicted pods".into(), needed.to_string()),
        ];

        let mut builder = if shortfall > Millicores::ZERO {
            FindingBuilder::new(
                "FF-CPU-001",
                Severity::Blocker,
                Confidence::Heuristic,
                "Not enough aggregate CPU after removing the selected nodes",
            )
            .explaining(format!(
                "The evicted pods request {needed} of CPU, but only {free} is free across the \
                 nodes that would remain — a shortfall of {shortfall}. Some pods will stay \
                 Pending. This is a sum, so it proves insufficiency; it would not have proven \
                 sufficiency."
            ))
            .remediation(
                "Add capacity before the maintenance, or reduce the number of nodes taken at once",
                None,
                Some("added nodes cost money for the duration"),
            )
        } else {
            FindingBuilder::new(
                "FF-CPU-002",
                Severity::Info,
                Confidence::Heuristic,
                "Aggregate CPU is sufficient after removing the selected nodes",
            )
            .explaining(format!(
                "The evicted pods request {needed} of CPU and {free} is free across the remaining \
                 nodes. This clears the aggregate check only — it is a necessary condition, not a \
                 guarantee that the scheduler can place every pod."
            ))
        };

        builder = builder
            .affecting(ctx.selected.iter().map(|n| n.resource_ref()))
            .calculation(
                inputs,
                "free = allocatable - alreadyRequested; shortfall = needed - free",
                if shortfall > Millicores::ZERO {
                    format!("short by {shortfall}")
                } else {
                    format!("{} spare", free.saturating_sub(needed))
                },
                Some("millicores"),
            )
            .limitation(PACKING_LIMITATION.to_owned())
            .limitation(PREDICATE_LIMITATION.to_owned())
            .limitation(REQUEST_LIMITATION.to_owned())
            .limitation(
                "Cordoned and not-Ready nodes are excluded from remaining capacity, since they \
                 accept no new pods."
                    .to_owned(),
            )
            .limitation(
                "Cluster autoscaling is not modelled. A cluster that would add nodes on demand \
                 may absorb this fine."
                    .to_owned(),
            );

        for node in &ctx.selected {
            builder = builder.evidence(
                node.resource_ref(),
                ".status.allocatable.cpu",
                node.allocatable_cpu,
                Some("leaving the fleet"),
            );
        }

        Ok(vec![builder.build(cluster, snapshot_id, now)])
    }
}

/// Checks aggregate memory after the selected nodes are removed.
pub struct MemoryCapacityAnalyzer;

impl Analyzer for MemoryCapacityAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-MEM"
    }

    fn describes(&self) -> &'static str {
        "whether remaining nodes have enough aggregate memory to absorb the evicted pods"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Node", "Pod", "DaemonSet"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();

        if ctx.selected.is_empty() {
            return Ok(Vec::new());
        }

        let needed = ctx.memory_to_reschedule();
        let allocatable = ctx.remaining_allocatable_memory();
        let already_used = ctx.memory_already_used_on_remaining();
        let free = allocatable.saturating_sub(already_used);
        let shortfall = needed.saturating_sub(free);

        let inputs = vec![
            (
                "allocatable on remaining schedulable nodes".into(),
                allocatable.to_string(),
            ),
            ("already requested there".into(), already_used.to_string()),
            ("free".into(), free.to_string()),
            ("needed by evicted pods".into(), needed.to_string()),
        ];

        let mut builder = if shortfall > Bytes::ZERO {
            FindingBuilder::new(
                "FF-MEM-001",
                Severity::Blocker,
                Confidence::Heuristic,
                "Not enough aggregate memory after removing the selected nodes",
            )
            .explaining(format!(
                "The evicted pods request {needed} of memory, but only {free} is free across the \
                 nodes that would remain — a shortfall of {shortfall}. Some pods will stay \
                 Pending. Memory is the harder of the two: unlike CPU it cannot be \
                 oversubscribed, so a shortfall here is not absorbed by throttling."
            ))
            .remediation(
                "Add capacity before the maintenance, or take fewer nodes at once",
                None,
                Some("added nodes cost money for the duration"),
            )
        } else {
            FindingBuilder::new(
                "FF-MEM-002",
                Severity::Info,
                Confidence::Heuristic,
                "Aggregate memory is sufficient after removing the selected nodes",
            )
            .explaining(format!(
                "The evicted pods request {needed} of memory and {free} is free across the \
                 remaining nodes. Aggregate check only."
            ))
        };

        builder = builder
            .affecting(ctx.selected.iter().map(|n| n.resource_ref()))
            .calculation(
                inputs,
                "free = allocatable - alreadyRequested; shortfall = needed - free",
                if shortfall > Bytes::ZERO {
                    format!("short by {shortfall}")
                } else {
                    format!("{} spare", free.saturating_sub(needed))
                },
                Some("bytes"),
            )
            .limitation(PACKING_LIMITATION.to_owned())
            .limitation(PREDICATE_LIMITATION.to_owned())
            .limitation(REQUEST_LIMITATION.to_owned())
            .limitation(
                "Memory cannot be oversubscribed the way CPU can. A pod exceeding its limit is \
                 killed, not throttled, so headroom here is less forgiving than it looks."
                    .to_owned(),
            );

        for node in &ctx.selected {
            builder = builder.evidence(
                node.resource_ref(),
                ".status.allocatable.memory",
                node.allocatable_memory,
                Some("leaving the fleet"),
            );
        }

        Ok(vec![builder.build(cluster, snapshot_id, now)])
    }
}
