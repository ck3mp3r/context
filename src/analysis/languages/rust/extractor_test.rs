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
    // Should extract: Person struct + impl + new function + greet method
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

// Phase 2 Tests - Query-based Calls extraction

#[test]
fn test_extract_calls_simple() {
    use crate::analysis::extractor::SymbolExtractor;
    use crate::analysis::types::RelationType;

    let code = r#"
fn caller() {
    callee();
}
"#;

    let extractor = RustExtractor;
    let relationships = extractor.extract_relationships(code, "test.rs");

    let calls: Vec<_> = relationships
        .iter()
        .filter(|r| matches!(r.relation_type, RelationType::Calls { .. }))
        .collect();

    assert!(!calls.is_empty());
    assert!(calls[0].from_symbol_id.contains("caller"));
    assert!(calls[0].to_symbol_id.contains("callee"));
    assert_eq!(calls[0].confidence, 0.8);
}

#[test]
fn test_extract_calls_method() {
    use crate::analysis::extractor::SymbolExtractor;

    let code = r#"
fn process() {
    obj.method();
}
"#;

    let extractor = RustExtractor;
    let relationships = extractor.extract_relationships(code, "test.rs");

    let method_call = relationships
        .iter()
        .find(|r| r.to_symbol_id.contains("method"))
        .expect("Should find method call");

    assert!(method_call.from_symbol_id.contains("process"));
    assert_eq!(method_call.confidence, 0.9);
}

// Phase 2 Tests - Query-based Type Reference extraction

#[test]
fn test_extract_type_refs_simple() {
    use crate::analysis::extractor::SymbolExtractor;
    use crate::analysis::types::RelationType;

    let code = r#"
fn process(input: String) -> i32 {
    42
}
"#;

    let extractor = RustExtractor;
    let relationships = extractor.extract_relationships(code, "test.rs");

    let type_refs: Vec<_> = relationships
        .iter()
        .filter(|r| matches!(r.relation_type, RelationType::References { .. }))
        .collect();

    // Should find String and/or i32
    assert!(!type_refs.is_empty(), "Should find type references");

    for rel in type_refs {
        assert!(rel.from_symbol_id.contains("process"));
        assert_eq!(rel.confidence, 1.0);
    }
}

// Phase 2 Tests - Trait implementation extraction

#[test]
fn test_extract_trait_impl() {
    use crate::analysis::extractor::SymbolExtractor;
    use crate::analysis::types::RelationType;

    let code = r#"
impl Display for MyStruct {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Ok(())
    }
}
"#;

    let extractor = RustExtractor;
    let relationships = extractor.extract_relationships(code, "test.rs");

    // Should find Inherits relationship: MyStruct -> Display
    let inherits: Vec<_> = relationships
        .iter()
        .filter(|r| matches!(r.relation_type, RelationType::Inherits { .. }))
        .collect();

    assert!(!inherits.is_empty(), "Should find trait implementation");
    assert!(inherits[0].from_symbol_id.contains("MyStruct"));
    assert!(inherits[0].to_symbol_id.contains("Display"));
    assert_eq!(inherits[0].confidence, 1.0);
}

#[test]
fn test_extract_multiple_trait_impls() {
    use crate::analysis::extractor::SymbolExtractor;
    use crate::analysis::types::RelationType;

    let code = r#"
impl Display for MyStruct {
    fn fmt(&self) -> String {
        String::new()
    }
}

impl Clone for MyStruct {
    fn clone(&self) -> Self {
        Self {}
    }
}
"#;

    let extractor = RustExtractor;
    let relationships = extractor.extract_relationships(code, "test.rs");

    // Should find 2 Inherits relationships
    let inherits: Vec<_> = relationships
        .iter()
        .filter(|r| matches!(r.relation_type, RelationType::Inherits { .. }))
        .collect();

    assert_eq!(inherits.len(), 2);

    // Both should be from MyStruct
    for rel in inherits {
        assert!(rel.from_symbol_id.contains("MyStruct"));
    }
}

