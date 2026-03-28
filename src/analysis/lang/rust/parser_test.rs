// Tests for Rust language parser

use crate::analysis::lang::rust::{Kind, Rust};
use crate::analysis::parser::Language;
use tree_sitter::{Node, Parser};

/// Helper: parse code and find first node of given kind
fn parse_and_find<'a>(tree: &'a tree_sitter::Tree, node_kind: &str) -> Option<Node<'a>> {
    fn find_node<'b>(node: Node<'b>, kind: &str) -> Option<Node<'b>> {
        if node.kind() == kind {
            return Some(node);
        }
        for child in node.children(&mut node.walk()) {
            if let Some(found) = find_node(child, kind) {
                return Some(found);
            }
        }
        None
    }
    find_node(tree.root_node(), node_kind)
}

/// Helper: parse code and collect all symbols
fn extract_all_symbols(code: &str) -> Vec<(Kind, String)> {
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let mut symbols = Vec::new();
    fn collect(node: Node, code: &str, symbols: &mut Vec<(Kind, String)>) {
        if let Some(sym) = Rust::parse_symbol(node, code) {
            symbols.push(sym);
        }
        for child in node.children(&mut node.walk()) {
            collect(child, code, symbols);
        }
    }
    collect(tree.root_node(), code, &mut symbols);
    symbols
}

// ============================================================================
// Symbol extraction tests
// ============================================================================

#[test]
fn test_parse_function() {
    let symbols = extract_all_symbols("fn hello() {}");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Function, "hello".to_string()));
}

#[test]
fn test_parse_struct() {
    let symbols = extract_all_symbols("struct Point { x: i32, y: i32 }");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Struct, "Point".to_string()));
}

#[test]
fn test_parse_enum() {
    let symbols = extract_all_symbols("enum Color { Red, Green, Blue }");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Enum, "Color".to_string()));
}

#[test]
fn test_parse_trait() {
    let symbols = extract_all_symbols("trait Drawable { fn draw(&self); }");
    assert_eq!(symbols.len(), 1); // trait method signatures are not function_items
    assert_eq!(symbols[0], (Kind::Trait, "Drawable".to_string()));
}

#[test]
fn test_parse_trait_with_default_method() {
    let symbols = extract_all_symbols("trait Drawable { fn draw(&self) {} }");
    assert_eq!(symbols.len(), 2); // default methods with bodies ARE function_items
    assert_eq!(symbols[0], (Kind::Trait, "Drawable".to_string()));
    assert_eq!(symbols[1], (Kind::Function, "draw".to_string()));
}

#[test]
fn test_parse_const() {
    let symbols = extract_all_symbols("const MAX: i32 = 100;");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Const, "MAX".to_string()));
}

#[test]
fn test_parse_static() {
    let symbols = extract_all_symbols("static COUNTER: i32 = 0;");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Static, "COUNTER".to_string()));
}

#[test]
fn test_parse_type_alias() {
    let symbols = extract_all_symbols("type Result<T> = std::result::Result<T, MyError>;");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Type, "Result".to_string()));
}

#[test]
fn test_parse_mod() {
    let symbols = extract_all_symbols("mod utils;");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Mod, "utils".to_string()));
}

#[test]
fn test_parse_impl_block() {
    let symbols = extract_all_symbols(
        r#"
struct Foo;
impl Foo {
    fn bar(&self) {}
    fn baz() {}
}
"#,
    );
    let names: Vec<_> = symbols.iter().map(|s| (&s.0, s.1.as_str())).collect();
    assert!(names.contains(&(&Kind::Struct, "Foo")));
    assert!(names.contains(&(&Kind::Function, "bar")));
    assert!(names.contains(&(&Kind::Function, "baz")));
}

#[test]
fn test_parse_mixed_file() {
    let symbols = extract_all_symbols(
        r#"
const VERSION: &str = "1.0";
static mut COUNT: u32 = 0;

struct Config {
    name: String,
}

enum Status {
    Active,
    Inactive,
}

trait Service {
    fn start(&self);
}

type BoxedService = Box<dyn Service>;

mod helpers;

fn main() {}
"#,
    );
    let kinds: Vec<_> = symbols.iter().map(|s| (&s.0, s.1.as_str())).collect();
    assert!(kinds.contains(&(&Kind::Const, "VERSION")));
    assert!(kinds.contains(&(&Kind::Static, "COUNT")));
    assert!(kinds.contains(&(&Kind::Struct, "Config")));
    assert!(kinds.contains(&(&Kind::Enum, "Status")));
    assert!(kinds.contains(&(&Kind::Trait, "Service")));
    // fn start(&self); is a signature, not a function_item - not extracted as symbol
    assert!(kinds.contains(&(&Kind::Type, "BoxedService")));
    assert!(kinds.contains(&(&Kind::Mod, "helpers")));
    assert!(kinds.contains(&(&Kind::Function, "main")));
}

// ============================================================================
// Callee extraction tests
// ============================================================================

#[test]
fn test_extract_simple_call() {
    let code = "fn foo() { bar(); }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let call_node = parse_and_find(&tree, "call_expression").unwrap();
    let callee = Rust::extract_callee(call_node, code);
    assert_eq!(callee, Some("bar".to_string()));
}

#[test]
fn test_extract_scoped_call() {
    let code = "fn foo() { String::new(); }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let call_node = parse_and_find(&tree, "call_expression").unwrap();
    let callee = Rust::extract_callee(call_node, code);
    assert_eq!(callee, Some("new".to_string()));
}

