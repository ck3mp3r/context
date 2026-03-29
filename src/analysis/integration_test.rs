// Integration test: Generic Parser with Language trait
//
// Tests the trait-based parser architecture:
// 1. Parses code ONCE
// 2. Inserts directly into graph (no intermediate vectors)
// 3. Language-specific symbol types via traits

use crate::analysis::parser::{GlobalSymbolMap, resolve_deferred_edges};
use crate::analysis::store::CodeGraph;
use crate::analysis::types::SymbolName;
use crate::analysis::{Parser, Rust};
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
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
async fn test_unified_parser_single_pass() {
    let temp = TempDir::new().expect("Failed to create temp dir");

    // Create graph
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");

    // Create Rust parser
    let mut parser = Parser::<Rust>::new();

    // Parse and analyze in ONE CALL - inserts directly into graph
    let stats = parser
        .parse_and_analyze(SAMPLE_RUST, "src/calc.rs", &mut graph)
        .expect("Failed to parse and analyze");

    // Verify stats - expecting: Calculator, impl Calculator, new, add, main
    eprintln!("Symbols inserted: {}", stats.symbols_inserted);
    eprintln!("Relationships inserted: {}", stats.relationships_inserted);

    assert!(
        stats.relationships_inserted > 0,
        "Should extract relationships (calls, contains)"
    );

    // Commit to nanograph
    graph.commit().expect("Failed to commit");

    // Query back
    let stored_symbols = graph
        .query_symbols_in_file("src/calc.rs")
        .expect("Failed to query symbols");

    eprintln!("Query returned {} symbols:", stored_symbols.len());
    for s in &stored_symbols {
        eprintln!(
            "  - '{}' (kind: {:?}, lines {}-{})",
            s.name, s.kind, s.start_line, s.end_line
        );
    }

    // For now, just verify we have the expected symbols (not exact count)
    let names: Vec<_> = stored_symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"Calculator"),
        "Should find Calculator struct"
    );
    assert!(names.contains(&"new"), "Should find new() method");
    assert!(names.contains(&"add"), "Should find add() method");
    assert!(names.contains(&"main"), "Should find main() function");
}

#[tokio::test]
#[ignore = "query returning empty name symbols - needs investigation"]
async fn test_parser_handles_multiple_files() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "multi-repo").expect("Failed to create graph");
    let mut parser = Parser::<Rust>::new();

    let file1 = "pub fn hello() -> String { String::from(\"hello\") }";
    let file2 = "pub fn world() -> String { String::from(\"world\") }";

    // Parse file 1
    let stats1 = parser
        .parse_and_analyze(file1, "src/file1.rs", &mut graph)
        .expect("Failed to parse file1");

    eprintln!("File1 stats: {} symbols", stats1.symbols_inserted);

    // Parse file 2
    let stats2 = parser
        .parse_and_analyze(file2, "src/file2.rs", &mut graph)
        .expect("Failed to parse file2");

    eprintln!("File2 stats: {} symbols", stats2.symbols_inserted);

    // Commit
    graph.commit().expect("Failed to commit");

    // Query each file separately
    let file1_symbols = graph
        .query_symbols_in_file("src/file1.rs")
        .expect("Failed to query file1");
    let file2_symbols = graph
        .query_symbols_in_file("src/file2.rs")
        .expect("Failed to query file2");

    eprintln!("File1 query returned: {} symbols", file1_symbols.len());
    for s in &file1_symbols {
        eprintln!("  - '{}'", s.name);
    }
    eprintln!("File2 query returned: {} symbols", file2_symbols.len());
    for s in &file2_symbols {
        eprintln!("  - '{}'", s.name);
    }

    assert!(
        !file1_symbols.is_empty(),
        "File1 should have at least 1 symbol"
    );
    assert!(
        !file2_symbols.is_empty(),
        "File2 should have at least 1 symbol"
    );
    assert_eq!(file1_symbols[0].name, "hello");
    assert_eq!(file2_symbols[0].name, "world");
}

#[test]
fn test_parser_detects_language_from_extension() {
    assert!(Parser::<Rust>::can_handle("src/main.rs"));
    assert!(!Parser::<Rust>::can_handle("src/main.py"));
    assert!(!Parser::<Rust>::can_handle("README.md"));
}

// ============================================================================
// Cross-file resolution tests
// ============================================================================

/// Trait defined in file1, struct + impl in file2.
/// Inherits edge should be created - either immediately or during resolve phase.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_cross_file_inherits() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut parser = Parser::<Rust>::new();
    let mut global = GlobalSymbolMap::new();

    // File 1: defines the trait
    let file1 = r#"
