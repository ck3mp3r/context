use super::extractor::RustExtractor;
use crate::a6s::extract::LanguageExtractor;
use crate::a6s::types::SymbolRef;

// ============================================================================
// Test Helpers
// ============================================================================

fn extract(code: &str) -> crate::a6s::types::ParsedFile {
    RustExtractor.extract(code, "test.rs")
}

fn find_symbol<'a>(
    parsed: &'a crate::a6s::types::ParsedFile,
    name: &str,
) -> Option<&'a crate::a6s::types::RawSymbol> {
    parsed.symbols.iter().find(|s| s.name == name)
}

fn count_symbols_of_kind(parsed: &crate::a6s::types::ParsedFile, kind: &str) -> usize {
    parsed.symbols.iter().filter(|s| s.kind == kind).count()
}

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/a6s/lang/rust/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

// ============================================================================
// Metadata Tests
// ============================================================================

#[test]
fn test_rust_extractor_language() {
    let extractor = RustExtractor;
    assert_eq!(extractor.language(), "rust");
}

#[test]
fn test_rust_extractor_extensions() {
    let extractor = RustExtractor;
    assert_eq!(extractor.extensions(), &["rs"]);
}

#[test]
fn test_rust_extractor_queries_nonempty() {
    let extractor = RustExtractor;
    assert!(!extractor.symbol_queries().is_empty());
    assert!(!extractor.type_ref_queries().is_empty());
}

// ============================================================================
// Phase 1: Basic Symbol Extraction Tests (RED)
// ============================================================================

#[test]
fn test_extracts_simple_function() {
    let code = "fn my_function() {}";
    let parsed = extract(code);

    let func = find_symbol(&parsed, "my_function").expect("Should find function");
    assert_eq!(func.kind, "function");
    assert_eq!(func.start_line, 1);
    assert!(func.entry_type.is_none());
}

#[test]
fn test_extracts_main_function() {
    let code = "fn main() {}";
    let parsed = extract(code);

    let main_fn = find_symbol(&parsed, "main").expect("Should find main");
    assert_eq!(main_fn.kind, "function");
    assert_eq!(main_fn.entry_type, Some("main".to_string()));
}

#[test]
fn test_extracts_struct() {
    let code = "struct MyStruct { field: i32 }";
    let parsed = extract(code);

    let st = find_symbol(&parsed, "MyStruct").expect("Should find struct");
    assert_eq!(st.kind, "struct");
    assert_eq!(st.start_line, 1);
}

#[test]
fn test_extracts_enum() {
    let code = "enum MyEnum { Variant }";
    let parsed = extract(code);

    let en = find_symbol(&parsed, "MyEnum").expect("Should find enum");
    assert_eq!(en.kind, "enum");
}

#[test]
fn test_extracts_trait() {
    let code = "trait MyTrait { fn method(&self); }";
    let parsed = extract(code);

    let tr = find_symbol(&parsed, "MyTrait").expect("Should find trait");
    assert_eq!(tr.kind, "trait");
}

#[test]
fn test_extracts_module() {
    let code = "mod my_module { fn inner() {} }";
    let parsed = extract(code);

    let module = find_symbol(&parsed, "my_module").expect("Should find module");
    assert_eq!(module.kind, "module");
}

#[test]
fn test_extracts_const() {
    let code = "const MAX: usize = 100;";
    let parsed = extract(code);

    let const_sym = find_symbol(&parsed, "MAX").expect("Should find const");
    assert_eq!(const_sym.kind, "const");
}

#[test]
fn test_extracts_static() {
    let code = r#"static REF: &str = "value";"#;
    let parsed = extract(code);

    let static_sym = find_symbol(&parsed, "REF").expect("Should find static");
    assert_eq!(static_sym.kind, "static");
}

#[test]
fn test_extracts_type_alias() {
    let code = "type Alias = String;";
    let parsed = extract(code);

    let alias = find_symbol(&parsed, "Alias").expect("Should find type alias");
    assert_eq!(alias.kind, "type_alias");
}

#[test]
fn test_extracts_macro_definition() {
    let code = "macro_rules! my_macro { () => {}; }";
    let parsed = extract(code);

    let mac = find_symbol(&parsed, "my_macro").expect("Should find macro");
    assert_eq!(mac.kind, "macro");
}

// ============================================================================
// Impl and Method Tests (RED)
// ============================================================================

#[test]
fn test_extracts_methods_inside_inherent_impl() {
    let code = r#"
        struct Point;
        impl Point {
            fn method(&self) {}
        }
    "#;
    let parsed = extract(code);

    let method = find_symbol(&parsed, "method").expect("Should find method");
    assert_eq!(method.kind, "function");
}

#[test]
fn test_extracts_methods_inside_trait_impl() {
    let code = r#"
        trait Shape {}
        struct Point;
        impl Shape for Point {
            fn area(&self) -> f64 { 0.0 }
        }
    "#;
    let parsed = extract(code);

    let area = find_symbol(&parsed, "area").expect("Should find area method");
    assert_eq!(area.kind, "function");
}

#[test]
fn test_extracts_methods_from_generic_impl() {
    let code = r#"
        struct Generic<T>(T);
        impl<T> Generic<T> {
            fn get(&self) -> &T { &self.0 }
        }
    "#;
    let parsed = extract(code);

    let get = find_symbol(&parsed, "get").expect("Should find get method");
    assert_eq!(get.kind, "function");
}

// ============================================================================
// Field Tests (RED)
// ============================================================================

#[test]
fn test_extracts_struct_fields() {
    let code = r#"
        struct Point {
            x: i32,
            y: i32,
        }
    "#;
    let parsed = extract(code);

    let x_field = find_symbol(&parsed, "x").expect("Should find x field");
    assert_eq!(x_field.kind, "field");

    let y_field = find_symbol(&parsed, "y").expect("Should find y field");
    assert_eq!(y_field.kind, "field");
}

#[test]
fn test_multiple_fields() {
    let code = r#"
        struct Data {
            name: String,
            age: u32,
            active: bool,
        }
    "#;
    let parsed = extract(code);

    assert_eq!(count_symbols_of_kind(&parsed, "field"), 3);
}

// ============================================================================
// Trait Method Signature Tests (RED)
// ============================================================================

#[test]
fn test_extracts_trait_method_signatures() {
    let code = r#"
        trait MyTrait {
            fn method(&self);
            fn another(&mut self) -> i32;
        }
    "#;
    let parsed = extract(code);

    let method = find_symbol(&parsed, "method").expect("Should find method signature");
    assert_eq!(method.kind, "function");

    let another = find_symbol(&parsed, "another").expect("Should find another signature");
    assert_eq!(another.kind, "function");
}

// ============================================================================
// Visibility Tests (RED)
// ============================================================================

#[test]
fn test_marks_pub_visibility() {
    let code = "pub fn public_function() {}";
    let parsed = extract(code);

    let func = find_symbol(&parsed, "public_function").expect("Should find function");
    assert_eq!(func.visibility, Some("pub".to_string()));
}

#[test]
fn test_marks_crate_visibility() {
    let code = "pub(crate) fn crate_visible() {}";
    let parsed = extract(code);

    let func = find_symbol(&parsed, "crate_visible").expect("Should find function");
    assert_eq!(func.visibility, Some("pub(crate)".to_string()));
}

#[test]
fn test_marks_private_visibility() {
    let code = "fn private_fn() {}";
    let parsed = extract(code);

    let func = find_symbol(&parsed, "private_fn").expect("Should find function");
    assert!(func.visibility.is_none());
}

#[test]
fn test_handles_pub_super() {
    let code = "pub(super) fn super_visible() {}";
    let parsed = extract(code);

    let func = find_symbol(&parsed, "super_visible").expect("Should find function");
    assert_eq!(func.visibility, Some("pub(super)".to_string()));
}

