//! Tests for analysis service

use super::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

/// Test that analyzer yields control and doesn't block forever
#[tokio::test]
#[cfg_attr(not(feature = "nanograph-tests"), ignore = "requires nanograph CLI")]
async fn test_analyzer_yields_and_completes() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path).unwrap();

    // Create 10 small test files
    for i in 0..10 {
        let file_path = repo_path.join(format!("file{}.rs", i));
        std::fs::write(&file_path, "fn main() {}").unwrap();
    }

    let graph_path = temp_dir.path().join("graph");

    // Run analyzer with a timeout - it should complete before timeout
    let result = timeout(
        Duration::from_secs(10),
        analyze_repository(&repo_path, "test-repo", &graph_path),
    )
    .await;

    assert!(result.is_ok(), "Analyzer should complete within timeout");
    assert!(result.unwrap().is_ok(), "Analysis should succeed");
}

/// Test that analyzer reports progress via callback
#[tokio::test]
#[cfg_attr(not(feature = "nanograph-tests"), ignore = "requires nanograph CLI")]
async fn test_analyzer_reports_progress() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path).unwrap();

    for i in 0..100 {
        let file_path = repo_path.join(format!("file{}.rs", i));
        std::fs::write(&file_path, "fn main() {}").unwrap();
    }

    let graph_path = temp_dir.path().join("graph");

    let progress_updates = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = Arc::clone(&progress_updates);

    let result = analyze_repository_with_progress(
        &repo_path,
        "test-repo",
        &graph_path,
        move |processed, total| {
            progress_clone.lock().unwrap().push((processed, total));
        },
    )
    .await;

    assert!(result.is_ok(), "Analysis should succeed");

    let updates = progress_updates.lock().unwrap();
    assert!(!updates.is_empty(), "Should have progress updates");

    // Last update should show completion
    let last = updates.last().unwrap();
    assert_eq!(
        last.0, last.1,
        "Last update should show all files processed: got {}/{}",
        last.0, last.1
    );
}

/// Running analysis twice should produce the same results (clean slate).
/// No stale data from the first run should leak into the second.
#[tokio::test]
#[cfg_attr(not(feature = "nanograph-tests"), ignore = "requires nanograph CLI")]
async fn test_analysis_is_clean_slate() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path).unwrap();

    // Create 5 files
    for i in 0..5 {
        let file_path = repo_path.join(format!("file{}.rs", i));
        std::fs::write(&file_path, format!("fn func{}() {{}}", i)).unwrap();
    }

    let graph_path = temp_dir.path().join("graph");

    // First analysis
    let result1 = analyze_repository(&repo_path, "test-repo", &graph_path)
        .await
        .unwrap();
    assert_eq!(result1.files_analyzed, 5);

    // Delete 2 files
    std::fs::remove_file(repo_path.join("file3.rs")).unwrap();
    std::fs::remove_file(repo_path.join("file4.rs")).unwrap();

    // Second analysis - should only see 3 files, not carry over stale data
    let result2 = analyze_repository(&repo_path, "test-repo", &graph_path)
        .await
        .unwrap();
    assert_eq!(
        result2.files_analyzed, 3,
        "Clean slate should only see 3 remaining files"
    );
}

/// Analysis should always process all files, even if nothing changed.
#[tokio::test]
#[cfg_attr(not(feature = "nanograph-tests"), ignore = "requires nanograph CLI")]
async fn test_reanalysis_processes_all_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path).unwrap();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create and commit files
    for i in 0..10 {
        let file_path = repo_path.join(format!("file{}.rs", i));
        std::fs::write(&file_path, "fn main() {}").unwrap();
    }
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    let graph_path = temp_dir.path().join("graph");

    // First analysis
    let result1 = analyze_repository(&repo_path, "test-repo", &graph_path)
        .await
        .unwrap();
    assert_eq!(result1.files_analyzed, 10);

    // Second analysis without any changes - should still process all 10 files
    let result2 = analyze_repository(&repo_path, "test-repo", &graph_path)
        .await
        .unwrap();
    assert_eq!(
        result2.files_analyzed, 10,
        "Re-analysis should process all files, not just changed ones"
    );
}