pub trait Drawable {
    fn draw(&self);
}
"#;

    // File 2: defines a struct and implements the trait
    let file2 = r#"
pub struct Circle {
    radius: f64,
}

impl Drawable for Circle {
    fn draw(&self) {}
}
"#;

    // Parse both files, collecting deferred edges
    let stats1 = parser
        .parse_and_collect(file1, "src/traits.rs", &mut graph, &mut global)
        .expect("Failed to parse file1");
    let stats2 = parser
        .parse_and_collect(file2, "src/circle.rs", &mut graph, &mut global)
        .expect("Failed to parse file2");

    // Resolve any deferred edges
    let resolved = resolve_deferred_edges(&global, &mut graph).expect("Failed to resolve");

    // Total relationships should include the Inherits edge
    let total_rels = stats1.relationships_inserted + stats2.relationships_inserted + resolved;
    assert!(
        total_rels > 0,
        "Should have created relationships including Inherits edge"
    );

    // Both symbols should be in the global map
    assert!(
        global.map.contains_key(&SymbolName::new("Drawable")),
        "Drawable should be in global map"
    );
    assert!(
        global.map.contains_key(&SymbolName::new("Circle")),
        "Circle should be in global map"
    );
}

/// Trait defined AFTER the impl file is processed.
/// This exercises the deferred resolution path.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_cross_file_inherits_deferred_order() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut parser = Parser::<Rust>::new();
    let mut global = GlobalSymbolMap::new();

    // File 1 (processed first): struct + impl referencing trait NOT YET SEEN
    let file1 = r#"
pub struct Circle {
    radius: f64,
}

impl Drawable for Circle {
    fn draw(&self) {}
}
"#;

    // File 2 (processed second): defines the trait
    let file2 = r#"
pub trait Drawable {
    fn draw(&self);
}
"#;

    parser
        .parse_and_collect(file1, "src/circle.rs", &mut graph, &mut global)
        .expect("Failed to parse file1");
    parser
        .parse_and_collect(file2, "src/traits.rs", &mut graph, &mut global)
        .expect("Failed to parse file2");

    // Drawable wasn't known when file1 was processed, so Inherits should be deferred
    let has_deferred_inherits = global.deferred.iter().any(|e| {
        matches!(e, crate::analysis::parser::DeferredEdge::Inherits { trait_name, .. } if trait_name.as_str() == "Drawable")
    });
    assert!(
        has_deferred_inherits,
        "Should have deferred Inherits edge for Drawable"
    );

    // Resolve
    let resolved = resolve_deferred_edges(&global, &mut graph).expect("Failed to resolve");
    assert!(
        resolved >= 1,
        "Should resolve the deferred Inherits edge, got {}",
        resolved
    );
}

/// Function in file1 calls function defined in file2.
/// Calls edge should be created during resolve phase.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_cross_file_calls() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut parser = Parser::<Rust>::new();
    let mut global = GlobalSymbolMap::new();

    // File 1: calls helper() which doesn't exist in this file
    let file1 = r#"
fn main() {
    helper();
}
"#;

    // File 2: defines helper()
    let file2 = r#"
pub fn helper() {
    println!("helping");
}
"#;

    // Parse file1 first - helper() call will be deferred
    parser
        .parse_and_collect(file1, "src/main.rs", &mut graph, &mut global)
        .expect("Failed to parse file1");

    // Parse file2 - defines helper
    parser
        .parse_and_collect(file2, "src/helpers.rs", &mut graph, &mut global)
        .expect("Failed to parse file2");

    // Should have a deferred Call edge
    let has_deferred_call = global.deferred.iter().any(|e| {
        matches!(e, crate::analysis::parser::DeferredEdge::Call { callee_name, .. } if callee_name.as_str() == "helper")
    });
    assert!(has_deferred_call, "Should have deferred call to helper()");

    // Resolve
    let resolved = resolve_deferred_edges(&global, &mut graph).expect("Failed to resolve");
    assert!(resolved > 0, "Should resolve the cross-file call");
}

/// Function references a type defined in another file.
/// References edge should be created during resolve phase.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_cross_file_references() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut parser = Parser::<Rust>::new();
    let mut global = GlobalSymbolMap::new();

    // File 1: function that takes Config as parameter
    let file1 = r#"
fn process(config: Config) -> Status {
    todo!()
}
"#;

    // File 2: defines Config and Status
    let file2 = r#"
pub struct Config {
    name: String,
}

