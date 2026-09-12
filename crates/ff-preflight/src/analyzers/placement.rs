//! Placement-constraint analysis: node selectors, taints, and required node
//! affinity.
//!
//! These are `Likely`, not `Certain`. Each reproduces one scheduler predicate
//! faithfully, and if no remaining node satisfies it the pod genuinely cannot
//! be placed. But the scheduler evaluates *all* predicates plus plugins the
//! cluster may have enabled, so a pod passing this check can still fail to
//! schedule for a reason FleetForge never saw.

use std::collections::BTreeMap;

use ff_core::{
    Confidence, Finding, NodeFact, NodeSelectorOperator, NodeSelectorTerm, PodFact, Result,
    Severity, TaintEffect, Toleration, TolerationOperator,
};

use crate::analyzer::{Analyzer, FindingBuilder};
use crate::context::AnalysisContext;

const PREDICATE_LIMITATION: &str = "Reproduces one scheduler predicate. The scheduler evaluates every predicate plus any plugins \
     this cluster has enabled, so passing this check is not proof that a pod will schedule.";

/// Whether a node's labels satisfy a plain node selector.
fn selector_matches(node: &NodeFact, selector: &BTreeMap<String, String>) -> bool {
    selector
        .iter()
        .all(|(k, v)| node.labels.get(k).is_some_and(|actual| actual == v))
}

/// Whether a node satisfies one required node-affinity term.
///
/// Requirements within a term are ANDed; terms are ORed. `matchFields` is
/// treated as unsatisfiable rather than guessed at, because the only field
/// selector Kubernetes supports here is `metadata.name` and quietly ignoring
/// the rest would turn an unsatisfiable constraint into a satisfied one.
fn affinity_term_matches(node: &NodeFact, term: &NodeSelectorTerm) -> bool {
    let expressions_ok = term.match_expressions.iter().all(|req| {
        let actual = node.labels.get(&req.key);
        match req.operator {
            NodeSelectorOperator::In => actual.is_some_and(|v| req.values.contains(v)),
            NodeSelectorOperator::NotIn => actual.is_none_or(|v| !req.values.contains(v)),
            NodeSelectorOperator::Exists => actual.is_some(),
            NodeSelectorOperator::DoesNotExist => actual.is_none(),
            NodeSelectorOperator::Gt => actual
                .and_then(|v| v.parse::<i64>().ok())
                .zip(req.values.first().and_then(|v| v.parse::<i64>().ok()))
                .is_some_and(|(a, b)| a > b),
            NodeSelectorOperator::Lt => actual
                .and_then(|v| v.parse::<i64>().ok())
                .zip(req.values.first().and_then(|v| v.parse::<i64>().ok()))
                .is_some_and(|(a, b)| a < b),
        }
    });

    let fields_ok = term.match_fields.iter().all(|req| {
        if req.key == "metadata.name" {
            match req.operator {
                NodeSelectorOperator::In => req.values.contains(&node.name),
                NodeSelectorOperator::NotIn => !req.values.contains(&node.name),
                _ => false,
            }
        } else {
            false
        }
    });

    expressions_ok && fields_ok
}

/// Whether a toleration matches a specific taint.
fn tolerates(toleration: &Toleration, key: &str, value: Option<&str>, effect: TaintEffect) -> bool {
    // An absent effect on a toleration matches every effect.
    if toleration
        .effect
        .is_some_and(|tol_effect| tol_effect != effect)
    {
        return false;
    }
    match (&toleration.key, toleration.operator) {
        // An empty key with Exists tolerates everything, which is how
        // cluster-critical DaemonSets run on tainted nodes.
        (None, TolerationOperator::Exists) => true,
        (Some(k), TolerationOperator::Exists) => k == key,
        (Some(k), TolerationOperator::Equal) => k == key && toleration.value.as_deref() == value,
        (None, TolerationOperator::Equal) => false,
    }
}

/// Whether a pod tolerates every scheduling-blocking taint on a node.
fn tolerates_node(pod: &PodFact, node: &NodeFact) -> bool {
    node.taints
        .iter()
        .filter(|t| matches!(t.effect, TaintEffect::NoSchedule | TaintEffect::NoExecute))
        .all(|taint| {
            pod.tolerations
                .iter()
                .any(|tol| tolerates(tol, &taint.key, taint.value.as_deref(), taint.effect))
        })
}