#[test]
fn test_struct_visibility() {
    let code = "pub struct PubStruct;";
    let parsed = extract(code);

    let st = find_symbol(&parsed, "PubStruct").expect("Should find struct");
    assert_eq!(st.visibility, Some("pub".to_string()));
}

// ============================================================================
// Entry Point Tests (RED)
// ============================================================================

#[test]
fn test_marks_test_fn_as_entry_point() {
    let code = r#"
        #[test]
        fn test_something() {}
    "#;
    let parsed = extract(code);

    let test_fn = find_symbol(&parsed, "test_something").expect("Should find test");
    assert_eq!(test_fn.entry_type, Some("test".to_string()));
}

#[test]
fn test_marks_tokio_test_as_entry_point() {
    let code = r#"
        #[tokio::test]
        async fn async_test() {}
    "#;
    let parsed = extract(code);

    let test_fn = find_symbol(&parsed, "async_test").expect("Should find async test");
    assert_eq!(test_fn.entry_type, Some("test".to_string()));
}

#[test]
fn test_marks_bench_fn_as_entry_point() {
    let code = r#"
        #[bench]
        fn bench_function(b: &mut Bencher) {}
    "#;
    let parsed = extract(code);

    let bench = find_symbol(&parsed, "bench_function").expect("Should find bench");
    assert_eq!(bench.entry_type, Some("bench".to_string()));
}

// ============================================================================
// Edge Case Tests (RED)
// ============================================================================

#[test]
fn test_handles_empty_file() {
    let code = "";
    let parsed = extract(code);

    // Empty file still gets implicit file module
    assert_eq!(parsed.symbols.len(), 1);
    assert_eq!(parsed.symbols[0].kind, "module");
    assert_eq!(parsed.symbols[0].name, "test"); // from "test.rs"
}

#[test]
fn test_handles_nested_modules() {
    let code = r#"
        mod outer {
            mod inner {
                fn nested_fn() {}
            }
        }
    "#;
    let parsed = extract(code);

    assert!(find_symbol(&parsed, "outer").is_some());
    assert!(find_symbol(&parsed, "inner").is_some());
    assert!(find_symbol(&parsed, "nested_fn").is_some());
}

#[test]
fn test_handles_generic_structs() {
    let code = "struct Generic<T> { value: T }";
    let parsed = extract(code);

    let generic_struct = find_symbol(&parsed, "Generic").expect("Should find generic struct");
    assert_eq!(generic_struct.kind, "struct");
}

#[test]
fn test_handles_generic_impl_bounds() {
    let code = r#"
        struct S<T>(T);
        impl<T: Clone> S<T> {
            fn get(&self) -> T { self.0.clone() }
        }
    "#;
    let parsed = extract(code);

    let get = find_symbol(&parsed, "get").expect("Should find method with generic bounds");
    assert_eq!(get.kind, "function");
}

// ============================================================================
// Integration Tests with Fixtures (RED)
// ============================================================================

#[test]
fn test_symbols_fixture() {
    let code = include_str!("testdata/symbols.rs");
    let parsed = extract(code);

    // Should extract all major symbol types
    assert!(find_symbol(&parsed, "standalone_function").is_some());
    assert!(find_symbol(&parsed, "main").is_some());
    assert!(find_symbol(&parsed, "public_function").is_some());
    assert!(find_symbol(&parsed, "MyStruct").is_some());
    assert!(find_symbol(&parsed, "MyEnum").is_some());
    assert!(find_symbol(&parsed, "MyTrait").is_some());
    assert!(find_symbol(&parsed, "my_module").is_some());
    assert!(find_symbol(&parsed, "MAX").is_some());
    assert!(find_symbol(&parsed, "REF").is_some());
    assert!(find_symbol(&parsed, "Alias").is_some());
    assert!(find_symbol(&parsed, "my_macro").is_some());

    // Verify counts
    assert!(count_symbols_of_kind(&parsed, "function") >= 3);
    assert_eq!(count_symbols_of_kind(&parsed, "struct"), 1);
    assert_eq!(count_symbols_of_kind(&parsed, "enum"), 1);
    assert_eq!(count_symbols_of_kind(&parsed, "trait"), 1);
}

#[test]
fn test_impl_blocks_fixture() {
    let code = include_str!("testdata/impl_blocks.rs");
    let parsed = extract(code);

    // Structs and traits
    assert!(find_symbol(&parsed, "Point").is_some());
    assert!(find_symbol(&parsed, "Shape").is_some());
    assert!(find_symbol(&parsed, "Generic").is_some());

    // Methods from various impl types
    assert!(find_symbol(&parsed, "method").is_some());
    assert!(find_symbol(&parsed, "area").is_some());
    assert!(find_symbol(&parsed, "get").is_some());
    assert!(find_symbol(&parsed, "clone").is_some());
}

#[test]
fn test_visibility_fixture() {
    let code = include_str!("testdata/visibility.rs");
    let parsed = extract(code);

    // Check visibility is correctly extracted
    let pub_fn = find_symbol(&parsed, "public_fn").expect("Should find public_fn");
    assert_eq!(pub_fn.visibility, Some("pub".to_string()));

    let priv_fn = find_symbol(&parsed, "private_fn").expect("Should find private_fn");
    assert!(priv_fn.visibility.is_none());

    let crate_fn = find_symbol(&parsed, "crate_visible").expect("Should find crate_visible");
    assert_eq!(crate_fn.visibility, Some("pub(crate)".to_string()));
}

// ============================================================================
// Phase 2: Edge Extraction Tests (RED → GREEN → REFACTOR)
// ============================================================================

// ----------------------------------------------------------------------------
// HasMember Edge Tests
// ----------------------------------------------------------------------------

#[test]
fn test_hasmember_module_to_function() {
    let code = r#"
mod my_module {
    fn inner_function() {}
}
"#;
    let parsed = extract(code);

    // Should have 2 HasMember edges:
    // 1. test (implicit file module) -> my_module
    // 2. my_module -> inner_function
    assert_eq!(
        parsed.edges.len(),
        2,
        "Should have two edges (including implicit file module)"
    );
    assert!(
        parsed
            .edges
            .iter()
            .all(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMember))
    );
}

#[test]
fn test_hasmember_module_to_struct() {
    let code = r#"
mod my_module {
    struct InnerStruct { x: i32 }
}
"#;
    let parsed = extract(code);

    // Should have 2 HasMember edges:
    // 1. test (implicit file module) -> my_module
    // 2. my_module -> InnerStruct
    // Plus 1 HasField edge for the field x
    let member_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMember))
        .collect();
    assert_eq!(
        member_edges.len(),
        2,
        "Should have 2 HasMember edges (including implicit file module)"
    );
}

#[test]
fn test_hasmember_multiple_members() {
    let code = r#"
mod my_module {
    fn func1() {}
    fn func2() {}
    struct MyStruct {}
    enum MyEnum {}
}
"#;
    let parsed = extract(code);

    // Should have 5 HasMember edges:
    // 1. test (implicit file module) -> my_module
    // 2-5. my_module -> (func1, func2, MyStruct, MyEnum)
    let member_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMember))
        .collect();
    assert_eq!(
        member_edges.len(),
        5,
        "Should have 5 HasMember edges (including implicit file module)"
    );
}

#[test]
fn test_hasmember_nested_modules() {
    let code = r#"
mod outer {
    fn outer_fn() {}
    mod inner {
        fn inner_fn() {}
    }
}
"#;
    let parsed = extract(code);

    // Should have 4 edges now (with implicit file module):
    // 1. test (implicit file module) -> outer
    // 2. outer -> outer_fn
    // 3. outer -> inner (nested module)
    // 4. inner -> inner_fn
    let member_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMember))
        .collect();
    assert_eq!(
        member_edges.len(),
        4,
        "Should have 4 HasMember edges (including implicit file module)"
    );
}

