use super::store::*;
use super::types::*;
use std::path::PathBuf;

#[test]
fn test_new_graph_empty_buffer() {
    let graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));
    assert_eq!(graph.buffer_len(), 0);
}

#[test]
fn test_insert_file_adds_to_buffer() {
    let mut graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));
    graph.insert_file("src/main.rs", "rust", "abc123");

    assert_eq!(graph.buffer_len(), 1);
    let buffer = graph.buffer();
    assert!(buffer[0].contains("\"type\":\"node\""));
    assert!(buffer[0].contains("\"label\":\"File\""));
    assert!(buffer[0].contains("src/main.rs"));
    assert!(buffer[0].contains("rust"));
    assert!(buffer[0].contains("abc123"));
}

#[test]
fn test_insert_symbol_adds_to_buffer() {
    let mut graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));

    let symbol = RawSymbol {
        name: "foo".to_string(),
        kind: "function".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 10,
        end_line: 20,
        signature: Some("fn foo()".to_string()),
        language: "rust".to_string(),
        visibility: Some("pub".to_string()),
        entry_type: None,
    };

    graph.insert_symbol(&symbol);

    assert_eq!(graph.buffer_len(), 1);
    let buffer = graph.buffer();
    assert!(buffer[0].contains("\"type\":\"node\""));
    assert!(buffer[0].contains("\"label\":\"Symbol\""));
    assert!(buffer[0].contains("foo"));
    assert!(buffer[0].contains("function"));
}

#[test]
fn test_insert_contains_edge() {
    let mut graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));

    let file_id = FileId::new("src/main.rs");
    let symbol_id = SymbolId::new("src/main.rs", "foo", 10);

    graph.insert_contains(&file_id, &symbol_id);

    assert_eq!(graph.buffer_len(), 1);
    let buffer = graph.buffer();
    assert!(buffer[0].contains("\"type\":\"edge\""));
    assert!(buffer[0].contains("\"label\":\"Contains\""));
}

#[test]
fn test_insert_calls_edge() {
    let mut graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));

    let from_id = SymbolId::new("src/main.rs", "foo", 10);
    let to_id = SymbolId::new("src/lib.rs", "bar", 20);

    graph.insert_calls_edge(&from_id, &to_id, Some(15));

    assert_eq!(graph.buffer_len(), 1);
    let buffer = graph.buffer();
    assert!(buffer[0].contains("\"type\":\"edge\""));
    assert!(buffer[0].contains("\"label\":\"Calls\""));
    assert!(buffer[0].contains("\"line\":15"));
}

#[test]
fn test_insert_inherits_edge() {
    let mut graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));

    let from_id = SymbolId::new("src/main.rs", "Dog", 10);
    let to_id = SymbolId::new("src/lib.rs", "Animal", 20);

    graph.insert_inherits_edge(&from_id, &to_id, &InheritanceType::Extends);

    assert_eq!(graph.buffer_len(), 1);
    let buffer = graph.buffer();
    assert!(buffer[0].contains("\"type\":\"edge\""));
    assert!(buffer[0].contains("\"label\":\"Inherits\""));
    assert!(buffer[0].contains("extends"));
}

#[test]
fn test_insert_references_edge() {
    let mut graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));

    let from_id = SymbolId::new("src/main.rs", "foo", 10);
    let to_id = SymbolId::new("src/lib.rs", "String", 5);

    graph.insert_references_edge(&from_id, &to_id, &EdgeKind::TypeRef, Some(12));

    assert_eq!(graph.buffer_len(), 1);
    let buffer = graph.buffer();
    assert!(buffer[0].contains("\"type\":\"edge\""));
    assert!(buffer[0].contains("\"label\":\"References\""));
    assert!(buffer[0].contains("TypeRef"));
    assert!(buffer[0].contains("\"line\":12"));
}

#[test]
fn test_insert_file_imports_edge() {
    let mut graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));

    let file_id = FileId::new("src/main.rs");
    let symbol_id = SymbolId::new("src/lib.rs", "foo", 10);

    graph.insert_file_imports_edge(&file_id, &symbol_id);

    assert_eq!(graph.buffer_len(), 1);
    let buffer = graph.buffer();
    assert!(buffer[0].contains("\"type\":\"edge\""));
    assert!(buffer[0].contains("\"label\":\"FileImports\""));
}

#[test]
fn test_commit_noop() {
    let graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));
    let result = graph.commit();
    assert!(result.is_ok());
}

#[test]
fn test_multiple_inserts() {
    let mut graph = CodeGraph::new(PathBuf::from("/tmp/test-analysis"));

    // Insert file
    graph.insert_file("src/main.rs", "rust", "abc123");

    // Insert 3 symbols
    let sym1 = RawSymbol {
        name: "foo".to_string(),
        kind: "function".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 10,
        end_line: 15,
        signature: None,
        language: "rust".to_string(),
        visibility: None,
        entry_type: None,
    };
    let sym2 = RawSymbol {
        name: "bar".to_string(),
        kind: "function".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 20,
        end_line: 25,
        signature: None,
        language: "rust".to_string(),
        visibility: None,
        entry_type: None,
    };
    let sym3 = RawSymbol {
        name: "MyStruct".to_string(),
        kind: "struct".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 30,
        end_line: 35,
        signature: None,
        language: "rust".to_string(),
        visibility: None,
        entry_type: None,
    };

    graph.insert_symbol(&sym1);
    graph.insert_symbol(&sym2);
    graph.insert_symbol(&sym3);

    // Insert 2 edges
    let id1 = sym1.symbol_id();
    let id2 = sym2.symbol_id();
    graph.insert_calls_edge(&id1, &id2, None);
    graph.insert_references_edge(&id1, &id2, &EdgeKind::Usage, Some(12));

    // Verify total: 1 file + 3 symbols + 2 edges = 6
    assert_eq!(graph.buffer_len(), 6);
}
