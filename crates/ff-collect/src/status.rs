//! Per-kind collection status tracking.
//!
//! The whole point of this module is that "we saw nothing" and "we were not
//! allowed to look" and "our credential expired" must never render the same
//! way. An empty PodDisruptionBudget list means no blockers, which means
//! **safe to drain** — so a tool that reports an empty list when it was
//! actually forbidden is worse than a tool that reports nothing (ADR-0019).

use std::time::Duration;

use chrono::{DateTime, Utc};
use ff_core::CollectionStatus;

/// How long a kind may go without a confirmed-current watch before its data is
/// treated as stale rather than current.
pub const DEFAULT_STALENESS_BUDGET: Duration = Duration::from_secs(30);

/// Why a watch stopped being authoritative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureCause {
    /// The API server rejected the request: RBAC does not permit it.
    Forbidden {
        /// The verb that was denied.
        verb: String,
        /// The resource that was denied.
        resource: String,
    },
    /// The credential is missing, invalid, or expired.
    ///
    /// Short-lived tokens make this a routine event rather than an exotic one,
    /// which is exactly why it needs its own state (ADR-0019).
    Unauthorized,
    /// The API server could not be reached.
    Disconnected {
        /// A redacted description. Never contains a URL or a token.
        detail: String,
    },
    /// Anything else the watch reported.
    WatchError {
        /// A redacted description.
        detail: String,
    },
}

impl FailureCause {
    /// A short label for the interface.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Forbidden { .. } => "forbidden",
            Self::Unauthorized => "unauthorized",
            Self::Disconnected { .. } => "disconnected",
            Self::WatchError { .. } => "watch error",
        }
    }

    /// A message safe to show a user and safe to log.
    ///
    /// Kubernetes error strings can embed the API server URL and occasionally
    /// request detail, so the raw error never reaches this. See
    /// `THREAT_MODEL.md` R1.
    #[must_use]
    pub fn redacted_message(&self) -> String {
        match self {
            Self::Forbidden { verb, resource } => {
                format!("RBAC denies {verb} on {resource}")
            }
            Self::Unauthorized => {
                "the Kubernetes credential is missing, invalid, or expired".to_owned()
            }
            Self::Disconnected { detail } | Self::WatchError { detail } => detail.clone(),
        }
    }
}

/// Tracks one Kubernetes kind's collection state over time.
#[derive(Clone, Debug)]
pub struct KindTracker {
    kind: String,
    resource_plural: String,
    state: TrackerState,
    last_current_at: Option<DateTime<Utc>>,
    observed_count: usize,
    staleness_budget: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TrackerState {
    Syncing,
    Current,
    Failed(FailureCause),
}

impl KindTracker {
    /// Start tracking a kind, in the syncing state.
    #[must_use]
    pub fn new(kind: impl Into<String>, resource_plural: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            resource_plural: resource_plural.into(),
            state: TrackerState::Syncing,
            last_current_at: None,
            observed_count: 0,
            staleness_budget: DEFAULT_STALENESS_BUDGET,
        }
    }

    /// Override the staleness budget.
    #[must_use]
    pub const fn with_staleness_budget(mut self, budget: Duration) -> Self {
        self.staleness_budget = budget;
        self
    }

    /// The Kubernetes kind being tracked.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The plural resource name, used when reporting a denial.
    #[must_use]
    pub fn resource_plural(&self) -> &str {
        &self.resource_plural
    }

    /// The watch delivered a complete, current view.
    pub fn mark_current(&mut self, at: DateTime<Utc>, observed_count: usize) {
        self.state = TrackerState::Current;
        self.last_current_at = Some(at);
        self.observed_count = observed_count;
    }

    /// The watch failed.
    ///
    /// The observed count is deliberately **not** reset to zero. Whatever was
    /// last seen is still the last thing seen; the status is what tells a
    /// consumer they may no longer rely on it.
    pub fn mark_failed(&mut self, cause: FailureCause) {
        self.state = TrackerState::Failed(cause);
    }

