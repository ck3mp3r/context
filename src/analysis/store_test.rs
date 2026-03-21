// Tests for NanoGraph wrapper

use crate::analysis::store::CodeGraph;
use crate::analysis::types::{ExtractedSymbol, SymbolKind};
use tempfile::TempDir;

#[tokio::test]
async fn test_create_code_graph() {
    let temp = TempDir::new().unwrap();
    let graph = CodeGraph::new(temp.path(), "test-repo").await;

    // Should successfully create a new code graph
    assert!(graph.is_ok());

    // Should create analysis.nano/ directory
    assert!(temp.path().join("analysis.nano").exists());
}

#[tokio::test]
async fn test_insert_file_node() {
    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "test-repo").await.unwrap();

    let file_id = graph.insert_file("src/main.rs", "rust", "abc123hash").await;

    assert!(file_id.is_ok());
    assert!(!file_id.unwrap().is_empty());
}

#[tokio::test]
async fn test_insert_symbol() {
    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "test-repo").await.unwrap();

    let symbol = ExtractedSymbol {
        name: "greet".to_string(),
        kind: SymbolKind::Function,
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        end_line: 3,
        content: "fn greet() { }".to_string(),
        signature: Some("fn greet()".to_string()),
    };

    let symbol_id = graph.insert_symbol(&symbol).await;

    assert!(symbol_id.is_ok());
    assert!(!symbol_id.unwrap().is_empty());
}

#[tokio::test]
async fn test_insert_contains_edge() {
    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "test-repo").await.unwrap();

    let file_id = graph
        .insert_file("src/main.rs", "rust", "abc123")
        .await
        .unwrap();

    let symbol = dummy_symbol("test_fn");
    let symbol_id = graph.insert_symbol(&symbol).await.unwrap();

    let result = graph.insert_contains(&file_id, &symbol_id, 1.0).await;

    if let Err(e) = &result {
        eprintln!("insert_contains error: {:?}", e);
    }
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_query_symbols_in_file() {
    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "test-repo").await.unwrap();

    // Insert file + 2 symbols
    let _file_id = graph
        .insert_file("src/main.rs", "rust", "abc123")
        .await
        .unwrap();

    let sym1 = dummy_symbol("foo");
    let sym2 = dummy_symbol("bar");

    graph.insert_symbol(&sym1).await.unwrap();
    graph.insert_symbol(&sym2).await.unwrap();

    let symbols = graph.query_symbols_in_file("src/main.rs").await;

    if let Err(e) = &symbols {
        eprintln!("query_symbols_in_file error: {:?}", e);
    }
    assert!(symbols.is_ok());
    let symbols = symbols.unwrap();
    assert_eq!(symbols.len(), 2);
}

// Helper function to create dummy symbols for testing
fn dummy_symbol(name: &str) -> ExtractedSymbol {
    ExtractedSymbol {
        name: name.to_string(),
        kind: SymbolKind::Function,
        file_path: "src/main.rs".to_string(),
        start_line: 1,
        end_line: 3,
        content: format!("fn {}() {{}}", name),
        signature: Some(format!("fn {}()", name)),
    }
}
