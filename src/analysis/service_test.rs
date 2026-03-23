//! Tests for analysis service - focusing on async behavior and progress

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

    // Create 100 test files to ensure multiple batches
    for i in 0..100 {
        let file_path = repo_path.join(format!("file{}.rs", i));
        std::fs::write(&file_path, "fn main() {}").unwrap();
    }

    let graph_path = temp_dir.path().join("graph");

    // Track progress updates
    let progress_updates = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = Arc::clone(&progress_updates);

    // Run analyzer with progress callback
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

    // Verify we got progress updates
    let updates = progress_updates.lock().unwrap();
    assert!(!updates.is_empty(), "Should have progress updates");

    // Verify progress is monotonically increasing
    for i in 1..updates.len() {
        assert!(
            updates[i].0 >= updates[i - 1].0,
            "Progress should increase: {:?}",
            updates
        );
    }

    // Last update should show completion
    let last = updates.last().unwrap();
    assert_eq!(
        last.0, last.1,
        "Last update should show all files processed: got {}/{}",
        last.0, last.1
    );
}

/// Test that analyzer processes files in batches
#[tokio::test]
#[cfg_attr(not(feature = "nanograph-tests"), ignore = "requires nanograph CLI")]
async fn test_analyzer_batches_work() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path).unwrap();

    // Create 150 files (should be 3 batches of 50)
    for i in 0..150 {
        let file_path = repo_path.join(format!("file{}.rs", i));
        std::fs::write(&file_path, "fn main() {}").unwrap();
    }

    let graph_path = temp_dir.path().join("graph");
    let batch_count = Arc::new(Mutex::new(0));
    let batch_count_clone = Arc::clone(&batch_count);

    let result = analyze_repository_with_progress(
        &repo_path,
        "test-repo",
        &graph_path,
        move |_processed, _total| {
            *batch_count_clone.lock().unwrap() += 1;
        },
    )
    .await;

    assert!(result.is_ok(), "Analysis should succeed");

    // Should have at least 3 progress updates (one per batch)
    let count = *batch_count.lock().unwrap();
    assert!(count >= 3, "Should have at least 3 batches, got {}", count);
}

/// Test incremental analysis: first scan analyzes all files
#[tokio::test]
#[cfg_attr(not(feature = "nanograph-tests"), ignore = "requires nanograph CLI")]
async fn test_incremental_first_scan() {
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

    // Create 10 files
    for i in 0..10 {
        let file_path = repo_path.join(format!("file{}.rs", i));
        std::fs::write(&file_path, "fn main() {}").unwrap();
    }

    // Initial commit
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

    // First scan - should analyze all 10 files
    let result = analyze_repository(&repo_path, "test-repo", &graph_path).await;

    assert!(result.is_ok());
    let analysis_result = result.unwrap();
    assert_eq!(
        analysis_result.files_analyzed, 10,
        "First scan should analyze all files"
    );

    // Metadata file should exist with commit SHA
    let metadata_path = graph_path.join("metadata.json");
    assert!(metadata_path.exists(), "Metadata file should be created");
}

/// Test incremental analysis: second scan only processes changed files
#[tokio::test]
#[cfg_attr(not(feature = "nanograph-tests"), ignore = "requires nanograph CLI")]
async fn test_incremental_changed_files() {
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

    // Create initial files
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

    // First scan
    let _result = analyze_repository(&repo_path, "test-repo", &graph_path)
        .await
        .unwrap();

    // Modify 2 files
    std::fs::write(repo_path.join("file0.rs"), "fn modified() {}").unwrap();
    std::fs::write(repo_path.join("file5.rs"), "fn also_modified() {}").unwrap();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["commit", "-m", "Modify 2 files"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Second scan - should only analyze 2 changed files
    let result = analyze_repository(&repo_path, "test-repo", &graph_path).await;

    assert!(result.is_ok());
    let analysis_result = result.unwrap();
    assert_eq!(
        analysis_result.files_analyzed, 2,
        "Second scan should only analyze 2 changed files"
    );
}

/// Test incremental analysis: handles deleted files
#[tokio::test]
#[cfg_attr(not(feature = "nanograph-tests"), ignore = "requires nanograph CLI")]
async fn test_incremental_deleted_files() {
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

    // Create initial files
    for i in 0..5 {
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

    // First scan
    let _result = analyze_repository(&repo_path, "test-repo", &graph_path)
        .await
        .unwrap();

    // Delete 1 file
    std::fs::remove_file(repo_path.join("file2.rs")).unwrap();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .args(["commit", "-m", "Delete file2.rs"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Second scan - should handle deletion
    let result = analyze_repository(&repo_path, "test-repo", &graph_path).await;

    assert!(result.is_ok(), "Should handle file deletion");
    // Deleted file should be removed from graph (implementation will verify this)
}
