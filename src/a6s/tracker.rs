//! In-memory tracker for per-repo analysis status.

use crate::api::notifier::{ChangeNotifier, UpdateMessage};
use crate::a6s::types::GraphStats;
use dashmap::DashMap;
use std::sync::Arc;

/// Status of a repo's code analysis.
#[derive(Debug, Clone)]
pub enum AnalysisStatus {
    /// Analysis is currently running, optionally with a phase descriptor.
    Analyzing { phase: Option<String> },
    /// Analysis completed successfully.
    Complete { stats: GraphStats },
    /// Analysis failed with an error.
    Failed { error: String },
}

/// In-memory tracker for per-repo analysis status.
///
/// Clone-friendly (wraps Arc). Broadcasts state changes via ChangeNotifier.
#[derive(Clone)]
pub struct AnalysisTracker {
    state: Arc<DashMap<String, AnalysisStatus>>,
    notifier: ChangeNotifier,
}

impl AnalysisTracker {
    /// Creates a new empty tracker.
    pub fn new(notifier: ChangeNotifier) -> Self {
        Self {
            state: Arc::new(DashMap::new()),
            notifier,
        }
    }

    /// Attempt to start analysis. Returns `true` if started, `false` if already analyzing.
    ///
    /// Uses DashMap's entry API to avoid TOCTOU races between check and insert.
    pub fn try_set_analyzing(&self, repo_id: &str) -> bool {
        use dashmap::mapref::entry::Entry;

        let started = match self.state.entry(repo_id.to_string()) {
            Entry::Occupied(ref entry)
                if matches!(entry.get(), AnalysisStatus::Analyzing { .. }) =>
            {
                false
            }
            Entry::Occupied(mut entry) => {
                entry.insert(AnalysisStatus::Analyzing { phase: None });
                true
            }
            Entry::Vacant(entry) => {
                entry.insert(AnalysisStatus::Analyzing { phase: None });
                true
            }
        };

        if started {
            self.notifier.notify(UpdateMessage::AnalysisStarted {
                repo_id: repo_id.to_string(),
            });
        }

        started
    }

    /// Mark a repo as currently being analyzed.
    pub fn set_analyzing(&self, repo_id: &str) {
        self.state
            .insert(repo_id.to_string(), AnalysisStatus::Analyzing { phase: None });
        self.notifier.notify(UpdateMessage::AnalysisStarted {
            repo_id: repo_id.to_string(),
        });
    }

    /// Update the phase of a currently-analyzing repo.
    ///
    /// No-op if the repo is not in `Analyzing` state (won't overwrite Complete/Failed).
    pub fn set_phase(&self, repo_id: &str, phase: &str) {
        if let Some(mut entry) = self.state.get_mut(repo_id)
            && matches!(entry.value(), AnalysisStatus::Analyzing { .. })
        {
            *entry = AnalysisStatus::Analyzing {
                phase: Some(phase.to_string()),
            };
        }
    }

    /// Mark a repo's analysis as complete.
    pub fn set_complete(&self, repo_id: &str, stats: GraphStats) {
        self.state
            .insert(repo_id.to_string(), AnalysisStatus::Complete { stats });
        self.notifier.notify(UpdateMessage::AnalysisCompleted {
            repo_id: repo_id.to_string(),
        });
    }

    /// Mark a repo's analysis as failed.
    pub fn set_failed(&self, repo_id: &str, error: String) {
        self.state.insert(
            repo_id.to_string(),
            AnalysisStatus::Failed {
                error: error.clone(),
            },
        );
        self.notifier.notify(UpdateMessage::AnalysisFailed {
            repo_id: repo_id.to_string(),
            error,
        });
    }

    /// Get the current analysis status for a repo.
    pub fn get(&self, repo_id: &str) -> Option<AnalysisStatus> {
        self.state.get(repo_id).map(|entry| entry.value().clone())
    }
}