#[test]
fn test_extract_method_call() {
    let code = "fn foo() { obj.method(); }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let call_node = parse_and_find(&tree, "call_expression").unwrap();
    // method calls are field_expression, not call_expression in tree-sitter
    // The callee extraction should still work if found
    let callee = Rust::extract_callee(call_node, code);
    assert_eq!(callee, Some("method".to_string()));
}

// ============================================================================
// Impl block parsing tests
// ============================================================================

#[test]
fn test_parse_impl_simple() {
    let code = "impl Foo { fn bar() {} }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let impl_node = parse_and_find(&tree, "impl_item").unwrap();
    let info = Rust::parse_impl(impl_node, code).unwrap();
    assert_eq!(info.target_type, "Foo");
    assert_eq!(info.trait_name, None);
}

#[test]
fn test_parse_impl_trait_for_type() {
    let code = "impl Display for Foo { fn fmt(&self) {} }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let impl_node = parse_and_find(&tree, "impl_item").unwrap();
    let info = Rust::parse_impl(impl_node, code).unwrap();
    assert_eq!(info.target_type, "Foo");
    assert_eq!(info.trait_name, Some("Display".to_string()));
}

// ============================================================================
// Signature extraction tests
// ============================================================================

#[test]
fn test_extract_function_signature() {
    let code = "fn add(a: i32, b: i32) -> i32 { a + b }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_item").unwrap();
    let sig = Rust::extract_signature(fn_node, code);
    assert_eq!(sig, Some("fn add(a: i32, b: i32) -> i32".to_string()));
}

#[test]
fn test_extract_pub_function_signature() {
    let code = "pub fn hello(name: &str) -> String { format!(\"hi {}\", name) }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_item").unwrap();
    let sig = Rust::extract_signature(fn_node, code);
    assert_eq!(sig, Some("pub fn hello(name: &str) -> String".to_string()));
}

#[test]
fn test_extract_trait_signature() {
    let code = "trait Drawable { fn draw(&self); }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let trait_node = parse_and_find(&tree, "trait_item").unwrap();
    let sig = Rust::extract_signature(trait_node, code);
    assert_eq!(sig, Some("trait Drawable".to_string()));
}

// ============================================================================
// Type reference extraction tests
// ============================================================================

#[test]
fn test_extract_type_refs_from_function() {
    let code = "fn process(config: Config) -> Status { todo!() }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_item").unwrap();
    let refs = Rust::extract_type_references(fn_node, code);
    let names: Vec<_> = refs.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"Config"), "Should reference Config");
    assert!(names.contains(&"Status"), "Should reference Status");
}

#[test]
fn test_type_refs_skip_builtins() {
    let code = "fn process(x: i32, name: String) -> bool { true }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_item").unwrap();
    let refs = Rust::extract_type_references(fn_node, code);
    // i32, String, bool are all builtins - should be empty
    assert!(
        refs.is_empty(),
        "Should skip builtin types, got: {:?}",
        refs
    );
}

// ============================================================================
// Supertrait edge bug regression test
// ============================================================================

/// `impl<'a> Trait for GenericType<'a>` should extract correct target type name.
/// Bug: parse_impl was extracting wrong names for generic impl blocks.
#[test]
fn test_parse_impl_with_lifetime_generics() {
    let code = "impl<'a> ProjectRepository for SqliteProjectRepository<'a> { fn get(&self) {} }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let impl_node = parse_and_find(&tree, "impl_item").unwrap();
    let info = Rust::parse_impl(impl_node, code).unwrap();
    assert_eq!(info.trait_name, Some("ProjectRepository".to_string()));
    assert_eq!(
        info.target_type, "SqliteProjectRepository",
        "Should extract SqliteProjectRepository, not just the first type_identifier"
    );
}

/// Trait with associated type bounds should NOT produce self-referencing Inherits edges.
/// Bug: `trait Database { type Projects<'a>: ProjectRepository }` was producing
/// `ProjectRepository -> ProjectRepository` Inherits edges.
#[test]
fn test_supertrait_bounds_no_self_inherits() {
    use crate::analysis::parser::{resolve_deferred_edges, GlobalSymbolMap};
    use crate::analysis::store::CodeGraph;
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let mut graph = CodeGraph::new(temp.path(), "test-repo").unwrap();
    let mut parser = crate::analysis::parser::Parser::<crate::analysis::lang::rust::Rust>::new();
    let mut global = GlobalSymbolMap::new();

    let code = r#"
pub trait ProjectRepository {
    fn get_project(&self) -> String;
}

pub trait RepoRepository {
    fn get_repo(&self) -> String;
}

pub trait Database: Send + Sync {
    type Projects<'a>: ProjectRepository where Self: 'a;
    type Repos<'a>: RepoRepository where Self: 'a;

    fn projects(&self) -> Self::Projects<'_>;
    fn repos(&self) -> Self::Repos<'_>;
}
"#;

    parser
        .parse_and_collect(code, "src/db/repository.rs", &mut graph, &mut global)
        .unwrap();
    resolve_deferred_edges(&global, &mut graph).unwrap();

    // Should NOT have any self-referencing Inherits edges
    let self_ref_inherits: Vec<_> = global
        .deferred
        .iter()
        .filter(|e| {
            matches!(e, crate::analysis::parser::DeferredEdge::Inherits { type_name, trait_name }
                if type_name == trait_name)
        })
        .collect();
    assert!(
        self_ref_inherits.is_empty(),
        "Should not have self-referencing Inherits edges, got: {:?}",
        self_ref_inherits
    );
}