// ----------------------------------------------------------------------------
// HasField Edge Tests
// ----------------------------------------------------------------------------

#[test]
fn test_hasfield_struct_to_fields() {
    let code = r#"
struct MyStruct {
    field1: i32,
    field2: String,
}
"#;
    let parsed = extract(code);

    // Should have 2 HasField edges
    let field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasField))
        .collect();
    assert_eq!(field_edges.len(), 2, "Should have 2 HasField edges");
}

#[test]
fn test_hasfield_tuple_struct() {
    let code = r#"
struct TupleStruct(i32, String);
"#;
    let parsed = extract(code);

    // Tuple struct fields might not have names - check implementation
    // For now, just verify struct is extracted
    assert_eq!(count_symbols_of_kind(&parsed, "struct"), 1);
}

// ----------------------------------------------------------------------------
// HasMethod Edge Tests
// ----------------------------------------------------------------------------

#[test]
fn test_hasmethod_impl_block() {
    let code = r#"
struct MyStruct;
impl MyStruct {
    fn method1(&self) {}
    fn method2(&mut self) {}
}
"#;
    let parsed = extract(code);

    // Should have 2 HasMethod edges from MyStruct to method1 and method2
    let method_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();
    assert_eq!(method_edges.len(), 2, "Should have 2 HasMethod edges");
}

#[test]
fn test_hasmethod_trait_impl() {
    let code = r#"
trait MyTrait {
    fn trait_method(&self);
}
struct MyStruct;
impl MyTrait for MyStruct {
    fn trait_method(&self) {}
}
"#;
    let parsed = extract(code);

    // Should have HasMethod edge from MyStruct to trait_method
    let method_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();
    assert!(
        !method_edges.is_empty(),
        "Should have at least 1 HasMethod edge"
    );
}

#[test]
fn test_hasmethod_same_file_resolved() {
    let code = r#"
struct Foo {}

impl Foo {
    fn bar(&self) {}
}
"#;
    let parsed = extract(code);

    let hasmethod_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();

    assert_eq!(
        hasmethod_edges.len(),
        1,
        "Expected 1 HasMethod edge, got: {:?}",
        hasmethod_edges
    );

    match &hasmethod_edges[0].from {
        SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Foo:"),
            "from should be Foo, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &hasmethod_edges[0].to {
        SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":bar:"),
            "to should be bar, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_hasmethod_multiple_methods() {
    let code = r#"
struct Server {}

impl Server {
    fn start(&self) {}
    fn stop(&self) {}
}
"#;
    let parsed = extract(code);

    let hasmethod_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();

    assert_eq!(
        hasmethod_edges.len(),
        2,
        "Expected 2 HasMethod edges, got: {:?}",
        hasmethod_edges
    );

    // Both should have Resolved from and to
    for edge in &hasmethod_edges {
        match &edge.from {
            SymbolRef::Resolved(_) => {}
            other => panic!("from should be Resolved, got {:?}", other),
        }
        match &edge.to {
            SymbolRef::Resolved(_) => {}
            other => panic!("to should be Resolved, got {:?}", other),
        }
    }
}

#[test]
fn test_hasmethod_disambiguates_same_name() {
    let code = r#"
struct Foo {}
struct Bar {}

impl Foo {
    fn process(&self) {}
}

impl Bar {
    fn process(&self) {}
}
"#;
    let parsed = extract(code);

    let hasmethod_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();

    assert_eq!(
        hasmethod_edges.len(),
        2,
        "Expected 2 HasMethod edges, got: {:?}",
        hasmethod_edges
    );

    // All should be Resolved — no ambiguity because we match on line number
    for edge in &hasmethod_edges {
        match &edge.from {
            SymbolRef::Resolved(_) => {}
            other => panic!("from should be Resolved, got {:?}", other),
        }
        match &edge.to {
            SymbolRef::Resolved(_) => {}
            other => panic!("to should be Resolved, got {:?}", other),
        }
    }
}

// ----------------------------------------------------------------------------
// Implements Edge Tests (Resolved vs Unresolved)
// ----------------------------------------------------------------------------

#[test]
fn test_implements_same_file_both_resolved() {
    let code = r#"
trait Greeter {
    fn greet(&self);
}

struct MyGreeter;

impl Greeter for MyGreeter {
    fn greet(&self) {}
}
"#;
    let extractor = RustExtractor;
    let parsed = extractor.extract(code, "test.rs");

    let impl_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Implements)
        .collect();

    assert_eq!(
        impl_edges.len(),
        1,
        "Expected 1 Implements edge, got: {:?}",
        impl_edges
    );

    // from = MyGreeter (Resolved, same file)
    match &impl_edges[0].from {
        SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":MyGreeter:"),
            "from should be MyGreeter, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    // to = Greeter (Resolved, same file)
    match &impl_edges[0].to {
        SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Greeter:"),
            "to should be Greeter, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_implements_cross_file_trait_unresolved() {
    let code = r#"
struct MyType;

impl Display for MyType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Ok(())
    }
}
"#;
    let extractor = RustExtractor;
    let parsed = extractor.extract(code, "test.rs");

    let impl_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Implements)
        .collect();

    assert_eq!(
        impl_edges.len(),
        1,
        "Expected 1 Implements edge, got: {:?}",
        impl_edges
    );

    // from = MyType (Resolved, same file)
    match &impl_edges[0].from {
        SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":MyType:"),
            "from should be MyType, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    // to = Display (Unresolved, foreign trait)
    match &impl_edges[0].to {
        SymbolRef::Unresolved { name, .. } => {
            assert_eq!(name, "Display", "to should be Display, got {}", name)
        }
        other => panic!("to should be Unresolved, got {:?}", other),
    }
}

#[test]
fn test_implements_multiple_traits_all_resolved() {
    let code = r#"
trait Readable {
    fn read(&self) -> String;
}

trait Writable {
    fn write(&self, data: &str);
}

struct File;

impl Readable for File {
    fn read(&self) -> String { String::new() }
}

impl Writable for File {
    fn write(&self, data: &str) {}
}
"#;
    let extractor = RustExtractor;
    let parsed = extractor.extract(code, "test.rs");

    let impl_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Implements)
        .collect();

    assert_eq!(
        impl_edges.len(),
        2,
        "Expected 2 Implements edges, got: {:?}",
        impl_edges
    );

    // All should be Resolved (same file)
    for edge in &impl_edges {
        match &edge.from {
            SymbolRef::Resolved(_) => {}
            other => panic!("from should be Resolved, got {:?}", other),
        }
        match &edge.to {
            SymbolRef::Resolved(_) => {}
            other => panic!("to should be Resolved, got {:?}", other),
        }
    }
}

// ============================================================================
// Phase 3: Import Extraction Tests (RawImport entries, not Import edges)
// ============================================================================

#[test]
fn test_simple_use_statement() {
    let code = r#"
use std::collections::HashMap;
"#;
    let parsed = extract(code);

    // Should extract RawImport entry (not Import edge)
    assert_eq!(parsed.imports.len(), 1, "Expected 1 RawImport entry");
}

#[test]
fn test_nested_use_statements() {
    let code = r#"
use std::{fs, io};
"#;
    let parsed = extract(code);

    // Should extract 2 RawImport entries
    assert!(
        parsed.imports.len() >= 2,
        "Expected at least 2 RawImport entries for nested use, got {}",
        parsed.imports.len()
    );
}

#[test]
fn test_crate_relative_import() {
    let code = r#"
use crate::db::Database;
"#;
    let parsed = extract(code);

    // Should have RawImport entry with crate:: prefix
    assert!(
        !parsed.imports.is_empty(),
        "Expected at least 1 RawImport entry"
    );
}

