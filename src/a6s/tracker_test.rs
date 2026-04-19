//! Tests for AnalysisTracker.

use super::tracker::{AnalysisStatus, AnalysisTracker};
use crate::api::notifier::{ChangeNotifier, UpdateMessage};
use std::collections::HashMap;

#[test]
fn test_new_tracker_returns_none_for_unknown_repo() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    assert!(tracker.get("unknown").is_none());
}

#[test]
fn test_set_analyzing() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    tracker.set_analyzing("repo1");
    let status = tracker.get("repo1").unwrap();
    assert!(matches!(
        status,
        AnalysisStatus::Analyzing { phase: Some(_) }
    ));
}

#[test]
fn test_set_phase_updates_analyzing() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    tracker.set_analyzing("repo1");
    tracker.set_phase("repo1", "Scanning");
    let status = tracker.get("repo1").unwrap();
    match status {
        AnalysisStatus::Analyzing { phase } => {
            assert_eq!(phase, Some("Scanning".to_string()));
        }
        _ => panic!("Expected Analyzing"),
    }
}

#[test]
fn test_set_phase_noop_when_complete() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    let stats = crate::a6s::types::GraphStats {
        total_symbols: 1,
        total_edges: 0,
        symbol_counts: HashMap::new(),
    };
    tracker.set_complete("repo1", stats);
    tracker.set_phase("repo1", "Scanning");
    assert!(matches!(
        tracker.get("repo1"),
        Some(AnalysisStatus::Complete { .. })
    ));
}

#[test]
fn test_set_phase_noop_when_failed() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    tracker.set_failed("repo1", "boom".into());
    tracker.set_phase("repo1", "Scanning");
    assert!(matches!(
        tracker.get("repo1"),
        Some(AnalysisStatus::Failed { .. })
    ));
}

#[test]
fn test_set_complete() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    let stats = crate::a6s::types::GraphStats {
        total_symbols: 42,
        total_edges: 10,
        symbol_counts: HashMap::new(),
    };
    tracker.set_complete("repo1", stats.clone());
    let status = tracker.get("repo1").unwrap();
    match status {
        AnalysisStatus::Complete { stats: s } => {
            assert_eq!(s.total_symbols, 42);
            assert_eq!(s.total_edges, 10);
        }
        _ => panic!("Expected Complete"),
    }
}

#[test]
fn test_set_failed() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    tracker.set_failed("repo1", "boom".into());
    let status = tracker.get("repo1").unwrap();
    match status {
        AnalysisStatus::Failed { error } => assert_eq!(error, "boom"),
        _ => panic!("Expected Failed"),
    }
}

#[test]
fn test_state_transitions() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    tracker.set_analyzing("repo1");
    assert!(matches!(
        tracker.get("repo1"),
        Some(AnalysisStatus::Analyzing { .. })
    ));

    let stats = crate::a6s::types::GraphStats {
        total_symbols: 5,
        total_edges: 3,
        symbol_counts: HashMap::new(),
    };
    tracker.set_complete("repo1", stats);
    assert!(matches!(
        tracker.get("repo1"),
        Some(AnalysisStatus::Complete { .. })
    ));
}

#[test]
fn test_try_set_analyzing_returns_true_when_idle() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    assert!(tracker.try_set_analyzing("repo1"));
    assert!(matches!(
        tracker.get("repo1"),
        Some(AnalysisStatus::Analyzing { phase: Some(_) })
    ));
}

#[test]
fn test_try_set_analyzing_returns_false_when_already_analyzing() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    tracker.set_analyzing("repo1");
    assert!(!tracker.try_set_analyzing("repo1"));
}

#[test]
fn test_try_set_analyzing_returns_true_after_complete() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    let stats = crate::a6s::types::GraphStats {
        total_symbols: 1,
        total_edges: 0,
        symbol_counts: HashMap::new(),
    };
    tracker.set_complete("repo1", stats);
    assert!(tracker.try_set_analyzing("repo1"));
    assert!(matches!(
        tracker.get("repo1"),
        Some(AnalysisStatus::Analyzing { .. })
    ));
}

#[test]
fn test_try_set_analyzing_returns_true_after_failed() {
    let tracker = AnalysisTracker::new(ChangeNotifier::new());
    tracker.set_failed("repo1", "boom".into());
    assert!(tracker.try_set_analyzing("repo1"));
    assert!(matches!(
        tracker.get("repo1"),
        Some(AnalysisStatus::Analyzing { .. })
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_notifier_receives_events() {
    let notifier = ChangeNotifier::new();
    let mut sub = notifier.subscribe();
    let tracker = AnalysisTracker::new(notifier);

    tracker.set_analyzing("repo1");

    let msg = sub.recv().await.unwrap();
    assert_eq!(
        msg,
        UpdateMessage::AnalysisStarted {
            repo_id: "repo1".into()
        }
    );
}