/// Nodes that remain in service and can accept new pods.
fn candidates<'a>(ctx: &AnalysisContext<'a>) -> Vec<&'a NodeFact> {
    ctx.remaining
        .iter()
        .filter(|n| !n.unschedulable && n.is_ready())
        .copied()
        .collect()
}

/// Detects pods pinned by a node selector to nodes that will not remain.
pub struct NodeSelectorAnalyzer;

impl Analyzer for NodeSelectorAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-SELECTOR"
    }

    fn describes(&self) -> &'static str {
        "pods whose nodeSelector no remaining node satisfies"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Node", "Pod"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();
        let remaining = candidates(ctx);
        let mut findings = Vec::new();

        for pod in ctx.rescheduling_pods() {
            if pod.node_selector.is_empty() {
                continue;
            }
            if remaining
                .iter()
                .any(|n| selector_matches(n, &pod.node_selector))
            {
                continue;
            }

            let selector_text = pod
                .node_selector
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", ");

            findings.push(
                FindingBuilder::new(
                    "FF-SELECTOR-001",
                    Severity::Blocker,
                    Confidence::Likely,
                    format!(
                        "Pod {}/{} has a nodeSelector no remaining node satisfies",
                        pod.namespace, pod.name
                    ),
                )
                .affecting([pod.resource_ref()])
                .evidence(
                    pod.resource_ref(),
                    ".spec.nodeSelector",
                    &selector_text,
                    Some("every label must match for a node to be eligible"),
                )
                .calculation(
                    vec![
                        (
                            "schedulable nodes remaining".into(),
                            remaining.len().to_string(),
                        ),
                        ("of those, matching the selector".into(), "0".into()),
                    ],
                    "blocked when no remaining schedulable node matches every selector label",
                    "0 eligible nodes",
                    None,
                )
                .explaining(format!(
                    "This pod requires a node labelled {selector_text}. Of the {} schedulable \
                     nodes that would remain, none carries those labels, so the pod will stay \
                     Pending after eviction.",
                    remaining.len()
                ))
                .remediation(
                    "Label another node to match, or exclude this pod's node from the maintenance",
                    None,
                    Some("labelling a node makes it eligible for every pod with that selector"),
                )
                .limitation(PREDICATE_LIMITATION.to_owned())
                .limitation(
                    "Cluster autoscaling and Karpenter are not modelled. A provisioner able to \
                     create a matching node would resolve this without intervention."
                        .to_owned(),
                )
                .build(cluster, snapshot_id, now),
            );
        }

        Ok(findings)
    }
}

/// Detects pods that tolerate no remaining node's taints.
pub struct TolerationAnalyzer;

impl Analyzer for TolerationAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-TOLERATION"
    }

    fn describes(&self) -> &'static str {
        "pods that lack tolerations for every remaining node's taints"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Node", "Pod"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();
        let remaining = candidates(ctx);
        let mut findings = Vec::new();

        for pod in ctx.rescheduling_pods() {
            if remaining.iter().any(|n| tolerates_node(pod, n)) {
                continue;
            }

            let blocking: Vec<String> = remaining
                .iter()
                .flat_map(|n| {
                    n.taints
                        .iter()
                        .filter(|t| {
                            matches!(t.effect, TaintEffect::NoSchedule | TaintEffect::NoExecute)
                        })
                        .map(move |t| {
                            format!(
                                "{}={}:{:?} on {}",
                                t.key,
                                t.value.as_deref().unwrap_or(""),
                                t.effect,
                                n.name
                            )
                        })
                })
                .collect();

            let mut builder = FindingBuilder::new(
                "FF-TOLERATION-001",
                Severity::Blocker,
                Confidence::Likely,
                format!(
                    "Pod {}/{} tolerates none of the remaining nodes",
                    pod.namespace, pod.name
                ),
            )
            .affecting([pod.resource_ref()])
            .calculation(
                vec![
                    (
                        "schedulable nodes remaining".into(),
                        remaining.len().to_string(),
                    ),
                    ("of those, whose taints are tolerated".into(), "0".into()),
                ],
                "blocked when every remaining node carries a NoSchedule or NoExecute taint the \
                 pod does not tolerate",
                "0 eligible nodes",
                None,
            )
            .explaining(format!(
                "Every one of the {} schedulable nodes that would remain carries a taint this pod \
                 does not tolerate, so it will stay Pending after eviction. Taints in the way: {}.",
                remaining.len(),
                if blocking.is_empty() {
                    "(none recorded)".to_owned()
                } else {
                    blocking.join("; ")
                }
            ))
            .remediation(
                "Add a matching toleration to the pod's controller",
                None,
                Some("tolerations are broad: the pod becomes eligible for every node with that taint"),
            )
            .remediation(
                "Remove the taint from a node that should accept this workload",
                None,
                Some("affects every pod's eligibility for that node, not just this one"),
            )
            .limitation(PREDICATE_LIMITATION.to_owned())
            .limitation(
                "PreferNoSchedule taints are ignored: they influence scoring, not admission, and \
                 cannot make a pod unschedulable."
                    .to_owned(),
            );

            for taint in remaining
                .iter()
                .flat_map(|n| n.taints.iter().map(move |t| (n, t)))
            {
                builder = builder.evidence(
                    taint.0.resource_ref(),
                    ".spec.taints",
                    format!("{}={:?}", taint.1.key, taint.1.effect),
                    None,
                );
            }

            findings.push(builder.build(cluster, snapshot_id, now));
        }

        Ok(findings)
    }
}