#[test]
fn test_super_relative_import() {
    let code = r#"
use super::utils::helper;
"#;
    let parsed = extract(code);

    // Should have RawImport entry with super:: prefix
    assert!(
        !parsed.imports.is_empty(),
        "Expected at least 1 RawImport entry"
    );
}

#[test]
fn test_aliased_import() {
    let code = r#"
use std::io::Result as IoResult;
"#;
    let parsed = extract(code);

    // Should extract aliased import as RawImport entry
    assert!(
        !parsed.imports.is_empty(),
        "Expected at least 1 RawImport entry"
    );
}

#[test]
fn test_pub_use_reexport() {
    let code = r#"
pub use crate::types::Symbol;
"#;
    let parsed = extract(code);

    // Should extract pub use as RawImport entry
    assert!(
        !parsed.imports.is_empty(),
        "Expected at least 1 RawImport entry"
    );
}

#[test]
fn test_glob_import() {
    let code = r#"
use std::prelude::*;
"#;
    let parsed = extract(code);

    // Should extract glob import as RawImport entry
    assert!(
        !parsed.imports.is_empty(),
        "Expected at least 1 RawImport entry"
    );
}

#[test]
fn test_deep_nested_imports() {
    let code = r#"
use std::collections::{HashMap, HashSet, BTreeMap};
"#;
    let parsed = extract(code);

    // Should extract 3 RawImport entries
    assert!(
        parsed.imports.len() >= 3,
        "Expected at least 3 RawImport entries for deep nested use, got {}",
        parsed.imports.len()
    );
}

#[test]
fn test_multiple_separate_use_statements() {
    let code = r#"
use std::fs::File;
use std::io::Read;
"#;
    let parsed = extract(code);

    // Should extract 2 RawImport entries
    assert!(
        parsed.imports.len() >= 2,
        "Expected at least 2 RawImport entries, got {}",
        parsed.imports.len()
    );
}

#[test]
fn test_self_import() {
    let code = r#"
use self::inner::function;
"#;
    let parsed = extract(code);

    // Should extract self:: import as RawImport entry
    assert!(
        !parsed.imports.is_empty(),
        "Expected at least 1 RawImport entry"
    );
}

#[test]
fn test_imports_fixture() {
    let fixture = load_testdata("imports.rs");
    let parsed = extract(&fixture);

    // The fixture has many imports, verify we extract them as RawImport entries
    let import_count = parsed.imports.len();

    assert!(
        import_count >= 10,
        "Expected at least 10 imports from fixture, got {}",
        import_count
    );
}

#[test]
fn test_implements_multiple_traits() {
    let code = r#"
trait Trait1 {}
trait Trait2 {}
struct MyStruct;
impl Trait1 for MyStruct {}
impl Trait2 for MyStruct {}
"#;
    let parsed = extract(code);

    // Should have 2 Implements edges
    let impl_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::Implements))
        .collect();
    assert_eq!(impl_edges.len(), 2, "Should have 2 Implements edges");
}

// ----------------------------------------------------------------------------
// Test Edge Tests
// ----------------------------------------------------------------------------

#[test]
fn test_no_special_test_edges_for_regular_main() {
    let code = r#"
#[test]
fn test_example() {}

fn main() {
    println!("Not a test runner");
}
"#;
    let parsed = extract(code);

    // Regular main should not create special test edges
    // (In Rust, we don't have test runner detection like Nushell)
    // Check that main doesn't have calls to test functions
    let calls_from_main_to_tests: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::Calls))
        .filter(|e| {
            if let SymbolRef::Resolved(ref from_id) = e.from {
                if let SymbolRef::Resolved(ref to_id) = e.to {
                    // Check if main is calling a test function
                    from_id.as_str().contains(":main:") && to_id.as_str().contains(":test_")
                } else {
                    false
                }
            } else {
                false
            }
        })
        .collect();

    // Regular main in test file shouldn't auto-create calls to test functions
    assert_eq!(
        calls_from_main_to_tests.len(),
        0,
        "Regular main should not auto-create Calls edges to test functions"
    );
}

// ----------------------------------------------------------------------------
// Enhanced Test Detection Tests
// ----------------------------------------------------------------------------

#[test]
fn test_detects_tokio_test() {
    let code = r#"
#[tokio::test]
async fn test_async_function() {}
"#;
    let parsed = extract(code);

    let test_fn =
        find_symbol(&parsed, "test_async_function").expect("Should find async test function");
    assert_eq!(test_fn.entry_type, Some("test".to_string()));
}

#[test]
fn test_detects_tokio_test_with_flavor() {
    let code = r#"
#[tokio::test(flavor = "multi_thread")]
async fn test_multithreaded() {}
"#;
    let parsed = extract(code);

    let test_fn =
        find_symbol(&parsed, "test_multithreaded").expect("Should find multithreaded test");
    assert_eq!(test_fn.entry_type, Some("test".to_string()));
}

#[test]
fn test_detects_bench() {
    let code = r#"
#![feature(test)]
extern crate test;

#[bench]
fn bench_example(b: &mut test::Bencher) {}
"#;
    let parsed = extract(code);

    let bench_fn = find_symbol(&parsed, "bench_example").expect("Should find benchmark function");
    assert_eq!(bench_fn.entry_type, Some("bench".to_string()));
}

#[test]
fn test_extracts_test_metadata_ignore() {
    let code = r#"
#[test]
#[ignore]
fn test_ignored() {}
"#;
    let parsed = extract(code);

    let test_fn = find_symbol(&parsed, "test_ignored").expect("Should find ignored test");
    assert_eq!(test_fn.entry_type, Some("test".to_string()));

    // Check signature contains metadata
    let sig = test_fn.signature.as_ref().expect("Should have signature");
    assert!(
        sig.contains("ignored") || sig.contains("ignore"),
        "Signature should contain ignore metadata: {}",
        sig
    );
}

#[test]
fn test_extracts_test_metadata_should_panic() {
    let code = r#"
#[test]
#[should_panic]
fn test_panics() {}
"#;
    let parsed = extract(code);

    let test_fn = find_symbol(&parsed, "test_panics").expect("Should find should_panic test");

    // Check signature contains metadata
    let sig = test_fn.signature.as_ref().expect("Should have signature");
    assert!(
        sig.contains("should_panic"),
        "Signature should contain should_panic metadata: {}",
        sig
    );
}

#[test]
fn test_extracts_multiple_test_metadata() {
    let code = r#"
#[test]
#[ignore]
#[should_panic]
fn test_complex() {}
"#;
    let parsed = extract(code);

    let test_fn =
        find_symbol(&parsed, "test_complex").expect("Should find test with multiple attributes");

    let sig = test_fn.signature.as_ref().expect("Should have signature");
    assert!(
        sig.contains("ignored") || sig.contains("ignore"),
        "Signature should contain ignore: {}",
        sig
    );
    assert!(
        sig.contains("should_panic"),
        "Signature should contain should_panic: {}",
        sig
    );
}

// ----------------------------------------------------------------------------
// File Categorization Tests
// ----------------------------------------------------------------------------

#[test]
fn test_file_category_test_file_by_path() {
    let extractor = RustExtractor;
    let code = r#"
fn regular_function() {}
"#;
    let parsed = extractor.extract(code, "tests/my_test.rs");

    assert_eq!(
        parsed.file_category,
        Some("test_file".to_string()),
        "File in tests/ directory should be categorized as test_file"
    );
}

#[test]
fn test_file_category_test_file_by_suffix() {
    let extractor = RustExtractor;
    let code = r#"
fn regular_function() {}
"#;
    let parsed = extractor.extract(code, "src/module_test.rs");

    assert_eq!(
        parsed.file_category,
        Some("test_file".to_string()),
        "File ending with _test.rs should be categorized as test_file"
    );
}