    /// Whether the tracker is currently in a failed state.
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self.state, TrackerState::Failed(_))
    }

    /// The failure cause, if any.
    #[must_use]
    pub const fn failure(&self) -> Option<&FailureCause> {
        match &self.state {
            TrackerState::Failed(cause) => Some(cause),
            _ => None,
        }
    }

    /// How many objects were last observed.
    #[must_use]
    pub const fn observed_count(&self) -> usize {
        self.observed_count
    }

    /// Resolve the status as of `now`.
    ///
    /// Staleness is evaluated here rather than stored, because a tracker
    /// becomes stale through the passage of time rather than through an event.
    /// Nothing arrives to tell you the data got old.
    #[must_use]
    pub fn status_at(&self, now: DateTime<Utc>) -> CollectionStatus {
        match &self.state {
            TrackerState::Syncing => CollectionStatus::Syncing,
            TrackerState::Failed(FailureCause::Forbidden { verb, resource }) => {
                CollectionStatus::Forbidden {
                    verb: verb.clone(),
                    resource: resource.clone(),
                }
            }
            TrackerState::Failed(cause) => CollectionStatus::Degraded {
                degraded_since: self.last_current_at.unwrap_or(now),
                error: cause.redacted_message(),
            },
            TrackerState::Current => {
                let Some(last) = self.last_current_at else {
                    return CollectionStatus::Syncing;
                };
                let age = now.signed_duration_since(last);
                let budget = chrono::Duration::from_std(self.staleness_budget)
                    .unwrap_or_else(|_| chrono::Duration::seconds(30));
                if age > budget {
                    CollectionStatus::Stale {
                        last_current_at: last,
                        age_seconds: age.num_seconds(),
                    }
                } else {
                    CollectionStatus::InSync { synced_at: last }
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn t(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    #[test]
    fn a_new_tracker_is_syncing_not_empty() {
        let tracker = KindTracker::new("PodDisruptionBudget", "poddisruptionbudgets");
        assert_eq!(tracker.status_at(t(100)), CollectionStatus::Syncing);
        assert!(!tracker.status_at(t(100)).is_authoritative());
    }

    #[test]
    fn forbidden_is_not_authoritative_and_names_the_denial() {
        let mut tracker = KindTracker::new("PodDisruptionBudget", "poddisruptionbudgets");
        tracker.mark_failed(FailureCause::Forbidden {
            verb: "list".into(),
            resource: "poddisruptionbudgets".into(),
        });
        let status = tracker.status_at(t(100));
        assert!(!status.is_authoritative());
        assert_eq!(
            status,
            CollectionStatus::Forbidden {
                verb: "list".into(),
                resource: "poddisruptionbudgets".into()
            }
        );
    }

    #[test]
    fn an_expired_credential_is_not_an_empty_cluster() {
        // The failure this test exists for: token expires, watches return 401,
        // PDB list renders empty, "no blockers", drain reported safe.
        let mut tracker = KindTracker::new("PodDisruptionBudget", "poddisruptionbudgets");
        tracker.mark_current(t(100), 3);
        tracker.mark_failed(FailureCause::Unauthorized);

        let status = tracker.status_at(t(110));
        assert!(!status.is_authoritative(), "expired credential must not be authoritative");
        assert_eq!(
            tracker.observed_count(),
            3,
            "the last known count is retained; status is what marks it unusable"
        );
        match status {
            CollectionStatus::Degraded { error, .. } => {
                assert!(error.contains("expired"), "cause must be legible: {error}");
            }
            other => panic!("expected Degraded, got {other:?}"),
        }
    }

    #[test]
    fn data_goes_stale_through_the_passage_of_time_alone() {
        let mut tracker = KindTracker::new("Node", "nodes")
            .with_staleness_budget(Duration::from_secs(30));
        tracker.mark_current(t(100), 3);

        assert!(tracker.status_at(t(120)).is_authoritative(), "20s is within budget");
        let stale = tracker.status_at(t(200));
        assert!(!stale.is_authoritative(), "100s is beyond budget");
        match stale {
            CollectionStatus::Stale { age_seconds, .. } => assert_eq!(age_seconds, 100),
            other => panic!("expected Stale, got {other:?}"),
        }
    }

    #[test]
    fn recovery_returns_to_authoritative() {
        let mut tracker = KindTracker::new("Pod", "pods");
        tracker.mark_failed(FailureCause::Disconnected {
            detail: "connection reset".into(),
        });
        assert!(!tracker.status_at(t(100)).is_authoritative());

        tracker.mark_current(t(150), 12);
        assert!(tracker.status_at(t(150)).is_authoritative());
    }

    #[test]
    fn redacted_messages_carry_no_transport_detail() {
        let cause = FailureCause::Forbidden {
            verb: "list".into(),
            resource: "poddisruptionbudgets".into(),
        };
        let msg = cause.redacted_message();
        assert!(!msg.contains("https://"));
        assert!(!msg.contains("Bearer"));
        assert!(msg.contains("poddisruptionbudgets"));
    }
}
