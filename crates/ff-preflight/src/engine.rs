//! Running the analyzers and assembling the result.
//!
//! Pure: `(ClusterSnapshot, MaintenanceRequest) -> PreflightResult`. No I/O, no
//! clock beyond what findings stamp for provenance, and deterministic for a
//! given snapshot (ADR-0016).

use ff_core::{
    Bytes, ClusterSnapshot, Confidence, ConstraintRef, Finding, FindingId, MaintenanceRequest,
    Millicores, PredictedImpact, PreflightResult, ResourceRef, Severity, SummaryInputs,
    WorkloadImpact,
};

use crate::analyzer::FindingBuilder;
use crate::analyzers;
use crate::context::AnalysisContext;

/// How many wave sizes the concurrency search will try.
///
/// The search is a pure function over an in-memory snapshot, so each step is
/// cheap — but it is still linear, and a request naming a hundred nodes should
/// not turn one click into a hundred full analyses.
const MAX_CONCURRENCY_SEARCH: u32 = 24;

/// Run preflight.
///
/// Analyzers whose inputs were not collected authoritatively are **not run**,
/// and their absence becomes a blocker rather than a silent pass. A check that
/// did not run has not cleared anything.
#[must_use]
pub fn run(snapshot: &ClusterSnapshot, request: &MaintenanceRequest) -> PreflightResult {
    let now = chrono::Utc::now();
    let ctx = AnalysisContext::new(snapshot, request);
    let mut findings = analyze_wave(&ctx, now);

    // A node the operator named that FleetForge cannot see is reported, never
    // quietly dropped from the set being analyzed.
    for name in &ctx.unknown_nodes {
        findings.push(
            FindingBuilder::new(
                "FF-REQUEST-001",
                Severity::Blocker,
                Confidence::Certain,
                format!("Node {name} is not present in this snapshot"),
            )
            .explaining(format!(
                "The request names {name}, which does not appear in the cluster state FleetForge \
                 collected. It may have been deleted, renamed, or never existed. Nothing is \
                 concluded about it — analyzing the rest of the selection as if this node were \
                 not part of the request would answer a different question from the one asked."
            ))
            .remediation("Re-run against a current snapshot", None, None)
            .limitation(
                "A node missing from the snapshot may still exist in the cluster if collection is \
                 incomplete. Check the collection status before concluding it is gone."
                    .to_owned(),
            )
            .build(snapshot.cluster_id(), snapshot.snapshot_id(), now),
        );
    }

    let blocked = findings.iter().any(Finding::is_blocker);
    let (recommended, constraint) = if blocked {
        (0, blocking_constraint(&findings))
    } else {
        recommend_concurrency(snapshot, request, now)
    };

    PreflightResult::new(
        snapshot.cluster_id(),
        request.clone(),
        findings,
        now,
        SummaryInputs {
            recommended_max_concurrency: recommended,
            concurrency_constraint: constraint,
            affected_workloads: workload_impacts(&ctx),
            predicted_impact: predicted_impact(&ctx),
        },
    )
}

/// Run every analyzer against one wave.
fn analyze_wave(ctx: &AnalysisContext<'_>, now: chrono::DateTime<chrono::Utc>) -> Vec<Finding> {
    let snapshot = ctx.snapshot;
    let mut findings = Vec::new();

    for analyzer in analyzers::all() {
        // Gate on coverage first. An analyzer that cannot see its inputs must
        // not run, and its silence must not read as a clean result.
        let missing: Vec<&str> = analyzer
            .required_kinds()
            .iter()
            .filter(|kind| snapshot.require_authoritative(kind).is_err())
            .copied()
            .collect();

        if !missing.is_empty() {
            let reasons: Vec<String> = missing
                .iter()
                .map(|kind| {
                    let status = snapshot
                        .coverage()
                        .iter()
                        .find(|c| c.kind == *kind)
                        .map_or("not collected".to_owned(), |c| c.status.label().to_owned());
                    format!("{kind} ({status})")
                })
                .collect();

            findings.push(
                FindingBuilder::new(
                    "FF-COVERAGE-001",
                    Severity::Blocker,
                    Confidence::Certain,
                    format!("Cannot evaluate: {}", analyzer.describes()),
                )
                .explaining(format!(
                    "This check needs {} and did not get it, so it did not run. Its silence is \
                     not a pass. Treating an unrun check as a clear result is how a tool reports \
                     'safe' about something it never looked at — for a PodDisruptionBudget check \
                     in particular, an empty list reads as 'no blockers'.",
                    reasons.join(", ")
                ))
                .remediation(
                    "Grant the missing read permission, or wait for collection to complete",
                    None,
                    None,
                )
                .limitation(
                    "This finding reports that an analysis is missing. It says nothing about \
                     whether the maintenance is in fact safe."
                        .to_owned(),
                )
                .build(snapshot.cluster_id(), snapshot.snapshot_id(), now),
            );
            continue;
        }

        match analyzer.analyze(ctx) {
            Ok(mut produced) => findings.append(&mut produced),
            Err(err) => findings.push(
                FindingBuilder::new(
                    "FF-ANALYZER-001",
                    Severity::Blocker,
                    Confidence::Certain,
                    format!("Analyzer {} failed", analyzer.id_prefix()),
                )
                .explaining(format!(
                    "The check for {} could not complete: {err}. As with missing coverage, a \
                     failed analysis is not a passed one.",
                    analyzer.describes()
                ))
                .limitation("No conclusion is available from this check.".to_owned())
                .build(snapshot.cluster_id(), snapshot.snapshot_id(), now),
            ),
        }
    }

    findings
}