#[test]
fn test_file_category_contains_tests() {
    let code = r#"
fn regular_function() {}

#[test]
fn test_something() {}
"#;
    let parsed = extract(code);

    assert_eq!(
        parsed.file_category,
        Some("contains_tests".to_string()),
        "File with test functions should be categorized as contains_tests"
    );
}

#[test]
fn test_file_category_regular() {
    let code = r#"
fn regular_function() {}
struct MyStruct {}
"#;
    let parsed = extract(code);

    assert!(
        parsed.file_category.is_none(),
        "Regular file without tests should not have file_category set"
    );
}

#[test]
fn test_debug_metadata() {
    let code = r#"
#[test]
#[ignore]
fn test_ignored() {}
"#;
    let parsed = extract(code);

    eprintln!("Symbols found: {}", parsed.symbols.len());
    for sym in &parsed.symbols {
        eprintln!(
            "Symbol: {} kind={} entry_type={:?} signature={:?}",
            sym.name, sym.kind, sym.entry_type, sym.signature
        );
    }

    assert!(!parsed.symbols.is_empty());
}

// ============================================================================
// Phase 3: Call Edge Extraction Tests (RED → GREEN → REFACTOR)
// ============================================================================

fn find_edge<'a>(
    parsed: &'a crate::a6s::types::ParsedFile,
    from_name: &str,
    to_name: &str,
    kind: &str,
) -> Option<&'a crate::a6s::types::RawEdge> {
    use crate::a6s::types::EdgeKind;
    let edge_kind = match kind {
        "Calls" => EdgeKind::Calls,
        "HasMethod" => EdgeKind::HasMethod,
        "Implements" => EdgeKind::Implements,
        "HasField" => EdgeKind::HasField,
        "HasMember" => EdgeKind::HasMember,
        _ => return None,
    };

    parsed.edges.iter().find(|e| {
        e.kind == edge_kind
            && match (&e.from, &e.to) {
                (
                    crate::a6s::types::SymbolRef::Resolved(from_id),
                    crate::a6s::types::SymbolRef::Unresolved {
                        name: to_name_val, ..
                    },
                ) => from_id.as_str().contains(from_name) && to_name_val == to_name,
                (
                    crate::a6s::types::SymbolRef::Resolved(from_id),
                    crate::a6s::types::SymbolRef::Resolved(to_id),
                ) => from_id.as_str().contains(from_name) && to_id.as_str().contains(to_name),
                (
                    crate::a6s::types::SymbolRef::Unresolved {
                        name: from_name_val,
                        ..
                    },
                    crate::a6s::types::SymbolRef::Unresolved {
                        name: to_name_val, ..
                    },
                ) => from_name_val == from_name && to_name_val == to_name,
                _ => false,
            }
    })
}

fn count_edges_of_kind(parsed: &crate::a6s::types::ParsedFile, kind: &str) -> usize {
    use crate::a6s::types::EdgeKind;
    let edge_kind = match kind {
        "Calls" => EdgeKind::Calls,
        "HasMethod" => EdgeKind::HasMethod,
        "Implements" => EdgeKind::Implements,
        "HasField" => EdgeKind::HasField,
        "HasMember" => EdgeKind::HasMember,
        _ => return 0,
    };
    parsed.edges.iter().filter(|e| e.kind == edge_kind).count()
}

// ============================================================================
// RED Phase: Basic Free Function Calls
// ============================================================================

#[test]
fn test_free_function_call_simple() {
    let code = r#"
fn main() {
    helper();
}

fn helper() {}
"#;
    let parsed = extract(code);

    // Should have 2 functions
    assert_eq!(count_symbols_of_kind(&parsed, "function"), 2);

    // Should have 1 Calls edge: main → helper
    assert_eq!(count_edges_of_kind(&parsed, "Calls"), 1);

    let edge = find_edge(&parsed, "main", "helper", "Calls");
    assert!(edge.is_some(), "Expected Calls edge from main to helper");
}

#[test]
fn test_multiple_calls_in_function() {
    let code = r#"
fn main() {
    foo();
    bar();
    baz();
}

fn foo() {}
fn bar() {}
fn baz() {}
"#;
    let parsed = extract(code);

    // Should have 4 functions
    assert_eq!(count_symbols_of_kind(&parsed, "function"), 4);

    // Should have 3 Calls edges
    assert_eq!(count_edges_of_kind(&parsed, "Calls"), 3);

    assert!(
        find_edge(&parsed, "main", "foo", "Calls").is_some(),
        "Expected call to foo"
    );
    assert!(
        find_edge(&parsed, "main", "bar", "Calls").is_some(),
        "Expected call to bar"
    );
    assert!(
        find_edge(&parsed, "main", "baz", "Calls").is_some(),
        "Expected call to baz"
    );
}

#[test]
fn test_call_with_arguments() {
    let code = r#"
fn main() {
    process(42, "test");
}

fn process(x: i32, s: &str) {}
"#;
    let parsed = extract(code);

    // Should have 1 Calls edge despite arguments
    assert_eq!(count_edges_of_kind(&parsed, "Calls"), 1);

    assert!(
        find_edge(&parsed, "main", "process", "Calls").is_some(),
        "Expected call to process with arguments"
    );
}

// ============================================================================
// RED Phase: Method Calls
// ============================================================================

#[test]
fn test_method_call_on_self() {
    let code = r#"
struct MyStruct;

impl MyStruct {
    fn method(&self) {
        self.helper();
    }

    fn helper(&self) {}
}
"#;
    let parsed = extract(code);

    // Should have 1 Calls edge: method → helper
    let calls = count_edges_of_kind(&parsed, "Calls");
    assert!(
        calls >= 1,
        "Expected at least 1 Calls edge, found {}",
        calls
    );

    assert!(
        find_edge(&parsed, "method", "helper", "Calls").is_some(),
        "Expected method to call helper"
    );
}

#[test]
fn test_method_call_on_variable() {
    let code = r#"
struct MyStruct;

impl MyStruct {
    fn do_work(&self) {}
}

fn main() {
    let obj = MyStruct;
    obj.do_work();
}
"#;
    let parsed = extract(code);

    // Should have 1 Calls edge: main → do_work
    assert!(
        count_edges_of_kind(&parsed, "Calls") >= 1,
        "Expected at least 1 Calls edge for method call"
    );

    assert!(
        find_edge(&parsed, "main", "do_work", "Calls").is_some(),
        "Expected main to call do_work method"
    );
}

#[test]
fn test_method_chain() {
    let code = r#"
struct MyStruct;

impl MyStruct {
    fn first(&self) -> &Self { self }
    fn second(&self) -> &Self { self }
}

fn main() {
    let obj = MyStruct;
    obj.first().second();
}
"#;
    let parsed = extract(code);

    // Should have calls to both first and second
    let calls = count_edges_of_kind(&parsed, "Calls");
    assert!(
        calls >= 2,
        "Expected at least 2 Calls edges for method chain, found {}",
        calls
    );

    assert!(
        find_edge(&parsed, "main", "first", "Calls").is_some(),
        "Expected call to first"
    );
    assert!(
        find_edge(&parsed, "main", "second", "Calls").is_some(),
        "Expected call to second"
    );
}

// ============================================================================
// RED Phase: Macro Invocations
// ============================================================================

#[test]
fn test_macro_invocation_println() {
    let code = r#"
fn main() {
    println!("hello");
}
"#;
    let parsed = extract(code);

    // Should have 1 Calls edge: main → println
    assert_eq!(count_edges_of_kind(&parsed, "Calls"), 1);

    assert!(
        find_edge(&parsed, "main", "println", "Calls").is_some(),
        "Expected call to println! macro"
    );
}

