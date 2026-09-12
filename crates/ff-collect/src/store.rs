//! The shared snapshot store.
//!
//! Readers get an `Arc<ClusterSnapshot>` — cheap to clone, immutable once
//! published. Subscribers are notified when a new snapshot replaces it.

use std::sync::Arc;

use ff_core::{ClusterSnapshot, SnapshotId};
use tokio::sync::{broadcast, watch};

/// What changed. Sent to every subscriber when a snapshot is published.
#[derive(Clone, Debug)]
pub struct SnapshotUpdate {
    /// The new snapshot.
    pub snapshot: Arc<ClusterSnapshot>,
    /// The snapshot it replaced, if any.
    ///
    /// When this equals the new identifier, nothing about the cluster actually
    /// changed — only the observation time — and the interface should not
    /// animate anything.
    pub previous: Option<SnapshotId>,
}

impl SnapshotUpdate {
    /// Whether the cluster's content changed, as opposed to merely being
    /// re-observed.
    #[must_use]
    pub fn is_content_change(&self) -> bool {
        self.previous
            .as_ref()
            .is_none_or(|prev| prev != self.snapshot.snapshot_id())
    }
}

/// Holds the current snapshot and notifies subscribers.
#[derive(Debug)]
pub struct SnapshotStore {
    current: watch::Sender<Option<Arc<ClusterSnapshot>>>,
    updates: broadcast::Sender<SnapshotUpdate>,
}

impl SnapshotStore {
    /// Create an empty store.
    ///
    /// `capacity` bounds the update backlog. A slow subscriber is lagged rather
    /// than allowed to stall the collector — the interface can always re-read
    /// the current snapshot, so dropping intermediate updates loses nothing
    /// that matters.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (current, _) = watch::channel(None);
        let (updates, _) = broadcast::channel(capacity);
        Self { current, updates }
    }

    /// Publish a new snapshot.
    pub fn publish(&self, snapshot: Arc<ClusterSnapshot>) {
        let previous = self
            .current
            .borrow()
            .as_ref()
            .map(|s| s.snapshot_id().clone());
        self.current.send_replace(Some(Arc::clone(&snapshot)));
        // An error here means nobody is listening, which is normal at startup.
        let _ = self.updates.send(SnapshotUpdate { snapshot, previous });
    }

    /// The current snapshot, if one has been built yet.
    ///
    /// `None` means the collector has not completed its first sync — which is
    /// a loading state, never an empty cluster.
    #[must_use]
    pub fn current(&self) -> Option<Arc<ClusterSnapshot>> {
        self.current.borrow().clone()
    }

    /// Subscribe to updates.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<SnapshotUpdate> {
        self.updates.subscribe()
    }

    /// Watch the current snapshot directly.
    #[must_use]
    pub fn watch(&self) -> watch::Receiver<Option<Arc<ClusterSnapshot>>> {
        self.current.subscribe()
    }
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new(64)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ff_core::Mode;

    fn snapshot(cluster: &str) -> Arc<ClusterSnapshot> {
        Arc::new(
            ClusterSnapshot::new(
                Utc::now(),
                Mode::Live,
                cluster.to_owned(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn an_empty_store_reports_none_not_an_empty_cluster() {
        let store = SnapshotStore::default();
        assert!(store.current().is_none());
    }

    #[tokio::test]
    async fn publishing_notifies_subscribers() {
        let store = SnapshotStore::default();
        let mut rx = store.subscribe();
        store.publish(snapshot("c1"));
        let update = rx.recv().await.unwrap();
        assert!(update.previous.is_none());
        assert!(update.is_content_change());
    }

    #[tokio::test]
    async fn re_observing_identical_state_is_not_a_content_change() {
        // Same cluster state, observed again. The identifier is unchanged, so
        // the interface should not report that something happened.
        let store = SnapshotStore::default();
        let mut rx = store.subscribe();
        store.publish(snapshot("c1"));
        let _ = rx.recv().await.unwrap();
        store.publish(snapshot("c1"));
        let second = rx.recv().await.unwrap();
        assert!(!second.is_content_change());
    }
}
