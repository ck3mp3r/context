use super::pipeline::*;
use super::types::*;
use tempfile::TempDir;
use tokio::sync::mpsc;

#[tokio::test(flavor = "multi_thread")]
async fn test_analyze_empty_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let analysis_path = temp_dir.path().join("analysis");

    let result = analyze(temp_dir.path(), &analysis_path, "test_commit", None).await;

    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.symbols_registered, 0);
    assert_eq!(stats.edges_resolved, 0);
    assert_eq!(stats.edges_dropped, 0);
    assert_eq!(stats.imports_resolved, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_progress_events_fire() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let analysis_path = temp_dir.path().join("analysis");

    let (tx, mut rx) = mpsc::channel(10);

    // Run analysis in background
    let analyze_handle = tokio::spawn(async move {
        analyze(temp_dir.path(), &analysis_path, "test_commit", Some(tx)).await
    });

    // Collect all progress events
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }

    // Wait for analysis to complete
    let result = analyze_handle.await.expect("Task panicked");
    assert!(result.is_ok());

    // Verify event order
    assert!(!events.is_empty());

    // Check sequence: Scanned → Extracted → Resolved → Loaded
    let mut saw_scanned = false;
    let mut saw_extracted = false;
    let mut saw_resolved = false;
    let mut saw_loaded = false;

    for event in events {
        match event {
            PipelineProgress::Scanned(_) => {
                assert!(!saw_extracted && !saw_resolved && !saw_loaded);
                saw_scanned = true;
            }
            PipelineProgress::Extracted(_) => {
                assert!(saw_scanned && !saw_resolved && !saw_loaded);
                saw_extracted = true;
            }
            PipelineProgress::Resolved(_) => {
                assert!(saw_scanned && saw_extracted && !saw_loaded);
                saw_resolved = true;
            }
            PipelineProgress::Loaded => {
                assert!(saw_scanned && saw_extracted && saw_resolved);
                saw_loaded = true;
            }
        }
    }

    assert!(saw_scanned);
    assert!(saw_extracted);
    assert!(saw_resolved);
    assert!(saw_loaded);
}