/// Find the largest wave size that produces no blocker.
///
/// A simple downward search rather than anything clever: run the analysis at
/// each size and take the first that comes back clean. The definition of
/// "recommended concurrency" is therefore exactly "the largest number of nodes
/// we can take at once without a blocker" — which is checkable by re-running
/// preflight at that number, rather than a heuristic the operator has to trust.
fn recommend_concurrency(
    snapshot: &ClusterSnapshot,
    request: &MaintenanceRequest,
    now: chrono::DateTime<chrono::Utc>,
) -> (u32, Option<ConstraintRef>) {
    let total = u32::try_from(request.node_names.len()).unwrap_or(u32::MAX);
    // MAX_CONCURRENCY_SEARCH is a non-zero constant, so the bounds cannot invert.
    let ceiling = total.clamp(1, MAX_CONCURRENCY_SEARCH);

    let mut best = 0;
    let mut first_blocker: Option<Finding> = None;

    for k in 1..=ceiling {
        let ctx = AnalysisContext::with_concurrency(snapshot, request, k);
        let findings = analyze_wave(&ctx, now);
        match findings.iter().find(|f| f.is_blocker()) {
            None => best = k,
            Some(blocker) => {
                // The first size that fails is what limits concurrency, and its
                // finding is the honest answer to "why that number?".
                first_blocker = Some(blocker.clone());
                break;
            }
        }
    }

    let constraint = first_blocker.map(|f| ConstraintRef {
        finding_id: f.id.clone(),
        resource: f.affected.first().cloned().unwrap_or(ResourceRef {
            kind: "Cluster".into(),
            namespace: None,
            name: snapshot.cluster_id().to_owned(),
            uid: None,
            resource_version: None,
        }),
        reason: format!(
            "taking {} nodes at once is blocked by: {}",
            best.saturating_add(1),
            f.title
        ),
    });

    (best, constraint)
}

/// Which finding is responsible for a blocked result.
fn blocking_constraint(findings: &[Finding]) -> Option<ConstraintRef> {
    findings
        .iter()
        .find(|f| f.is_blocker())
        .map(|f| ConstraintRef {
            finding_id: FindingId::new(f.id.as_str()),
            resource: f.affected.first().cloned().unwrap_or(ResourceRef {
                kind: "Unknown".into(),
                namespace: None,
                name: "unknown".into(),
                uid: None,
                resource_version: None,
            }),
            reason: format!("blocked: {}", f.title),
        })
}

/// Per-workload predicted disruption.
fn workload_impacts(ctx: &AnalysisContext<'_>) -> Vec<WorkloadImpact> {
    use std::collections::BTreeMap;
    let mut by_workload: BTreeMap<(String, String), WorkloadImpact> = BTreeMap::new();

    for pod in ctx.evicted_pods() {
        let Some(workload) = ctx.workload_for(pod) else {
            continue;
        };
        let entry = by_workload
            .entry((workload.namespace.clone(), workload.name.clone()))
            .or_insert_with(|| WorkloadImpact {
                workload: workload.resource_ref(),
                pods_on_selected_nodes: 0,
                ready_replicas: workload.ready_replicas,
                desired_replicas: workload.desired_replicas,
                blocked_by_pdb: false,
                has_immovable_pods: false,
            });
        entry.pods_on_selected_nodes = entry.pods_on_selected_nodes.saturating_add(1);
        if ctx.pdbs_for(pod).iter().any(|p| p.blocks_disruption()) {
            entry.blocked_by_pdb = true;
        }
        if pod.is_node_bound() {
            entry.has_immovable_pods = true;
        }
    }

    by_workload.into_values().collect()
}

/// Aggregate predicted impact of the wave.
fn predicted_impact(ctx: &AnalysisContext<'_>) -> PredictedImpact {
    let evicted = ctx.evicted_pods();
    let rescheduling = ctx.rescheduling_pods();
    let cpu_needed = ctx.cpu_to_reschedule();
    let mem_needed = ctx.memory_to_reschedule();

    let cpu_free = ctx
        .remaining_allocatable_cpu()
        .saturating_sub(ctx.cpu_already_used_on_remaining());
    let mem_free = ctx
        .remaining_allocatable_memory()
        .saturating_sub(ctx.memory_already_used_on_remaining());

    let minimum_pdb_margin = evicted
        .iter()
        .flat_map(|p| ctx.pdbs_for(p))
        .map(|pdb| pdb.disruptions_allowed)
        .min()
        .unwrap_or(i32::MAX);

    PredictedImpact {
        pods_evicted: u32::try_from(evicted.len()).unwrap_or(u32::MAX),
        pods_not_rescheduled: u32::try_from(evicted.len().saturating_sub(rescheduling.len()))
            .unwrap_or(0),
        cpu_to_reschedule: cpu_needed,
        memory_to_reschedule: mem_needed,
        cpu_headroom_after: Millicores(cpu_free.0.saturating_sub(cpu_needed.0)),
        memory_headroom_after: Bytes(mem_free.0.saturating_sub(mem_needed.0)),
        minimum_pdb_margin: if minimum_pdb_margin == i32::MAX {
            0
        } else {
            minimum_pdb_margin
        },
    }
}
