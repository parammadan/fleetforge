//! Collector configuration.

use std::path::PathBuf;
use std::time::Duration;

use crate::status::DEFAULT_STALENESS_BUDGET;

/// How the collector should connect and behave.
#[derive(Clone, Debug)]
pub struct CollectorConfig {
    /// Explicit kubeconfig path. `None` uses the standard resolution order,
    /// including the in-cluster ServiceAccount when running as a pod.
    pub kubeconfig: Option<PathBuf>,
    /// Named context. `None` uses the current context.
    pub context: Option<String>,
    /// Override the derived cluster identity. Normally left `None`.
    pub cluster_id: Option<String>,
    /// How long a kind may go unconfirmed before its data is stale.
    pub staleness_budget: Duration,
    /// How often to actively confirm the API server is reachable.
    ///
    /// Needed because a watch is silent both when nothing is happening and when
    /// the connection has died.
    pub liveness_interval: Duration,
    /// How many consecutive failed probes before declaring the API server
    /// unreachable. More than one, so a single blip does not flip the whole
    /// interface into a failure state.
    pub liveness_failures_before_disconnected: u32,
    /// How long to coalesce watch events before rebuilding a snapshot.
    ///
    /// A single `kubectl scale` produces a burst — the Deployment, the
    /// ReplicaSet, and each new Pod. Rebuilding once per event would hash the
    /// snapshot five times and emit five updates describing one action.
    pub debounce: Duration,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            kubeconfig: None,
            context: None,
            cluster_id: None,
            staleness_budget: DEFAULT_STALENESS_BUDGET,
            liveness_interval: Duration::from_secs(5),
            liveness_failures_before_disconnected: 2,
            debounce: Duration::from_millis(250),
        }
    }
}

impl CollectorConfig {
    /// Use a specific kubeconfig file.
    #[must_use]
    pub fn with_kubeconfig(mut self, path: impl Into<PathBuf>) -> Self {
        self.kubeconfig = Some(path.into());
        self
    }

    /// Use a named context.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}