#[test]
fn test_macro_invocation_multiple() {
    let code = r#"
fn main() {
    println!("hello");
    assert!(true);
    vec![1, 2, 3];
}
"#;
    let parsed = extract(code);

    // Should have 3 Calls edges
    assert_eq!(count_edges_of_kind(&parsed, "Calls"), 3);

    assert!(
        find_edge(&parsed, "main", "println", "Calls").is_some(),
        "Expected call to println"
    );
    assert!(
        find_edge(&parsed, "main", "assert", "Calls").is_some(),
        "Expected call to assert"
    );
    assert!(
        find_edge(&parsed, "main", "vec", "Calls").is_some(),
        "Expected call to vec"
    );
}

// ============================================================================
// RED Phase: Nested Calls
// ============================================================================

#[test]
fn test_calls_in_nested_blocks() {
    let code = r#"
fn main() {
    if true {
        foo();
    } else {
        bar();
    }
}

fn foo() {}
fn bar() {}
"#;
    let parsed = extract(code);

    // Should have calls from nested blocks
    assert!(
        count_edges_of_kind(&parsed, "Calls") >= 2,
        "Expected calls from nested if/else blocks"
    );

    assert!(
        find_edge(&parsed, "main", "foo", "Calls").is_some(),
        "Expected call to foo in if block"
    );
    assert!(
        find_edge(&parsed, "main", "bar", "Calls").is_some(),
        "Expected call to bar in else block"
    );
}

#[test]
fn test_calls_in_match_expression() {
    let code = r#"
fn main() {
    match Some(42) {
        Some(x) => process(x),
        None => handle_none(),
    }
}

fn process(x: i32) {}
fn handle_none() {}
"#;
    let parsed = extract(code);

    // Should have calls from match arms
    assert!(
        count_edges_of_kind(&parsed, "Calls") >= 2,
        "Expected calls from match arms"
    );

    assert!(
        find_edge(&parsed, "main", "process", "Calls").is_some(),
        "Expected call to process in match arm"
    );
    assert!(
        find_edge(&parsed, "main", "handle_none", "Calls").is_some(),
        "Expected call to handle_none in match arm"
    );
}

// ============================================================================
// RED Phase: Qualified Path Calls
// ============================================================================

#[test]
fn test_qualified_path_call() {
    let code = r#"
fn main() {
    std::fs::read_to_string("file.txt");
}
"#;
    let parsed = extract(code);

    // Should have 1 Calls edge (qualified path)
    assert_eq!(count_edges_of_kind(&parsed, "Calls"), 1);

    // The callee name might be just "read_to_string" or full path
    let calls_edge = parsed
        .edges
        .iter()
        .find(|e| matches!(e.kind, crate::a6s::types::EdgeKind::Calls));
    assert!(
        calls_edge.is_some(),
        "Expected a Calls edge for qualified path"
    );
}

#[test]
fn test_module_function_call() {
    let code = r#"
mod utils {
    pub fn helper() {}
}

fn main() {
    utils::helper();
}
"#;
    let parsed = extract(code);

    // Should have 1 Calls edge
    assert!(
        count_edges_of_kind(&parsed, "Calls") >= 1,
        "Expected call to module function"
    );
}

// ============================================================================
// RED Phase: Edge Cases
// ============================================================================

#[test]
fn test_no_calls_in_empty_function() {
    let code = r#"
fn empty() {}

fn main() {
    empty();
}
"#;
    let parsed = extract(code);

    // Only main should have a call edge
    assert_eq!(count_edges_of_kind(&parsed, "Calls"), 1);
}

#[test]
fn test_recursive_call() {
    let code = r#"
fn fibonacci(n: u32) -> u32 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}
"#;
    let parsed = extract(code);

    // Should have 2 Calls edges (recursive calls to self)
    assert_eq!(count_edges_of_kind(&parsed, "Calls"), 2);

    assert!(
        find_edge(&parsed, "fibonacci", "fibonacci", "Calls").is_some(),
        "Expected recursive call"
    );
}

// ============================================================================
// Phase 3: Module Path Derivation Tests (RED → GREEN → REFACTOR)
// ============================================================================

#[test]
fn test_module_path_simple() {
    let path = derive_module_path("src/db/project.rs");
    assert_eq!(path, Some("db".to_string())); // Parent module of project
}

#[test]
fn test_module_path_nested() {
    let path = derive_module_path("src/api/v1/tasks.rs");
    assert_eq!(path, Some("api::v1".to_string())); // Parent module of tasks
}

#[test]
fn test_module_path_mod_rs() {
    let path = derive_module_path("src/api/mod.rs");
    assert_eq!(path, None); // api is top-level, no parent
}

#[test]
fn test_module_path_nested_mod_rs() {
    let path = derive_module_path("src/db/sqlite/mod.rs");
    assert_eq!(path, Some("db".to_string())); // Parent module of sqlite
}

#[test]
fn test_module_path_main_rs() {
    let path = derive_module_path("src/main.rs");
    assert_eq!(path, None); // crate root
}

#[test]
fn test_module_path_lib_rs() {
    let path = derive_module_path("src/lib.rs");
    assert_eq!(path, None); // crate root
}

#[test]
fn test_module_path_single_level() {
    let path = derive_module_path("src/config.rs");
    assert_eq!(path, None); // config is top-level, no parent
}

#[test]
fn test_module_path_without_src_prefix() {
    // Should handle paths without src/ prefix
    let path = derive_module_path("db/project.rs");
    assert_eq!(path, Some("db".to_string())); // Parent module of project
}

#[test]
fn test_module_path_deep_nesting() {
    let path = derive_module_path("src/analysis/lang/rust/extractor.rs");
    assert_eq!(path, Some("analysis::lang::rust".to_string())); // Parent module of extractor
}

#[test]
fn test_module_path_empty() {
    let path = derive_module_path("");
    assert_eq!(path, None);
}

#[test]
fn test_module_path_root_only() {
    let path = derive_module_path("src/");
    assert_eq!(path, None);
}

// Helper function that uses the trait method
fn derive_module_path(file_path: &str) -> Option<String> {
    RustExtractor.derive_module_path(file_path)
}

// ============================================================================
// Phase 3: Module Path Integration Tests
// ============================================================================

#[test]
fn test_symbols_have_module_path_set() {
    let code = r#"
fn test_func() {}
struct TestStruct {}
"#;
    let parsed = RustExtractor.extract(code, "src/api/handlers.rs");

    // Non-implicit symbols should have module_path set to "api" (their parent module)
    // The implicit module "handlers" should also have module_path "api"
    for symbol in &parsed.symbols {
        assert_eq!(
            symbol.module_path.as_deref(),
            Some("api"),
            "Symbol {} should have module_path set to api (parent module)",
            symbol.name
        );
    }
}

#[test]
fn test_root_symbols_have_no_module_path() {
    let code = r#"
fn main() {}
"#;
    let parsed = RustExtractor.extract(code, "src/main.rs");

    // main.rs is crate root, so module_path should be None
    for symbol in &parsed.symbols {
        assert_eq!(
            symbol.module_path, None,
            "Symbol {} in main.rs should have None module_path",
            symbol.name
        );
    }
}

#[test]
fn test_mod_rs_symbols_have_parent_module_path() {
    let code = r#"
fn helper() {}
"#;
    let parsed = RustExtractor.extract(code, "src/api/mod.rs");

    // Symbols in mod.rs should have module_path pointing to "api" (the module itself)
    // The implicit module "api" should have module_path None (it's top-level)
    let implicit_module = parsed.symbols.iter().find(|s| s.kind == "module");
    assert!(implicit_module.is_some());
    assert_eq!(
        implicit_module.unwrap().module_path,
        None,
        "Implicit module 'api' should have None module_path (it's top-level)"
    );

    // Regular symbols should have module_path "api"
    let func = parsed.symbols.iter().find(|s| s.name == "helper");
    assert!(func.is_some());
    assert_eq!(
        func.unwrap().module_path.as_deref(),
        Some("api"),
        "Function in api/mod.rs should have module_path 'api'"
    );
}

