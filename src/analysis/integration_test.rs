// Integration test: Full analysis pipeline (Parser → Extractor → NanoGraph)

use crate::analysis::extractor::SymbolExtractor;
use crate::analysis::languages::rust::RustExtractor;
use crate::analysis::parser::Parser;
use crate::analysis::store::CodeGraph;
use tempfile::TempDir;

const SAMPLE_RUST: &str = r#"
pub struct Calculator {
    value: i32,
}

impl Calculator {
    pub fn new() -> Self {
        Self { value: 0 }
    }
    
    pub fn add(&mut self, n: i32) -> i32 {
        self.value += n;
        self.value
    }
}

pub fn main() {
    let mut calc = Calculator::new();
    let result = calc.add(42);
    println!("Result: {}", result);
}
"#;

#[tokio::test]
#[ignore = "nanograph query syntax changed - needs investigation"]
async fn test_full_analysis_pipeline() {
    let temp = TempDir::new().unwrap();

    // 1. Create code graph
    let mut graph = CodeGraph::new(temp.path(), "test-repo").await.unwrap();

    // 2. Parse file
    let mut parser = Parser::new_rust().unwrap();
    let tree = parser.parse(SAMPLE_RUST).unwrap();
    assert!(!tree.root_node().has_error());

    // 3. Extract symbols
    let extractor = RustExtractor;
    let symbols = extractor.extract_symbols(SAMPLE_RUST, "src/calc.rs");

    // Should extract: Calculator struct + impl block + new() + add() + main()
    assert!(
        symbols.len() >= 5,
        "Expected at least 5 symbols, got {}",
        symbols.len()
    );

    // 4. Insert into graph
    let file_id = graph
        .insert_file("src/calc.rs", "rust", "hash123")
        .await
        .unwrap();

    for symbol in &symbols {
        let sym_id = graph.insert_symbol(symbol).await.unwrap();
        graph.insert_contains(&file_id, &sym_id, 1.0).await.unwrap();
    }

    // 5. Query back
    let stored_symbols = graph.query_symbols_in_file("src/calc.rs").await.unwrap();
    assert_eq!(
        stored_symbols.len(),
        symbols.len(),
        "Should retrieve all inserted symbols"
    );

    // Verify specific symbols exist
    let names: Vec<_> = stored_symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Calculator"));
    assert!(names.contains(&"new"));
    assert!(names.contains(&"add"));
    assert!(names.contains(&"main"));
}

#[tokio::test]
#[ignore = "nanograph query syntax changed - needs investigation"]
async fn test_pipeline_with_multiple_files() {
    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "multi-repo").await.unwrap();

    let file1 = "pub fn hello() -> String { String::from(\"hello\") }";
    let file2 = "pub fn world() -> String { String::from(\"world\") }";

    let extractor = RustExtractor;

    // Process file1
    let symbols1 = extractor.extract_symbols(file1, "src/file1.rs");
    let file1_id = graph
        .insert_file("src/file1.rs", "rust", "hash1")
        .await
        .unwrap();
    for sym in &symbols1 {
        let sym_id = graph.insert_symbol(sym).await.unwrap();
        graph
            .insert_contains(&file1_id, &sym_id, 1.0)
            .await
            .unwrap();
    }

    // Process file2
    let symbols2 = extractor.extract_symbols(file2, "src/file2.rs");
    let file2_id = graph
        .insert_file("src/file2.rs", "rust", "hash2")
        .await
        .unwrap();
    for sym in &symbols2 {
        let sym_id = graph.insert_symbol(sym).await.unwrap();
        graph
            .insert_contains(&file2_id, &sym_id, 1.0)
            .await
            .unwrap();
    }

    // Query each file separately
    let file1_symbols = graph.query_symbols_in_file("src/file1.rs").await.unwrap();
    let file2_symbols = graph.query_symbols_in_file("src/file2.rs").await.unwrap();

    assert_eq!(file1_symbols.len(), 1);
    assert_eq!(file2_symbols.len(), 1);
    assert_eq!(file1_symbols[0].name, "hello");
    assert_eq!(file2_symbols[0].name, "world");
}
