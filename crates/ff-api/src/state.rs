//! Application state.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use ff_collect::{Collector, SnapshotStore};
use ff_core::{ClusterSnapshot, KindCoverage, Mode};

/// Where this process gets its data.
///
/// Modelled as an enum rather than a flag so that fixture mode cannot
/// accidentally acquire a live code path: there is no collector to reach for.
pub enum DataSource {
    /// Watching a real cluster.
    Live(Arc<Collector>),
    /// Serving a recorded snapshot from disk.
    Fixture(Arc<ClusterSnapshot>),
}

/// Shared application state.
pub struct AppState {
    /// The data source.
    pub source: DataSource,
    /// When the process started, for the environment panel.
    pub started_at: DateTime<Utc>,
    /// Build version.
    pub version: &'static str,
}

impl AppState {
    /// The mode everything this process serves is in.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        match self.source {
            DataSource::Live(_) => Mode::Live,
            DataSource::Fixture(_) => Mode::Fixture,
        }
    }

    /// The cluster identifier.
    #[must_use]
    pub fn cluster_id(&self) -> String {
        match &self.source {
            DataSource::Live(c) => c.cluster_id().to_owned(),
            DataSource::Fixture(s) => s.cluster_id().to_owned(),
        }
    }

    /// The current snapshot, if there is one.
    ///
    /// `None` in live mode means the first sync has not completed. That is a
    /// loading state and must never be rendered as an empty cluster.
    #[must_use]
    pub fn snapshot(&self) -> Option<Arc<ClusterSnapshot>> {
        match &self.source {
            DataSource::Live(c) => c.store().current(),
            DataSource::Fixture(s) => Some(Arc::clone(s)),
        }
    }

    /// The live snapshot store, when running live.
    #[must_use]
    pub fn store(&self) -> Option<Arc<SnapshotStore>> {
        match &self.source {
            DataSource::Live(c) => Some(c.store()),
            DataSource::Fixture(_) => None,
        }
    }

    /// Per-kind collection coverage.
    pub async fn coverage(&self) -> Vec<KindCoverage> {
        match &self.source {
            DataSource::Live(c) => c.coverage().await,
            DataSource::Fixture(s) => s.coverage().to_vec(),
        }
    }

    /// Whether the API server is currently reachable.
    ///
    /// Fixture mode has no API server, so the question does not apply and the
    /// answer is true — the data is exactly as trustworthy as it was when
    /// captured, which is what `FIXTURE` already says.
    #[must_use]
    pub fn api_server_reachable(&self) -> bool {
        match &self.source {
            DataSource::Live(c) => c.connection().is_reachable(),
            DataSource::Fixture(_) => true,
        }
    }

    /// When the API server was last reached.
    pub async fn last_contact(&self) -> Option<DateTime<Utc>> {
        match &self.source {
            DataSource::Live(c) => c.connection().last_contact().await,
            DataSource::Fixture(_) => None,
        }
    }

    /// Whether every kind was collected authoritatively **and** the cluster is
    /// still reachable.
    ///
    /// Both halves are required. Perfect coverage collected ten minutes ago
    /// from a cluster we can no longer reach is not authoritative, however good
    /// the per-kind statuses look.
    pub async fn authoritative(&self) -> bool {
        self.api_server_reachable()
            && self
                .coverage()
                .await
                .iter()
                .all(|c| c.status.is_authoritative())
    }
}