// ============================================================================
// Phase 4: Implicit File Module Tests (RED → GREEN → REFACTOR)
// ============================================================================

#[test]
fn test_implicit_file_module_created() {
    let extractor = RustExtractor;
    let code = r#"
        fn top_level_function() {}
        struct TopStruct {}
    "#;

    let parsed = extractor.extract(code, "src/example.rs");

    // Should have implicit "example" module
    let module = parsed
        .symbols
        .iter()
        .find(|s| s.kind == "module" && s.name == "example");
    assert!(module.is_some(), "Should create implicit file module");

    let module = module.unwrap();
    assert_eq!(module.start_line, 1);
    assert_eq!(module.end_line, code.lines().count());
    assert!(
        module
            .signature
            .as_ref()
            .unwrap()
            .contains("implicit_module: true"),
        "Module signature should indicate it's implicit"
    );
}

#[test]
fn test_top_level_symbols_linked_to_file_module() {
    let extractor = RustExtractor;
    let code = r#"
        fn top_level_function() {}
        struct TopStruct {}
    "#;

    let parsed = extractor.extract(code, "src/example.rs");

    // Should have HasMember edges: example -> top_level_function, TopStruct
    let member_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMember))
        .collect();

    assert_eq!(
        member_edges.len(),
        2,
        "Should link both symbols to file module"
    );
}

// ============================================================================
// Phase 5: Module Deduplication and Test Propagation (resolve_file_modules)
// ============================================================================

#[test]
fn test_deduplicates_module_symbols() {
    use crate::a6s::extract::LanguageExtractor;

    // Simulate two files:
    // 1. src/config/mod.rs with "mod defaults;" declaration
    // 2. src/config/defaults.rs with content

    // File 1: Parent module with explicit declaration
    let mod_rs_code = r#"
mod defaults;

pub fn config_helper() {}
"#;
    let mod_parsed = RustExtractor.extract(mod_rs_code, "src/config/mod.rs");

    // File 2: defaults.rs with implicit module + content
    let defaults_code = r#"
pub fn get_defaults() -> String {
    "defaults".to_string()
}
"#;
    let defaults_parsed = RustExtractor.extract(defaults_code, "src/config/defaults.rs");

    // Count modules before resolution
    let modules_before: Vec<_> = defaults_parsed
        .symbols
        .iter()
        .filter(|s| s.kind == "module")
        .collect();

    // Should have 2 modules: one implicit from file, one explicit from declaration (in mod.rs)
    // But the explicit one is in mod_parsed, not defaults_parsed
    // defaults_parsed only has the implicit module
    assert_eq!(
        modules_before.len(),
        1,
        "Should have 1 implicit module before resolution"
    );
    assert!(
        modules_before[0]
            .signature
            .as_ref()
            .unwrap()
            .contains("implicit_module: true"),
        "Should be marked as implicit"
    );

    // Now call resolve_file_modules
    let mut files = vec![mod_parsed, defaults_parsed];
    RustExtractor.resolve_file_modules(&mut files);

    // After resolution, the implicit module should be updated (not removed)
    // because edges still reference it
    let modules_after: Vec<_> = files[1]
        .symbols
        .iter()
        .filter(|s| s.kind == "module")
        .collect();

    assert_eq!(
        modules_after.len(),
        1,
        "Implicit module should be kept (not removed) to preserve edges"
    );

    // Verify it's marked as resolved
    assert!(
        modules_after[0]
            .signature
            .as_ref()
            .unwrap()
            .contains("resolved_from_explicit_declaration: true"),
        "Module should be marked as resolved (not implicit anymore)"
    );
}

#[test]
fn test_propagates_test_attribute_to_file_symbols() {
    use crate::a6s::extract::LanguageExtractor;

    // File 1: mod.rs with #[cfg(test)] mod test_module;
    let mod_rs_code = r#"
#[cfg(test)]
mod mod_test;

pub fn regular_function() {}
"#;
    let mod_parsed = RustExtractor.extract(mod_rs_code, "src/config/mod.rs");

    // Verify the module declaration is marked as test
    let test_mod_decl = mod_parsed.symbols.iter().find(|s| s.name == "mod_test");
    assert!(test_mod_decl.is_some(), "Should find mod_test declaration");
    assert_eq!(
        test_mod_decl.unwrap().entry_type,
        Some("test".to_string()),
        "Module declaration should be marked as test"
    );

    // File 2: mod_test.rs with test functions
    let test_file_code = r#"
#[test]
fn test_something() {
    assert!(true);
}

fn helper_function() {
    // Not marked as test, but should inherit from module declaration
}
"#;
    let test_parsed = RustExtractor.extract(test_file_code, "src/config/mod_test.rs");

    // Before resolution, helper_function is NOT marked as test
    let helper_before = test_parsed
        .symbols
        .iter()
        .find(|s| s.name == "helper_function");
    assert!(helper_before.is_some());
    assert_eq!(
        helper_before.unwrap().entry_type,
        None,
        "helper_function should NOT be marked as test before resolution"
    );

    // Call resolve_file_modules
    let mut files = vec![mod_parsed, test_parsed];
    RustExtractor.resolve_file_modules(&mut files);

    // After resolution, ALL symbols in mod_test.rs should be marked as test
    let test_fn = files[1].symbols.iter().find(|s| s.name == "test_something");
    assert!(test_fn.is_some());
    assert_eq!(
        test_fn.unwrap().entry_type,
        Some("test".to_string()),
        "test_something should remain marked as test"
    );

    let helper_after = files[1]
        .symbols
        .iter()
        .find(|s| s.name == "helper_function");
    assert!(helper_after.is_some());
    assert_eq!(
        helper_after.unwrap().entry_type,
        Some("test".to_string()),
        "helper_function should be marked as test after resolution"
    );
}

#[test]
fn test_mod_rs_implicit_module_has_correct_name() {
    // Test that mod.rs files create an implicit module with the parent directory name,
    // and the module_path points to the PARENT module (not the module itself).
    let code = r#"
pub fn some_function() {
    println!("Hello");
}
"#;

    let parsed = RustExtractor.extract(code, "src/common/cmd/mod.rs");

    // Should have an implicit module symbol
    let implicit_module = parsed.symbols.iter().find(|s| {
        s.kind == "module"
            && s.signature
                .as_ref()
                .map(|sig| sig.contains("implicit_module: true"))
                .unwrap_or(false)
    });

    assert!(
        implicit_module.is_some(),
        "Should have implicit module symbol"
    );
    let module = implicit_module.unwrap();

    // Name should be "cmd" (parent directory), not "mod"
    assert_eq!(
        module.name, "cmd",
        "Implicit module name should be 'cmd' (parent dir), not 'mod'"
    );

    // Module path should be "common" (the PARENT module, not common::cmd)
    assert_eq!(
        module.module_path,
        Some("common".to_string()),
        "Module path should be 'common' (parent module)"
    );

    // Other symbols in the file should have module_path = "common::cmd"
    let func = parsed.symbols.iter().find(|s| s.name == "some_function");
    assert!(func.is_some());
    assert_eq!(
        func.unwrap().module_path,
        Some("common::cmd".to_string()),
        "Functions in the file should have module_path 'common::cmd'"
    );
}

