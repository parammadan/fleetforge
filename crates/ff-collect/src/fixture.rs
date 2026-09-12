//! Loading recorded cluster state from disk.
//!
//! Everything here produces `Mode::Fixture`. These files were captured from a
//! real cluster, and that does not make them live — provenance describes where
//! a fact is being read from now (ADR-0020). The guarantee is structural:
//! `Origin::Fixture` builds `Source::Fixture`, which permits only
//! `Mode::Fixture`, checked at construction and again at deserialization.

use std::path::{Path, PathBuf};

use chrono::Utc;
use ff_core::{ClusterSnapshot, CollectionStatus, KindCoverage, Mode};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::core::v1::{Event, Node, Pod};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use kube::api::DynamicObject;
use serde::de::DeserializeOwned;

use crate::error::CollectError;
use crate::normalize::{self, NormalizeContext, Origin};

/// Loads a snapshot from a directory of captured Kubernetes list responses.
#[derive(Clone, Debug)]
pub struct FixtureSource {
    dir: PathBuf,
    cluster_id: String,
}

impl FixtureSource {
    /// Read fixtures from a directory.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            cluster_id: "fixture-cluster".to_owned(),
        }
    }

    /// Override the cluster identifier used for fixture facts.
    #[must_use]
    pub fn with_cluster_id(mut self, id: impl Into<String>) -> Self {
        self.cluster_id = id.into();
        self
    }

    /// Load and normalize a snapshot.
    ///
    /// A missing file is not an error: fixtures are written per-kind, and a set
    /// that omits PodDisruptionBudgets should report that kind as *not
    /// collected* rather than as an empty list. That distinction is the whole
    /// point of `KindCoverage`.
    ///
    /// # Errors
    ///
    /// Returns an error if a file exists but cannot be parsed, or if the
    /// resulting snapshot cannot be built.
    pub fn load(&self) -> Result<ClusterSnapshot, CollectError> {
        let now = Utc::now();
        let mut coverage = Vec::new();

        let ctx = |file: &str| NormalizeContext {
            cluster_id: self.cluster_id.clone(),
            observed_at: now,
            collection: CollectionStatus::InSync { synced_at: now },
            origin: Origin::Fixture {
                file: file.to_owned(),
            },
        };

        let nodes_raw: Option<Vec<Node>> = self.read_list("nodes.json")?;
        let pods_raw: Option<Vec<Pod>> = self.read_list("pods.json")?;
        let deployments_raw: Option<Vec<Deployment>> = self.read_list("deployments.json")?;
        let stateful_raw: Option<Vec<StatefulSet>> = self.read_list("statefulsets.json")?;
        let daemon_raw: Option<Vec<DaemonSet>> = self.read_list("daemonsets.json")?;
        let replica_raw: Option<Vec<ReplicaSet>> = self.read_list("replicasets.json")?;
        let pdbs_raw: Option<Vec<PodDisruptionBudget>> = self.read_list("pdbs.json")?;
        let events_raw: Option<Vec<Event>> = self.read_list("events.json")?;

        let mut record = |kind: &str, present: bool, count: usize| {
            coverage.push(KindCoverage {
                kind: kind.to_owned(),
                status: if present {
                    CollectionStatus::InSync { synced_at: now }
                } else {
                    CollectionStatus::Degraded {
                        degraded_since: now,
                        error: "no fixture file for this kind".to_owned(),
                    }
                },
                observed_count: count,
            });
        };

        let nodes: Vec<_> = nodes_raw
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|n| normalize::node::normalize(n, &ctx("nodes.json")))
                    .collect()
            })
            .unwrap_or_default();
        record("Node", nodes_raw.is_some(), nodes.len());

        let pods: Vec<_> = pods_raw
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|p| normalize::pod::normalize(p, &ctx("pods.json")))
                    .collect()
            })
            .unwrap_or_default();
        record("Pod", pods_raw.is_some(), pods.len());

        let mut workloads = Vec::new();
        if let Some(items) = &deployments_raw {
            workloads.extend(
                items
                    .iter()
                    .map(|d| normalize::workload::deployment(d, &ctx("deployments.json"))),
            );
        }
        record(
            "Deployment",
            deployments_raw.is_some(),
            deployments_raw.as_ref().map_or(0, Vec::len),
        );
        if let Some(items) = &stateful_raw {
            workloads.extend(
                items
                    .iter()
                    .map(|s| normalize::workload::stateful_set(s, &ctx("statefulsets.json"))),
            );
        }
        record(
            "StatefulSet",
            stateful_raw.is_some(),
            stateful_raw.as_ref().map_or(0, Vec::len),
        );
        if let Some(items) = &daemon_raw {
            workloads.extend(
                items
                    .iter()
                    .map(|d| normalize::workload::daemon_set(d, &ctx("daemonsets.json"))),
            );
        }
        record(
            "DaemonSet",
            daemon_raw.is_some(),
            daemon_raw.as_ref().map_or(0, Vec::len),
        );
        if let Some(items) = &replica_raw {
            workloads.extend(
                items
                    .iter()
                    .map(|r| normalize::workload::replica_set(r, &ctx("replicasets.json"))),
            );
        }
        record(
            "ReplicaSet",
            replica_raw.is_some(),
            replica_raw.as_ref().map_or(0, Vec::len),
        );

        let pdbs: Vec<_> = pdbs_raw
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|p| normalize::pdb::normalize(p, &ctx("pdbs.json")))
                    .collect()
            })
            .unwrap_or_default();
        record("PodDisruptionBudget", pdbs_raw.is_some(), pdbs.len());

        let brupop_raw: Option<Vec<DynamicObject>> = self.read_list("brupop.json")?;
        let brupop: Vec<_> = brupop_raw
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|b| normalize::brupop::normalize(b, &ctx("brupop.json")))
                    .collect()
            })
            .unwrap_or_default();
        // Recorded even when absent. A kind with no coverage entry at all leaves
        // the interface unable to say anything about it, and "no entry" is
        // easily rendered as "still loading".
        record("BottlerocketShadow", brupop_raw.is_some(), brupop.len());

        let events: Vec<_> = events_raw
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|e| normalize::event::normalize(e, &ctx("events.json")))
                    .collect()
            })
            .unwrap_or_default();
        record("Event", events_raw.is_some(), events.len());

        ClusterSnapshot::new(
            now,
            Mode::Fixture,
            self.cluster_id.clone(),
            ff_core::SnapshotFacts {
                nodes,
                pods,
                workloads,
                pdbs,
                events,
                brupop,
                coverage,
            },
        )
        .map_err(CollectError::from)
    }

    /// Read one captured list response, if the file exists.
    fn read_list<T: DeserializeOwned>(&self, file: &str) -> Result<Option<Vec<T>>, CollectError> {
        let path: &Path = &self.dir.join(file);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path).map_err(|e| CollectError::Fixture {
            file: file.to_owned(),
            reason: e.to_string(),
        })?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| CollectError::Fixture {
                file: file.to_owned(),
                reason: e.to_string(),
            })?;
        // Accept either a Kubernetes List object or a bare array.
        let items = value.get("items").cloned().unwrap_or(value);
        let parsed: Vec<T> = serde_json::from_value(items).map_err(|e| CollectError::Fixture {
            file: file.to_owned(),
            reason: e.to_string(),
        })?;
        Ok(Some(parsed))
    }
}