/// Detects pods whose required node affinity no remaining node satisfies.
pub struct NodeAffinityAnalyzer;

impl Analyzer for NodeAffinityAnalyzer {
    fn id_prefix(&self) -> &'static str {
        "FF-AFFINITY"
    }

    fn describes(&self) -> &'static str {
        "pods whose required node affinity no remaining node satisfies"
    }

    fn required_kinds(&self) -> &'static [&'static str] {
        &["Node", "Pod"]
    }

    fn analyze(&self, ctx: &AnalysisContext<'_>) -> Result<Vec<Finding>> {
        let now = chrono::Utc::now();
        let cluster = ctx.snapshot.cluster_id();
        let snapshot_id = ctx.snapshot.snapshot_id();
        let remaining = candidates(ctx);
        let mut findings = Vec::new();

        for pod in ctx.rescheduling_pods() {
            if pod.required_node_affinity.is_empty() {
                continue;
            }
            // Terms are ORed: one satisfied term is enough.
            let placeable = remaining.iter().any(|node| {
                pod.required_node_affinity
                    .iter()
                    .any(|term| affinity_term_matches(node, term))
            });
            if placeable {
                continue;
            }

            let terms = pod
                .required_node_affinity
                .iter()
                .map(|t| {
                    t.match_expressions
                        .iter()
                        .map(|r| format!("{} {:?} {:?}", r.key, r.operator, r.values))
                        .collect::<Vec<_>>()
                        .join(" AND ")
                })
                .collect::<Vec<_>>()
                .join(" OR ");

            findings.push(
                FindingBuilder::new(
                    "FF-AFFINITY-001",
                    Severity::Blocker,
                    Confidence::Likely,
                    format!(
                        "Pod {}/{} has required node affinity no remaining node satisfies",
                        pod.namespace, pod.name
                    ),
                )
                .affecting([pod.resource_ref()])
                .evidence(
                    pod.resource_ref(),
                    ".spec.affinity.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution",
                    &terms,
                    Some("terms are ORed; requirements within a term are ANDed"),
                )
                .calculation(
                    vec![
                        (
                            "schedulable nodes remaining".into(),
                            remaining.len().to_string(),
                        ),
                        ("of those, satisfying any term".into(), "0".into()),
                    ],
                    "blocked when no remaining schedulable node satisfies any required term",
                    "0 eligible nodes",
                    None,
                )
                .explaining(format!(
                    "This pod's required node affinity is: {terms}. None of the {} schedulable \
                     nodes that would remain satisfies it, so the pod will stay Pending after \
                     eviction. Note that 'IgnoredDuringExecution' means the running pod is \
                     unaffected — the constraint only applies when it is rescheduled, which is \
                     exactly what a drain forces.",
                    remaining.len()
                ))
                .remediation(
                    "Label a remaining node to satisfy the affinity, or exclude this pod's node",
                    None,
                    None,
                )
                .limitation(PREDICATE_LIMITATION.to_owned())
                .limitation(
                    "Preferred affinity is deliberately not evaluated: it affects scoring, not \
                     admission, and cannot make a pod unschedulable."
                        .to_owned(),
                )
                .limitation(
                    "matchFields is only understood for metadata.name. Any other field selector \
                     is treated as unsatisfiable rather than assumed satisfied."
                        .to_owned(),
                )
                .build(cluster, snapshot_id, now),
            );
        }

        Ok(findings)
    }
}