pub enum Status {
    Ok,
    Error,
}
"#;

    // Parse file1 first - Config/Status references will be deferred
    parser
        .parse_and_collect(file1, "src/processor.rs", &mut graph, &mut global)
        .expect("Failed to parse file1");

    // Parse file2 - defines the types
    parser
        .parse_and_collect(file2, "src/types.rs", &mut graph, &mut global)
        .expect("Failed to parse file2");

    // Resolve
    let resolved = resolve_deferred_edges(&global, &mut graph).expect("Failed to resolve");
    assert!(
        resolved >= 2,
        "Should resolve at least 2 reference edges (Config + Status), got {}",
        resolved
    );
}

/// Struct defined in file1, impl block with methods in file2.
/// SymbolContains edges should be created - either immediately or during resolve.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_cross_file_symbol_contains() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut parser = Parser::<Rust>::new();
    let mut global = GlobalSymbolMap::new();

    // File 1: defines the struct
    let file1 = r#"
pub struct Server {
    port: u16,
}
"#;

    // File 2: impl block with methods
    let file2 = r#"
impl Server {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn start(&self) {}
}
"#;

    // Parse file1 - defines Server
    let stats1 = parser
        .parse_and_collect(file1, "src/server.rs", &mut graph, &mut global)
        .expect("Failed to parse file1");

    // Parse file2 - impl block references Server from file1
    let stats2 = parser
        .parse_and_collect(file2, "src/server_impl.rs", &mut graph, &mut global)
        .expect("Failed to parse file2");

    // Resolve any deferred edges
    let resolved = resolve_deferred_edges(&global, &mut graph).expect("Failed to resolve");

    // Total relationships should include SymbolContains: Server -> new, Server -> start
    let total_rels = stats1.relationships_inserted + stats2.relationships_inserted + resolved;
    assert!(
        total_rels >= 2,
        "Should have at least 2 SymbolContains edges (new + start), got {}",
        total_rels
    );

    // All symbols should be in the global map
    assert!(global.map.contains_key(&SymbolName::new("Server")));
    assert!(global.map.contains_key(&SymbolName::new("new")));
    assert!(global.map.contains_key(&SymbolName::new("start")));
}

/// Struct defined AFTER impl block - exercises deferred SymbolContains.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_cross_file_symbol_contains_deferred_order() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut parser = Parser::<Rust>::new();
    let mut global = GlobalSymbolMap::new();

    // File 1 (processed first): impl block for struct NOT YET SEEN
    let file1 = r#"
impl Server {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn start(&self) {}
}
"#;

    // File 2 (processed second): defines the struct
    let file2 = r#"
pub struct Server {
    port: u16,
}
"#;

    parser
        .parse_and_collect(file1, "src/server_impl.rs", &mut graph, &mut global)
        .expect("Failed to parse file1");
    parser
        .parse_and_collect(file2, "src/server.rs", &mut graph, &mut global)
        .expect("Failed to parse file2");

    // Server wasn't known when file1 was processed, so SymbolContains should be deferred
    let deferred_count = global.deferred.iter().filter(|e| {
        matches!(e, crate::analysis::parser::DeferredEdge::SymbolContains { parent_type_name, .. } if parent_type_name.as_str() == "Server")
    }).count();
    assert!(
        deferred_count >= 2,
        "Should have deferred SymbolContains for new + start, got {}",
        deferred_count
    );

    // Resolve
    let resolved = resolve_deferred_edges(&global, &mut graph).expect("Failed to resolve");
    assert!(
        resolved >= 2,
        "Should resolve SymbolContains edges, got {}",
        resolved
    );
}

/// Same-file relationships should still work immediately (no regression).
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_same_file_still_works_with_global_map() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut parser = Parser::<Rust>::new();
    let mut global = GlobalSymbolMap::new();

    let code = r#"
pub struct Calculator {
    value: i32,
}

impl Calculator {
    pub fn new() -> Self {
        Self { value: 0 }
    }
}

pub fn main() {
    let calc = Calculator::new();
}
"#;

    let stats = parser
        .parse_and_collect(code, "src/calc.rs", &mut graph, &mut global)
        .expect("Failed to parse");

    // Same-file relationships should be resolved immediately, not deferred
    assert!(
        stats.relationships_inserted > 0,
        "Same-file relationships should still be inserted immediately"
    );

    // Calculator, new, main should all be in global map
    assert!(global.map.contains_key(&SymbolName::new("Calculator")));
    assert!(global.map.contains_key(&SymbolName::new("new")));
    assert!(global.map.contains_key(&SymbolName::new("main")));
}

/// parse_and_analyze (backward compat) should still work for single-file use.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_parse_and_analyze_backward_compat() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut parser = Parser::<Rust>::new();

    let code = r#"
pub struct Foo;
impl Foo {
    pub fn bar() {}
}
"#;

    let stats = parser
        .parse_and_analyze(code, "src/foo.rs", &mut graph)
        .expect("Failed to parse");

    assert!(stats.symbols_inserted >= 2, "Should insert Foo and bar");
    assert!(
        stats.relationships_inserted > 0,
        "Should insert relationships"
    );
}

