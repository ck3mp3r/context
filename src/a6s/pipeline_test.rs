#[cfg(feature = "backend")]
use super::pipeline::*;
#[cfg(feature = "backend")]
use super::store::CodeGraph;
#[cfg(feature = "backend")]
use super::types::PipelineProgress;
#[cfg(feature = "backend")]
use tempfile::TempDir;
#[cfg(feature = "backend")]
use tokio::sync::mpsc;

#[cfg(feature = "backend")]
#[tokio::test(flavor = "multi_thread")]
async fn test_analyze_empty_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let graph = CodeGraph::new_in_memory("test_repo".to_string())
        .await
        .expect("Failed to create graph");

    let result = analyze_with_graph(temp_dir.path(), "test_commit", None, graph).await;

    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.symbols_registered, 0);
    assert_eq!(stats.edges_resolved, 0);
    assert_eq!(stats.edges_dropped, 0);
    assert_eq!(stats.imports_resolved, 0);
}

#[cfg(feature = "backend")]
#[tokio::test(flavor = "multi_thread")]
async fn test_progress_events_fire() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let (tx, mut rx) = mpsc::channel(10);

    let graph = CodeGraph::new_in_memory("test_repo".to_string())
        .await
        .expect("Failed to create graph");

    // Run analysis in background
    let repo_path = temp_dir.path().to_path_buf();
    let analyze_handle = tokio::spawn(async move {
        analyze_with_graph(&repo_path, "test_commit", Some(tx), graph).await
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

    assert!(saw_scanned && saw_extracted && saw_resolved && saw_loaded);
}

#[cfg(feature = "backend")]
#[tokio::test(flavor = "multi_thread")]
async fn test_nushell_multi_file_integration() {
    use std::fs;

    // Create temporary directory structure
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create lib directory
    fs::create_dir_all(repo_path.join("lib")).unwrap();

    // Write math.nu
    fs::write(
        repo_path.join("lib/math.nu"),
        r#"
export def add [a: int, b: int] -> int {
    $a + $b
}

export def multiply [a: int, b: int] -> int {
    $a * $b
}
"#,
    )
    .unwrap();

    // Write utils.nu
    fs::write(
        repo_path.join("lib/utils.nu"),
        r#"
export def greet [name: string] -> string {
    $"Hello, ($name)!"
}
"#,
    )
    .unwrap();

    // Write main.nu using glob import (not named import, because that's broken in import extraction)
    fs::write(
        repo_path.join("main.nu"),
        r#"
use lib/math *
use lib/utils *

def main [] {
    let sum = add 5 10
    let product = multiply 3 4
    let message = greet "World"

    print $"Sum: ($sum), Product: ($product)"
    print $message
}
"#,
    )
    .unwrap();

    // Run analysis
    let graph = CodeGraph::new_in_memory("test_repo".to_string())
        .await
        .expect("Failed to create graph");

    let result = analyze_with_graph(repo_path, "abc123", None, graph).await;

    assert!(result.is_ok(), "Analysis should succeed");
    let stats = result.unwrap();

    println!(
        "Stats: symbols={}, edges={}, imports={}, dropped={}",
        stats.symbols_registered, stats.edges_resolved, stats.imports_resolved, stats.edges_dropped
    );

    // Verify extraction
    assert!(
        stats.symbols_registered > 0,
        "Should extract symbols, got: {}",
        stats.symbols_registered
    );

    // Verify imports resolved
    assert!(
        stats.imports_resolved > 0,
        "Should resolve imports, got: {}",
        stats.imports_resolved
    );

    // Verify edges resolved (calls + imports)
    assert!(
        stats.edges_resolved > 0,
        "Should resolve edges, got: {}",
        stats.edges_resolved
    );

    // Check specific counts
    // Expected symbols: add, multiply, greet, main = 4 commands
    assert!(
        stats.symbols_registered >= 4,
        "Should have at least 4 symbols, got: {}",
        stats.symbols_registered
    );

    // Expected imports: 3 (add, multiply, greet) via glob imports
    assert!(
        stats.imports_resolved >= 3,
        "Should resolve at least 3 imports, got: {}",
        stats.imports_resolved
    );
}
