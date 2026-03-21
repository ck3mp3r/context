// Tests for Rust symbol extractor

use crate::analysis::languages::rust::RustExtractor;
use crate::analysis::types::SymbolKind;

const FUNCTION_CODE: &str = r#"
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#;

const STRUCT_CODE: &str = r#"
struct Person {
    name: String,
    age: u32,
}

impl Person {
    fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }
    
    fn greet(&self) -> String {
        format!("Hello, I'm {}", self.name)
    }
}
"#;

#[test]
fn test_extract_function() {
    let symbols = RustExtractor::extract(FUNCTION_CODE, "test.rs");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "greet");
    assert_eq!(symbols[0].kind, SymbolKind::Function);
    assert_eq!(symbols[0].start_line, 2);
    assert!(symbols[0].signature.is_some());
}

#[test]
fn test_extract_struct_and_impl() {
    let symbols = RustExtractor::extract(STRUCT_CODE, "test.rs");
    // Should extract: Person struct + new function + greet method
    assert!(symbols.len() >= 3);

    let struct_sym = symbols
        .iter()
        .find(|s| s.kind == SymbolKind::Struct)
        .unwrap();
    assert_eq!(struct_sym.name, "Person");

    let methods: Vec<_> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .collect();
    assert!(methods.len() >= 2);
}

#[test]
fn test_extract_file_path() {
    let symbols = RustExtractor::extract(FUNCTION_CODE, "src/main.rs");
    assert_eq!(symbols[0].file_path, "src/main.rs");
}