// Phase 2 Tests - Symbol containment extraction

#[test]
fn test_extract_symbol_contains() {
    use crate::analysis::extractor::SymbolExtractor;
    use crate::analysis::types::RelationType;

    let code = r#"
impl MyStruct {
    fn new() -> Self {
        Self {}
    }
    
    fn process(&self) {
        // do something
    }
}
"#;

    let extractor = RustExtractor;
    let relationships = extractor.extract_relationships(code, "test.rs");

    // Should find Contains relationships: impl MyStruct -> new, impl MyStruct -> process
    let contains: Vec<_> = relationships
        .iter()
        .filter(|r| matches!(r.relation_type, RelationType::Contains))
        .collect();

    assert_eq!(contains.len(), 2, "Should find 2 Contains relationships");

    // All should be from impl MyStruct
    for rel in contains {
        assert!(rel.from_symbol_id.contains("impl MyStruct"));
        assert_eq!(rel.confidence, 1.0);
    }
}

#[test]
fn test_extract_contains_multiple_impls() {
    use crate::analysis::extractor::SymbolExtractor;
    use crate::analysis::types::RelationType;

    let code = r#"
impl Foo {
    fn foo_method(&self) {}
}

impl Bar {
    fn bar_method(&self) {}
}
"#;

    let extractor = RustExtractor;
    let relationships = extractor.extract_relationships(code, "test.rs");

    let contains: Vec<_> = relationships
        .iter()
        .filter(|r| matches!(r.relation_type, RelationType::Contains))
        .collect();

    assert_eq!(contains.len(), 2);

    // Check that methods are linked to correct impl blocks
    let foo_contains = contains
        .iter()
        .find(|r| r.from_symbol_id.contains("impl Foo"))
        .expect("Should find Foo contains");
    assert!(foo_contains.to_symbol_id.contains("foo_method"));

    let bar_contains = contains
        .iter()
        .find(|r| r.from_symbol_id.contains("impl Bar"))
        .expect("Should find Bar contains");
    assert!(bar_contains.to_symbol_id.contains("bar_method"));
}

#[test]
fn test_extract_scoped_identifier_calls() {
    use crate::analysis::extractor::SymbolExtractor;
    use crate::analysis::types::RelationType;

    let code = r#"
use crate::analysis::service;

fn execute() {
    service::analyze_repository();
    crate::db::migrate();
    super::helper::process();
}
"#;

    let extractor = RustExtractor;
    let relationships = extractor.extract_relationships(code, "test.rs");

    // Filter to Calls relationships
    let calls: Vec<_> = relationships
        .iter()
        .filter(|r| matches!(r.relation_type, RelationType::Calls { .. }))
        .collect();

    assert_eq!(calls.len(), 3, "Should find 3 scoped calls");

    // Check that callee names have module prefix stripped
    let analyze_call = calls
        .iter()
        .find(|r| r.to_symbol_id.contains("analyze_repository"))
        .expect("Should find analyze_repository call");

    // The to_symbol_id should NOT contain "service::"
    assert!(!analyze_call.to_symbol_id.contains("service::"));
    assert!(analyze_call.to_symbol_id.contains("analyze_repository"));
    assert_eq!(analyze_call.confidence, 1.0); // scoped_identifier = high confidence

    let migrate_call = calls
        .iter()
        .find(|r| r.to_symbol_id.contains("migrate"))
        .expect("Should find migrate call");

    assert!(!migrate_call.to_symbol_id.contains("crate::"));
    assert!(!migrate_call.to_symbol_id.contains("db::"));
    assert!(migrate_call.to_symbol_id.contains("migrate"));

    let helper_call = calls
        .iter()
        .find(|r| r.to_symbol_id.contains("process"))
        .expect("Should find process call");

    assert!(!helper_call.to_symbol_id.contains("super::"));
    assert!(!helper_call.to_symbol_id.contains("helper::"));
    assert!(helper_call.to_symbol_id.contains("process"));
}
