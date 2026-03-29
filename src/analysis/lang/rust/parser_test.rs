// Tests for Rust language parser

use crate::analysis::lang::rust::{Kind, Rust};
use crate::analysis::parser::Language;
use crate::analysis::types::ReferenceType;
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
fn test_extract_type_refs_from_function_is_empty() {
    // Function type refs are now handled by extract_return_types and extract_param_types.
    // extract_type_references should return empty for function_item nodes.
    let code = "fn process(config: Config) -> Status { todo!() }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_item").unwrap();
    let refs = Rust::extract_type_references(fn_node, code);
    assert!(
        refs.is_empty(),
        "Function type refs should be empty, got: {:?}",
        refs
    );
}

#[test]
fn test_type_refs_skip_builtins() {
    let code = r#"
struct Data {
    count: i32,
    name: String,
    flag: bool,
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let struct_node = parse_and_find(&tree, "struct_item").unwrap();
    let refs = Rust::extract_type_references(struct_node, code);
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
#[cfg_attr(
    not(feature = "nanograph-tests"),
    ignore = "requires nanograph CLI - disabled in CI"
)]
fn test_supertrait_bounds_no_self_inherits() {
    use crate::analysis::parser::{GlobalSymbolMap, resolve_deferred_edges};
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

// ============================================================================
// Usage extraction tests
// ============================================================================

/// Helper: parse Rust code as a function_item and extract usages
fn extract_usages_from_func(code: &str) -> Vec<(String, usize)> {
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_item").expect("should find function_item");
    Rust::extract_usages(fn_node, code)
        .into_iter()
        .map(|(name, line)| (name.as_str().to_string(), line))
        .collect()
}

#[test]
fn test_extract_usages_simple_const_reference() {
    let code = r#"
fn do_work() {
    println!("{}", MAX_RETRIES);
}
"#;
    let usages = extract_usages_from_func(code);
    let names: Vec<&str> = usages.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"MAX_RETRIES"),
        "Should detect usage of MAX_RETRIES, got: {:?}",
        names
    );
}

#[test]
fn test_extract_usages_excludes_local_let_bindings() {
    let code = r#"
fn do_work() {
    let local_var = 42;
    println!("{}", local_var);
    println!("{}", GLOBAL_VAR);
}
"#;
    let usages = extract_usages_from_func(code);
    let names: Vec<&str> = usages.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        !names.contains(&"local_var"),
        "Should NOT include let binding, got: {:?}",
        names
    );
    assert!(
        names.contains(&"GLOBAL_VAR"),
        "Should include module-level reference, got: {:?}",
        names
    );
}

#[test]
fn test_extract_usages_excludes_parameters() {
    let code = r#"
fn do_work(config: Config, count: usize) {
    println!("{}", config);
    println!("{}", GLOBAL_VAR);
}
"#;
    let usages = extract_usages_from_func(code);
    let names: Vec<&str> = usages.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        !names.contains(&"config"),
        "Should NOT include parameter, got: {:?}",
        names
    );
    assert!(
        !names.contains(&"count"),
        "Should NOT include parameter, got: {:?}",
        names
    );
    assert!(
        names.contains(&"GLOBAL_VAR"),
        "Should include module-level reference, got: {:?}",
        names
    );
}

#[test]
fn test_extract_usages_deduplicates() {
    let code = r#"
fn do_work() {
    println!("{}", MAX_RETRIES);
    println!("{}", MAX_RETRIES);
    println!("{}", MAX_RETRIES);
}
"#;
    let usages = extract_usages_from_func(code);
    let max_count = usages.iter().filter(|(n, _)| n == "MAX_RETRIES").count();
    assert_eq!(
        max_count, 1,
        "Should deduplicate — one usage per symbol, got {} occurrences",
        max_count
    );
}

#[test]
fn test_extract_usages_excludes_for_loop_bindings() {
    let code = r#"
fn do_work() {
    for item in global_items.iter() {
        println!("{}", item);
    }
}
"#;
    let usages = extract_usages_from_func(code);
    let names: Vec<&str> = usages.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        !names.contains(&"item"),
        "Should NOT include for-loop binding, got: {:?}",
        names
    );
    assert!(
        names.contains(&"global_items"),
        "Should include the iterated collection, got: {:?}",
        names
    );
}

#[test]
fn test_extract_usages_non_function_returns_empty() {
    let code = r#"
const MAX_RETRIES: u32 = 3;
"#;
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let const_node = parse_and_find(&tree, "const_item").expect("should find const_item");
    let usages = Rust::extract_usages(const_node, code);
    assert!(
        usages.is_empty(),
        "Should return empty for non-function nodes, got: {:?}",
        usages
    );
}

#[test]
fn test_extract_usages_if_let_binding_excluded() {
    let code = r#"
fn do_work() {
    if let Some(value) = get_config() {
        println!("{}", value);
        println!("{}", DEFAULT_VALUE);
    }
}
"#;
    let usages = extract_usages_from_func(code);
    let names: Vec<&str> = usages.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        !names.contains(&"value"),
        "Should NOT include if-let binding, got: {:?}",
        names
    );
    assert!(
        names.contains(&"DEFAULT_VALUE"),
        "Should include module-level reference, got: {:?}",
        names
    );
}

// ============================================================================
// Return type extraction tests
// ============================================================================

