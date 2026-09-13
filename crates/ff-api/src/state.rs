//! Application state.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use ff_collect::{Collector, SnapshotStore};
use ff_core::{ClusterSnapshot, KindCoverage, Mode};
use ff_record::EventLog;
use ff_replay::ReplayBundle;

/// Where this process gets its data.
///
/// Modelled as an enum rather than a flag so that fixture mode cannot
/// accidentally acquire a live code path: there is no collector to reach for.
pub enum DataSource {
    /// Watching a real cluster.
    Live(Arc<Collector>),
    /// Serving a recorded snapshot from disk.
    Fixture(Arc<ClusterSnapshot>),
    /// Replaying a captured incident bundle.
    ///
    /// A third variant rather than a flag on `Fixture`: replay and fixture make
    /// different claims — "this happened" versus "this never happened" — and a
    /// shared code path is how one gets rendered as the other.
    Replay(Arc<ReplayBundle>),
}

/// Shared application state.
pub struct AppState {
    /// The data source.
    pub source: DataSource,
    /// When the process started, for the environment panel.
    pub started_at: DateTime<Utc>,
    /// Build version.
    pub version: &'static str,
    /// The event log, when recording is enabled.
    pub log: Option<Arc<EventLog>>,
}

impl AppState {
    /// The replay bundle, when this process is serving one.
    #[must_use]
    pub fn replay(&self) -> Option<&Arc<ReplayBundle>> {
        match &self.source {
            DataSource::Replay(b) => Some(b),
            _ => None,
        }
    }
}

impl AppState {
    /// The mode everything this process serves is in.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        match self.source {
            DataSource::Live(_) => Mode::Live,
            DataSource::Fixture(_) => Mode::Fixture,
            DataSource::Replay(_) => Mode::Replay,
        }
    }

    /// The cluster identifier.
    #[must_use]
    pub fn cluster_id(&self) -> String {
        match &self.source {
            DataSource::Live(c) => c.cluster_id().to_owned(),
            DataSource::Fixture(s) => s.cluster_id().to_owned(),
            DataSource::Replay(b) => b.context.cluster_id.clone(),
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
            // Replay serves its own typed endpoints; there is no single
            // "current" ClusterSnapshot, because the whole point is that the
            // state depends on where you are in the timeline.
            DataSource::Replay(_) => None,
        }
    }

    /// The live snapshot store, when running live.
    #[must_use]
    pub fn store(&self) -> Option<Arc<SnapshotStore>> {
        match &self.source {
            DataSource::Live(c) => Some(c.store()),
            DataSource::Fixture(_) | DataSource::Replay(_) => None,
        }
    }

    /// Per-kind collection coverage.
    pub async fn coverage(&self) -> Vec<KindCoverage> {
        match &self.source {
            DataSource::Live(c) => c.coverage().await,
            DataSource::Fixture(s) => s.coverage().to_vec(),
            DataSource::Replay(_) => Vec::new(),
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
            DataSource::Fixture(_) | DataSource::Replay(_) => true,
        }
    }

    /// The most recent liveness-probe outcome, as a label.
    ///
    /// `unauthorized` and `unreachable` are both unsafe to analyze against, and
    /// they are reported separately because they send an operator to different
    /// fixes.
    #[must_use]
    pub fn connection_state(&self) -> &'static str {
        match &self.source {
            DataSource::Live(c) => c.connection().state().label(),
            DataSource::Fixture(_) | DataSource::Replay(_) => "not applicable",
        }
    }

    /// Whether the last probe both reached the server and was accepted.
    #[must_use]
    pub fn connection_usable(&self) -> bool {
        match &self.source {
            DataSource::Live(c) => c.connection().is_usable(),
            DataSource::Fixture(_) | DataSource::Replay(_) => true,
        }
    }

    /// When the API server was last reached.
    pub async fn last_contact(&self) -> Option<DateTime<Utc>> {
        match &self.source {
            DataSource::Live(c) => c.connection().last_contact().await,
            DataSource::Fixture(_) | DataSource::Replay(_) => None,
        }
    }

    /// Whether every kind was collected authoritatively **and** the cluster is
    /// still reachable.
    ///
    /// Both halves are required. Perfect coverage collected ten minutes ago
    /// from a cluster we can no longer reach is not authoritative, however good
    /// the per-kind statuses look.
    pub async fn authoritative(&self) -> bool {
        self.connection_usable()
            && self
                .coverage()
                .await
                .iter()
                .all(|c| c.status.is_authoritative())
    }
}
