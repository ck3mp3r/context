//! Tests for code_analyze MCP tool status checking.

use crate::a6s::tracker::{AnalysisStatus, AnalysisTracker};
use crate::a6s::types::GraphStats;
use crate::api::notifier::ChangeNotifier;
use std::collections::HashMap;

/// Helper to create a tracker for testing
fn test_tracker() -> AnalysisTracker {
    AnalysisTracker::new(ChangeNotifier::new())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_returns_idle_for_unknown_repo() {
    let tracker = test_tracker();
    let result = tracker.get("unknown");
    assert!(result.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_returns_analyzing() {
    let tracker = test_tracker();
    tracker.set_analyzing("repo1");
    let status = tracker.get("repo1").unwrap();
    assert!(matches!(status, AnalysisStatus::Analyzing { .. }));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_returns_complete() {
    let tracker = test_tracker();
    let stats = GraphStats {
        total_symbols: 42,
        total_edges: 10,
        symbol_counts: HashMap::new(),
    };
    tracker.set_complete("repo1", stats);
    let status = tracker.get("repo1").unwrap();
    match status {
        AnalysisStatus::Complete { stats } => {
            assert_eq!(stats.total_symbols, 42);
        }
        _ => panic!("Expected Complete"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_status_returns_failed() {
    let tracker = test_tracker();
    tracker.set_failed("repo1", "boom".into());
    let status = tracker.get("repo1").unwrap();
    match status {
        AnalysisStatus::Failed { error } => {
            assert_eq!(error, "boom");
        }
        _ => panic!("Expected Failed"),
    }
}