/// Helper: extract return types from the first function in code
fn extract_return_types_from_func(code: &str) -> Vec<String> {
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_item").expect("should find function_item");
    Rust::extract_return_types(fn_node, code)
        .into_iter()
        .map(|n| n.as_str().to_string())
        .collect()
}

#[test]
fn test_extract_return_type_simple() {
    let code = "fn create() -> Config { todo!() }";
    let types = extract_return_types_from_func(code);
    assert_eq!(types, vec!["Config"]);
}

#[test]
fn test_extract_return_type_option() {
    let code = "fn find() -> Option<Config> { todo!() }";
    let types = extract_return_types_from_func(code);
    assert!(
        types.contains(&"Config".to_string()),
        "Should unwrap Option to find Config, got: {:?}",
        types
    );
}

#[test]
fn test_extract_return_type_result() {
    let code = "fn load() -> Result<Config, AppError> { todo!() }";
    let types = extract_return_types_from_func(code);
    assert!(
        types.contains(&"Config".to_string()),
        "Should extract Config from Result, got: {:?}",
        types
    );
    assert!(
        types.contains(&"AppError".to_string()),
        "Should extract AppError from Result, got: {:?}",
        types
    );
}

#[test]
fn test_extract_return_type_builtin_only() {
    let code = "fn count() -> usize { 0 }";
    let types = extract_return_types_from_func(code);
    assert!(
        types.is_empty(),
        "Should skip builtin return types, got: {:?}",
        types
    );
}

#[test]
fn test_extract_return_type_no_return() {
    let code = "fn do_work() { }";
    let types = extract_return_types_from_func(code);
    assert!(
        types.is_empty(),
        "Function with no return type should return empty, got: {:?}",
        types
    );
}

#[test]
fn test_extract_return_type_non_function() {
    let code = "struct Config { name: String }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let struct_node = parse_and_find(&tree, "struct_item").expect("should find struct_item");
    let types = Rust::extract_return_types(struct_node, code);
    assert!(
        types.is_empty(),
        "Non-function nodes should return empty, got: {:?}",
        types
    );
}

// ============================================================================
// Parameter type extraction tests
// ============================================================================

/// Helper: extract param types from the first function in code
fn extract_param_types_from_func(code: &str) -> Vec<String> {
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_item").expect("should find function_item");
    Rust::extract_param_types(fn_node, code)
        .into_iter()
        .map(|n| n.as_str().to_string())
        .collect()
}

#[test]
fn test_extract_param_type_simple() {
    let code = "fn process(config: Config) { }";
    let types = extract_param_types_from_func(code);
    assert_eq!(types, vec!["Config"]);
}

#[test]
fn test_extract_param_type_reference() {
    let code = "fn process(config: &Config) { }";
    let types = extract_param_types_from_func(code);
    assert!(
        types.contains(&"Config".to_string()),
        "Should unwrap & to find Config, got: {:?}",
        types
    );
}

#[test]
fn test_extract_param_type_multiple() {
    let code = "fn process(a: TypeA, b: TypeB) { }";
    let types = extract_param_types_from_func(code);
    assert!(types.contains(&"TypeA".to_string()), "got: {:?}", types);
    assert!(types.contains(&"TypeB".to_string()), "got: {:?}", types);
}

#[test]
fn test_extract_param_type_builtin_only() {
    let code = "fn process(x: i32, name: String) { }";
    let types = extract_param_types_from_func(code);
    assert!(
        types.is_empty(),
        "Should skip builtin param types, got: {:?}",
        types
    );
}

#[test]
fn test_extract_param_type_no_params() {
    let code = "fn do_work() { }";
    let types = extract_param_types_from_func(code);
    assert!(
        types.is_empty(),
        "Function with no params should return empty, got: {:?}",
        types
    );
}

#[test]
fn test_extract_param_type_skips_self() {
    // &self is not a user-defined type
    let code = "fn method(&self, config: Config) { }";
    let types = extract_param_types_from_func(code);
    assert_eq!(types, vec!["Config"], "Should skip self, got: {:?}", types);
}

// ============================================================================
// Type reference edge kind tests (FieldType vs TypeAnnotation cleanup)
// ============================================================================

#[test]
fn test_function_type_refs_excluded_from_extract_type_references() {
    // extract_type_references on function_item should return empty
    // because extract_return_types and extract_param_types handle those separately
    let code = "fn process(config: Config) -> Status { todo!() }";
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_item").unwrap();
    let refs = Rust::extract_type_references(fn_node, code);
    assert!(
        refs.is_empty(),
        "Function type refs should be empty (handled by extract_return_types/extract_param_types), got: {:?}",
        refs
    );
}

#[test]
fn test_struct_field_types_produce_field_type_edges() {
    let code = r#"
struct Server {
    config: Config,
    logger: Logger,
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Rust::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let struct_node = parse_and_find(&tree, "struct_item").unwrap();
    let refs = Rust::extract_type_references(struct_node, code);
    for (name, ref_kind) in &refs {
        assert_eq!(
            *ref_kind,
            ReferenceType::FieldType,
            "Struct field type '{}' should produce FieldType edge, got {:?}",
            name.as_str(),
            ref_kind
        );
    }
    let names: Vec<_> = refs.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"Config"),
        "Should contain Config, got: {:?}",
        names
    );
    assert!(
        names.contains(&"Logger"),
        "Should contain Logger, got: {:?}",
        names
    );
}