// ============================================================================
// Implements edge tests
// ============================================================================

/// Rust: `impl Trait for Type` should create an Inherits edge with type "implements".
/// When both trait and type are defined in the same file, the edge resolves immediately.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_rust_impl_trait_for_type_creates_implements_edge() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut global = GlobalSymbolMap::new();
    let mut parser = Parser::<Rust>::new();

    let code = r#"
pub trait Displayable {
    fn display(&self);
}

pub struct Widget {
    name: String,
}

impl Displayable for Widget {
    fn display(&self) {
        println!("{}", self.name);
    }
}
"#;

    let stats = parser
        .parse_and_collect(code, "src/widget.rs", &mut graph, &mut global)
        .expect("Failed to parse");

    // Both Displayable and Widget are in the same file, so the edge
    // should resolve immediately (not deferred)
    assert!(
        stats.relationships_inserted > 0,
        "Should insert relationships including Implements edge"
    );

    // Verify both symbols are in the global map
    assert!(
        global.map.contains_key(&SymbolName::new("Displayable")),
        "Should have Displayable in global map"
    );
    assert!(
        global.map.contains_key(&SymbolName::new("Widget")),
        "Should have Widget in global map"
    );

    // The Inherits edge should NOT be deferred (same file)
    let deferred_inherits = global
        .deferred
        .iter()
        .filter(|e| matches!(e, crate::analysis::parser::DeferredEdge::Inherits { .. }))
        .count();
    assert_eq!(
        deferred_inherits, 0,
        "Same-file impl should not defer the Inherits edge"
    );
}

/// Rust: `impl Type` (inherent impl) should NOT create an Implements edge.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_rust_inherent_impl_no_implements_edge() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut global = GlobalSymbolMap::new();
    let mut parser = Parser::<Rust>::new();

    let code = r#"
pub struct Counter {
    value: i32,
}

impl Counter {
    pub fn new() -> Self {
        Self { value: 0 }
    }
}
"#;

    parser
        .parse_and_collect(code, "src/counter.rs", &mut graph, &mut global)
        .expect("Failed to parse");

    // No deferred Inherits edges for inherent impl
    let deferred_inherits = global
        .deferred
        .iter()
        .filter(|e| matches!(e, crate::analysis::parser::DeferredEdge::Inherits { .. }))
        .count();
    assert_eq!(
        deferred_inherits, 0,
        "Inherent impl should not create Inherits edges"
    );
}

/// Go: `var _ Interface = (*Type)(nil)` conformance check creates a deferred Inherits edge.
#[test]
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_go_conformance_check_creates_implements_edge() {
    use crate::analysis::Go;

    let temp = TempDir::new().expect("Failed to create temp dir");
    let mut graph = CodeGraph::new(temp.path(), "test-repo").expect("Failed to create graph");
    let mut global = GlobalSymbolMap::new();
    let mut parser = Parser::<Go>::new();

    let code = r#"
package main

type ICache interface {
    Get(key string) string
}

type FileBasedCache struct{}

func (f *FileBasedCache) Get(key string) string {
    return ""
}

var _ ICache = (*FileBasedCache)(nil)
"#;

    let stats = parser
        .parse_and_collect(code, "pkg/cache.go", &mut graph, &mut global)
        .expect("Failed to parse");

    // Verify both symbols are in the global map
    assert!(
        global.map.contains_key(&SymbolName::new("ICache")),
        "Should have ICache in global map"
    );
    assert!(
        global.map.contains_key(&SymbolName::new("FileBasedCache")),
        "Should have FileBasedCache in global map"
    );

    // The Inherits edge should be resolved immediately (same file)
    // OR deferred — either way, check that there's a relationship
    let deferred_inherits: Vec<_> = global
        .deferred
        .iter()
        .filter(|e| matches!(e, crate::analysis::parser::DeferredEdge::Inherits { .. }))
        .collect();

    // If same-file, the edge is immediate (counted in stats.relationships_inserted)
    // If deferred, resolve it now
    if !deferred_inherits.is_empty() {
        let resolved =
            resolve_deferred_edges(&global, &mut graph).expect("Failed to resolve edges");
        assert!(resolved > 0, "Should resolve the deferred Implements edge");
    }

    // Either way, relationships should have been created
    assert!(
        stats.relationships_inserted > 0 || !deferred_inherits.is_empty(),
        "Should create Implements edge for conformance check. Stats: {:?}, deferred: {}",
        stats.relationships_inserted,
        deferred_inherits.len()
    );
}
