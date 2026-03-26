// Tests for NanoGraph wrapper

use crate::analysis::store::CodeGraph;
use crate::analysis::{Parser, Rust};
use tempfile::TempDir;

#[tokio::test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
async fn test_create_code_graph() {
    let temp = TempDir::new().unwrap();
    let graph = CodeGraph::new(temp.path(), "test-repo").await;

    // Should successfully create a new code graph
    assert!(graph.is_ok());

    // Should create analysis.nano/ directory
    assert!(temp.path().join("analysis.nano").exists());
}

#[tokio::test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
async fn test_insert_file_node() {
    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "test-repo").await.unwrap();

    let file_id = graph.insert_file("src/main.rs", "rust", "abc123hash").await;

    assert!(file_id.is_ok());
    assert!(!file_id.unwrap().is_empty());
}

#[tokio::test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
async fn test_insert_symbol() {
    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "test-repo").await.unwrap();
    let mut parser = Parser::<Rust>::new();

    let code = "fn greet() {}";
    let stats = parser
        .parse_and_analyze(code, "src/main.rs", &mut graph)
        .await
        .expect("Parse failed");

    assert!(stats.symbols_inserted > 0);
}

#[tokio::test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
async fn test_insert_contains_edge() {
    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "test-repo").await.unwrap();
    let mut parser = Parser::<Rust>::new();

    // Parse code to insert symbol (file node and FileContains edge created automatically)
    let code = "fn test_fn() {}";
    parser
        .parse_and_analyze(code, "src/main.rs", &mut graph)
        .await
        .expect("Parse failed");

    // FileContains edge is created automatically by parser
    graph.commit().await.expect("Commit failed");
}

#[tokio::test]
#[ignore = "nanograph query syntax changed - needs investigation"]
async fn test_query_symbols_in_file() {
    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "test-repo").await.unwrap();
    let mut parser = Parser::<Rust>::new();

    // Parse code with 2 functions
    let code = "fn foo() {} fn bar() {}";
    parser
        .parse_and_analyze(code, "src/main.rs", &mut graph)
        .await
        .expect("Parse failed");

    let symbols = graph.query_symbols_in_file("src/main.rs").await;

    if let Err(e) = &symbols {
        eprintln!("query_symbols_in_file error: {:?}", e);
    }
    assert!(symbols.is_ok());
    let symbols = symbols.unwrap();
    assert_eq!(symbols.len(), 2);
}