#[test]
fn test_cross_file_module_hasmember_edges() {
    use crate::a6s::extract::LanguageExtractor;

    // Scenario: src/app/mod.rs declares `mod manager;`
    //           src/app/manager/mod.rs is the actual module definition
    // Expected: resolve_file_modules() should create a HasMember edge
    //           from app module (definition) to manager module (definition)

    // File 1: src/app/mod.rs with module declaration
    let app_mod_code = r#"
mod manager;

pub fn app_function() {}
"#;
    let app_parsed = RustExtractor.extract(app_mod_code, "src/app/mod.rs");

    // File 2: src/app/manager/mod.rs - the actual manager module
    let manager_code = r#"
pub fn manager_function() {}
"#;
    let manager_parsed = RustExtractor.extract(manager_code, "src/app/manager/mod.rs");

    // Call resolve_file_modules to create cross-file edges
    let mut files = vec![app_parsed, manager_parsed];
    RustExtractor.resolve_file_modules(&mut files);

    // After resolution, find the cross-file edge
    let edges_after: Vec<_> = files[0]
        .edges
        .iter()
        .chain(files[1].edges.iter())
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMember))
        .collect();

    // Find the cross-file edge pointing to the manager DEFINITION
    let cross_file_edge = edges_after.iter().find(|e| {
        if let crate::a6s::types::SymbolRef::Resolved(target_id) = &e.to {
            // SymbolId format: "symbol:file_path:name:line"
            target_id.as_str() == "symbol:src/app/manager/mod.rs:manager:1"
        } else {
            false
        }
    });

    assert!(
        cross_file_edge.is_some(),
        "Should have HasMember edge pointing to manager module DEFINITION in src/app/manager/mod.rs:1"
    );

    // Verify the edge source is the app module DEFINITION (not declaration)
    let edge = cross_file_edge.unwrap();
    if let crate::a6s::types::SymbolRef::Resolved(source_id) = &edge.from {
        assert_eq!(
            source_id.as_str(),
            "symbol:src/app/mod.rs:app:1",
            "Edge source should be the app module definition at src/app/mod.rs:1"
        );
    } else {
        panic!("Edge source should be resolved");
    }
}

// ============================================================================
// Phase 6: Cross-File Resolution Tests (resolve_cross_file)
// ============================================================================

#[test]
fn test_resolve_cross_file_calls_same_module() {
    let extractor = RustExtractor;

    // File 1: calls bar()
    let code1 = r#"fn foo() { bar(); }"#;
    let file1 = extractor.extract(code1, "src/utils/a.rs");

    // File 2: defines bar()
    let code2 = r#"fn bar() {}"#;
    let file2 = extractor.extract(code2, "src/utils/b.rs");

    let mut files = [file1, file2];
    let (resolved, _imports) = extractor.resolve_cross_file(&mut files);

    // Should resolve the Calls edge from foo -> bar
    let calls: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Calls)
        .collect();
    assert!(
        calls.iter().any(|e| e.to.as_str().contains(":bar:")),
        "Should resolve cross-file call to bar, got: {:?}",
        calls
    );
}

#[test]
fn test_resolve_cross_file_different_modules_unique_name() {
    let extractor = RustExtractor;

    // File 1 in src/api/: calls helper()
    let code1 = r#"fn handler() { helper(); }"#;
    let file1 = extractor.extract(code1, "src/api/handler.rs");

    // File 2 in src/db/: defines helper()
    let code2 = r#"fn helper() {}"#;
    let file2 = extractor.extract(code2, "src/db/utils.rs");

    let mut files = [file1, file2];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // With bare-name fallback (single candidate), this WILL resolve
    // because helper() is the only function with that name across all files
    let calls: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Calls)
        .collect();
    assert!(
        calls.iter().any(|e| e.to.as_str().contains(":helper:")),
        "Should resolve via bare-name fallback when only one candidate exists, got: {:?}",
        calls
    );
}

#[test]
fn test_resolve_cross_file_ambiguous_name_no_resolve() {
    let extractor = RustExtractor;

    // File 1: calls helper()
    let code1 = r#"fn handler() { helper(); }"#;
    let file1 = extractor.extract(code1, "src/api/handler.rs");

    // File 2: defines helper()
    let code2 = r#"fn helper() {}"#;
    let file2 = extractor.extract(code2, "src/db/utils.rs");

    // File 3: ALSO defines helper() — creates ambiguity
    let code3 = r#"fn helper() {}"#;
    let file3 = extractor.extract(code3, "src/core/helpers.rs");

    let mut files = [file1, file2, file3];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // With 2 candidates for "helper", bare-name fallback should NOT resolve
    let calls: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Calls)
        .collect();
    assert!(
        calls.is_empty(),
        "Should NOT resolve ambiguous bare names, got: {:?}",
        calls
    );
}

#[test]
fn test_resolve_cross_file_skips_file_imports() {
    let extractor = RustExtractor;

    let code = r#"use std::io::Result;"#;
    let file = extractor.extract(code, "src/main.rs");

    // After fix: no Import edges should be created at all in raw edges.
    // Imports are captured via RawImport entries, not RawEdge.
    let import_edge_count = file
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Import)
        .count();
    assert_eq!(
        import_edge_count, 0,
        "Should have ZERO Import edges in raw edges; imports use RawImport entries instead"
    );

    let mut files = [file];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // No Import edges in resolved output either
    let resolved_imports = resolved
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Import)
        .count();
    assert_eq!(
        resolved_imports, 0,
        "No Import edges should appear in resolved output"
    );
}

#[test]
fn test_resolve_cross_file_type_refs() {
    let extractor = RustExtractor;

    // File 1: function with unresolved type reference
    let code1 = r#"
struct Config {}
fn get_config() -> Config {
    Config {}
}
"#;
    let file1 = extractor.extract(code1, "src/config.rs");

    // File 2: function that uses Config type but it's defined elsewhere
    let code2 = r#"
fn process(c: Config) {}
"#;
    let file2 = extractor.extract(code2, "src/handler.rs");

    let mut files = [file1, file2];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // ParamType edge for process -> Config should resolve (single candidate)
    let param_types: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();
    assert!(
        param_types
            .iter()
            .any(|e| e.to.as_str().contains(":Config:")),
        "Should resolve cross-file ParamType to Config, got: {:?}",
        param_types
    );
}

#[test]
fn test_resolve_cross_file_implements_trait() {
    let extractor = RustExtractor;

    // File 1: defines a trait
    let code1 = r#"
trait Processor {
    fn process(&self);
}
"#;
    let file1 = extractor.extract(code1, "src/traits.rs");

    // File 2: implements the trait
    let code2 = r#"
struct MyProcessor;

impl Processor for MyProcessor {
    fn process(&self) {}
}
"#;
    let file2 = extractor.extract(code2, "src/impl.rs");

    let mut files = [file1, file2];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // Implements edge MyProcessor -> Processor should resolve (single candidate)
    let impl_edges: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Implements)
        .collect();
    assert!(
        impl_edges
            .iter()
            .any(|e| e.to.as_str().contains(":Processor:")),
        "Should resolve cross-file Implements to Processor, got: {:?}",
        impl_edges
    );
}

#[test]
fn test_resolve_cross_file_already_resolved_edges_skipped() {
    let extractor = RustExtractor;

    // File with same-file resolved edges (HasMember, HasField, etc.)
    let code = r#"
struct Foo {
    x: i32,
}
impl Foo {
    fn bar(&self) {}
}
"#;
    let file = extractor.extract(code, "src/foo.rs");

    let mut files = [file];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // Already-resolved same-file edges will be passed through by resolve_cross_file
    // since both from and to are Resolved. This test verifies the method handles
    // fully-resolved edges correctly without panicking.
    let _hasmember: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasMember)
        .collect();
    // Fully-resolved edges pass through (both from and to are Resolved)
    assert!(
        !resolved.is_empty(),
        "Already-resolved edges should pass through resolve_cross_file"
    );
}

#[test]
fn test_resolve_cross_file_no_imports_returned() {
    let extractor = RustExtractor;

    let code = r#"
use std::io::Result;
fn foo() {}
"#;
    let file = extractor.extract(code, "src/main.rs");

    let mut files = [file];
    let (_resolved, imports) = extractor.resolve_cross_file(&mut files);

    // Rust doesn't resolve imports through this path yet
    assert!(
        imports.is_empty(),
        "Rust resolve_cross_file should return empty imports for now"
    );
}
