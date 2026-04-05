use super::analyser::Rust;
use crate::analysis::types::*;

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/analysis/lang/rust/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

/// Extract the name component from a SymbolId string.
/// Format: "symbol:file_path:name:line"
fn extract_name(sid: &str) -> Option<&str> {
    let s = sid.strip_prefix("symbol:")?;
    let last_colon = s.rfind(':')?;
    let before_last = &s[..last_colon];
    let second_last_colon = before_last.rfind(':')?;
    Some(&before_last[second_last_colon + 1..])
}

/// Check if an edge exists with the given kind and from/to names.
fn has_edge(parsed: &ParsedFile, kind: EdgeKind, from_name: &str, to_name: &str) -> bool {
    parsed.edges.iter().any(|e| {
        e.kind == kind
            && extract_name(e.from.as_str()) == Some(from_name)
            && extract_name(e.to.as_str()) == Some(to_name)
    })
}

/// Get all edges of a given kind as (from_name, to_name) pairs.
fn edges_of_kind<'a>(parsed: &'a ParsedFile, kind: EdgeKind) -> Vec<(&'a str, &'a str)> {
    parsed
        .edges
        .iter()
        .filter(|e| e.kind == kind)
        .filter_map(|e| {
            let from = extract_name(e.from.as_str())?;
            let to = extract_name(e.to.as_str())?;
            Some((from, to))
        })
        .collect()
}

/// Get all edges from a specific symbol.
fn edges_from<'a>(parsed: &'a ParsedFile, kind: EdgeKind, from_name: &str) -> Vec<&'a str> {
    parsed
        .edges
        .iter()
        .filter(|e| e.kind == kind && extract_name(e.from.as_str()) == Some(from_name))
        .filter_map(|e| extract_name(e.to.as_str()))
        .collect()
}

#[test]
fn test_query_compiles() {
    let language = Rust::grammar();
    assert!(tree_sitter::Query::new(&language, Rust::queries()).is_ok());
}

#[test]
fn test_server_symbols() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let sym = |name: &str| parsed.symbols.iter().find(|s| s.name == name);

    assert!(sym("Server").is_some_and(|s| s.kind == "struct"));
    assert!(sym("Status").is_some_and(|s| s.kind == "enum"));
    assert!(sym("Handler").is_some_and(|s| s.kind == "trait"));
    assert!(sym("Middleware").is_some_and(|s| s.kind == "trait"));
    assert!(sym("Request").is_some_and(|s| s.kind == "struct"));
    assert!(sym("Response").is_some_and(|s| s.kind == "struct"));
    assert!(sym("MAX_CONNECTIONS").is_some_and(|s| s.kind == "const"));
    assert!(sym("INSTANCE_COUNT").is_some_and(|s| s.kind == "static"));
    assert!(sym("internal").is_some_and(|s| s.kind == "module"));
    assert!(
        sym("InternalConfig").is_some_and(|s| s.kind == "struct"),
        "should extract InternalConfig struct from module, got: {:?}",
        parsed
            .symbols
            .iter()
            .map(|s| (&s.name, &s.kind))
            .collect::<Vec<_>>()
    );
    assert!(
        sym("INTERNAL_VERSION").is_some_and(|s| s.kind == "const"),
        "should extract INTERNAL_VERSION const from module"
    );
}

#[test]
fn test_no_duplicate_symbols() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let mut seen = std::collections::HashSet::new();
    for sym in &parsed.symbols {
        let key = format!("{}:{}:{}", sym.file_path, sym.name, sym.start_line);
        assert!(seen.insert(key.clone()), "duplicate symbol: {}", key);
    }
}

#[test]
fn test_server_methods_and_containment() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Get method names from HasMethod edges where parent contains "Server"
    let server_methods: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasMethod && e.from.as_str().contains("Server"))
        .filter_map(|e| e.to.as_str().split(':').nth(2))
        .collect();

    assert!(
        server_methods.contains(&"new"),
        "missing 'new', got: {:?}",
        server_methods
    );
    assert!(
        server_methods.contains(&"register"),
        "missing 'register', got: {:?}",
        server_methods
    );
    assert!(
        server_methods.contains(&"start"),
        "missing 'start', got: {:?}",
        server_methods
    );
    assert!(
        server_methods.contains(&"listen"),
        "missing 'listen', got: {:?}",
        server_methods
    );
    assert!(
        server_methods.contains(&"route"),
        "missing 'route', got: {:?}",
        server_methods
    );
    assert!(
        server_methods.contains(&"before"),
        "missing 'before', got: {:?}",
        server_methods
    );
}

#[test]
fn test_impl_node_captured_for_method() {
    use tree_sitter::StreamingIterator;

    // Verify that the impl_item node is captured alongside method definitions
    // so we can compute the impl's SymbolId for containment
    let code = "struct Server {}\n\nimpl Server {\n    fn start(&self) {}\n}\n";

    let language = Rust::grammar();
    let query = tree_sitter::Query::new(&language, Rust::queries()).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

    // Find the match that has method_def
    let mut found_impl_line = None;
    while let Some(m) = matches.next() {
        let has_method_def = m
            .captures
            .iter()
            .any(|c| query.capture_names()[c.index as usize] == "method_def");

        if has_method_def {
            // Check that method_impl is also captured
            if let Some(impl_capture) = m
                .captures
                .iter()
                .find(|c| query.capture_names()[c.index as usize] == "method_impl")
            {
                found_impl_line = Some(impl_capture.node.start_position().row + 1);
                break;
            }
        }
    }

    assert!(
        found_impl_line.is_some(),
        "impl_item should be captured as @method_impl alongside @method_def"
    );
    assert_eq!(
        found_impl_line.unwrap(),
        3,
        "impl block should start on line 3"
    );
}

#[test]
fn test_server_heritage() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    assert!(
        has_edge(&parsed, EdgeKind::Implements, "Server", "Middleware"),
        "Server should implement Middleware"
    );
}

#[test]
fn test_server_calls() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Get all call targets
    let calls: Vec<&str> = edges_of_kind(&parsed, EdgeKind::Calls)
        .into_iter()
        .map(|(_, to)| to)
        .collect();

    // Should capture various call types
    assert!(
        calls.contains(&"format"),
        "should capture format!() macro calls: {:?}",
        calls
    );
    assert!(
        calls.contains(&"println"),
        "should capture println!() macro calls: {:?}",
        calls
    );
    assert!(
        calls.contains(&"new"),
        "should capture HashMap::new() scoped call: {:?}",
        calls
    );
    assert!(
        calls.contains(&"insert"),
        "should capture .insert() method call: {:?}",
        calls
    );
    assert!(
        calls.contains(&"get"),
        "should capture .get() method call: {:?}",
        calls
    );
    assert!(
        calls.contains(&"handle"),
        "should capture .handle() method call: {:?}",
        calls
    );
    assert!(
        calls.contains(&"listen"),
        "should capture .listen() method call: {:?}",
        calls
    );
}

#[test]
fn test_server_imports() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    assert!(parsed.imports.iter().any(|i| {
        i.entry.module_path == "std::collections"
            && i.entry.imported_names.contains(&"HashMap".to_string())
    }));
    assert!(parsed.imports.iter().any(|i| {
        i.entry.module_path == "std::sync" && i.entry.imported_names.contains(&"Arc".to_string())
    }));
}

#[test]
fn test_macro_definition_symbol() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let sym = |name: &str| parsed.symbols.iter().find(|s| s.name == name);
    assert!(
        sym("log").is_some_and(|s| s.kind == "macro"),
        "should extract macro_rules! log as macro symbol, got: {:?}",
        parsed
            .symbols
            .iter()
            .map(|s| (&s.name, &s.kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_generic_inherent_impl_containment() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let container_methods: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasMethod && e.from.as_str().contains("Container"))
        .filter_map(|e| e.to.as_str().split(':').nth(2))
        .collect();

    assert!(
        container_methods.contains(&"new"),
        "should contain Container<T>.new(), got: {:?}",
        container_methods
    );
    assert!(
        container_methods.contains(&"add"),
        "should contain Container<T>.add(), got: {:?}",
        container_methods
    );
    assert!(
        container_methods.contains(&"labels"),
        "should contain Container<T>.labels(), got: {:?}",
        container_methods
    );
}

#[test]
fn test_generic_heritage_variants() {
    use crate::analysis::types::EdgeKind;

    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Extract name from SymbolId format "symbol:file_path:name:line"
    fn extract_name(sid: &str) -> Option<&str> {
        let s = sid.strip_prefix("symbol:")?;
        let last_colon = s.rfind(':')?;
        let before_last = &s[..last_colon];
        let second_last_colon = before_last.rfind(':')?;
        Some(&before_last[second_last_colon + 1..])
    }

    let implements_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Implements)
        .map(|e| {
            (
                extract_name(e.from.as_str()).unwrap_or(""),
                extract_name(e.to.as_str()).unwrap_or(""),
            )
        })
        .collect();

    // Concrete trait, concrete type: impl Middleware for Server (already tested)
    assert!(
        implements_edges
            .iter()
            .any(|(from, to)| *from == "Server" && *to == "Middleware"),
        "concrete×concrete: impl Middleware for Server"
    );

    // Generic trait, concrete type: impl Serializer<String> for Server
    assert!(
        implements_edges
            .iter()
            .any(|(from, to)| *from == "Server" && *to == "Serializer"),
        "generic_trait×concrete: impl Serializer<String> for Server, got: {:?}",
        implements_edges
    );

    // Concrete trait, generic type: impl<T> Handler for Container<T>
    assert!(
        implements_edges.iter().any(
            |(from, to)| (*from == "Container" || *from == "Container<T>") && *to == "Handler"
        ),
        "concrete×generic: impl<T> Handler for Container<T>, got: {:?}",
        implements_edges
    );

    // Generic trait, generic type: impl<T> Serializer<Vec<T>> for Container<T>
    assert!(
        implements_edges.iter().any(
            |(from, to)| (*from == "Container" || *from == "Container<T>") && *to == "Serializer"
        ),
        "generic×generic: impl<T> Serializer<Vec<T>> for Container<T>, got: {:?}",
        implements_edges
    );
}

#[test]
fn test_generic_function_call() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // collect::<Vec<_>>() should be captured (it's a generic method call)
    let calls: Vec<&str> = edges_of_kind(&parsed, EdgeKind::Calls)
        .into_iter()
        .map(|(_, to)| to)
        .collect();
    assert!(
        calls.contains(&"collect"),
        "should capture collect::<Vec<_>>() generic function call, got calls: {:?}",
        calls
    );
}

#[test]
fn test_struct_expression_call() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Response { status: 200, body: "OK".to_string() } should be captured as constructor
    let calls: Vec<&str> = edges_of_kind(&parsed, EdgeKind::Calls)
        .into_iter()
        .map(|(_, to)| to)
        .collect();
    assert!(
        calls.contains(&"Response"),
        "should capture Response {{ ... }} struct expression as constructor call, got: {:?}",
        calls
    );
}

#[test]
fn test_struct_field_extraction() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let fields: Vec<&str> = parsed
        .symbols
        .iter()
        .filter(|s| s.kind == "field")
        .map(|s| s.name.as_str())
        .collect();

    assert!(
        fields.contains(&"host"),
        "should extract Server.host field, got: {:?}",
        fields
    );
    assert!(
        fields.contains(&"port"),
        "should extract Server.port field, got: {:?}",
        fields
    );
    assert!(
        fields.contains(&"handlers"),
        "should extract Server.handlers field, got: {:?}",
        fields
    );
    assert!(
        fields.contains(&"path"),
        "should extract Request.path field, got: {:?}",
        fields
    );
    assert!(
        fields.contains(&"items"),
        "should extract Container.items field, got: {:?}",
        fields
    );
}

#[test]
fn test_rust_type_alias() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let sym = |name: &str| parsed.symbols.iter().find(|s| s.name == name);
    assert!(
        sym("HandlerMap").is_some_and(|s| s.kind == "type"),
        "should extract type alias HandlerMap"
    );
}

#[test]
fn test_rust_visibility() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let vis = |name: &str| -> Option<String> {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.visibility.clone())
    };

    // Public items
    assert_eq!(
        vis("MAX_CONNECTIONS"),
        Some("public".to_string()),
        "pub const should be public"
    );
    assert_eq!(
        vis("Server"),
        Some("public".to_string()),
        "pub struct should be public"
    );
    assert_eq!(
        vis("Status"),
        Some("public".to_string()),
        "pub enum should be public"
    );
    assert_eq!(
        vis("Handler"),
        Some("public".to_string()),
        "pub trait should be public"
    );
    assert_eq!(
        vis("HandlerMap"),
        Some("public".to_string()),
        "pub type alias should be public"
    );

    // Private items
    assert_eq!(
        vis("INSTANCE_COUNT"),
        Some("private".to_string()),
        "non-pub static should be private"
    );
    assert_eq!(
        vis("internal"),
        Some("private".to_string()),
        "non-pub mod should be private"
    );
}

#[test]
fn test_entry_type_test_attribute() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.entry_type.clone())
    };

    assert_eq!(
        entry("test_server_creation"),
        Some("test".to_string()),
        "#[test] function should have entry_type 'test'"
    );
}

#[test]
fn test_entry_type_tokio_main() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.entry_type.clone())
    };

    assert_eq!(
        entry("main"),
        Some("main".to_string()),
        "#[tokio::main] function should have entry_type 'main'"
    );
}

#[test]
fn test_entry_type_no_mangle() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.entry_type.clone())
    };

    assert_eq!(
        entry("exported_function"),
        Some("export".to_string()),
        "#[no_mangle] function should have entry_type 'export'"
    );
}

#[test]
fn test_entry_type_regular_function_is_none() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.entry_type.clone())
    };

    assert_eq!(
        entry("create_default_config"),
        None,
        "regular function should not have entry_type"
    );
}

#[test]
fn test_cfg_test_module_tags_all_symbols() {
    // All symbols inside #[cfg(test)] mod tests { } should have entry_type = "test"
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .map(|s| (s.name.clone(), s.entry_type.clone()))
    };

    // The tests module itself should be tagged
    assert_eq!(
        entry("tests").map(|(_, e)| e),
        Some(Some("test".to_string())),
        "#[cfg(test)] mod tests should have entry_type 'test'"
    );

    // Helper function inside #[cfg(test)] mod should be tagged
    assert_eq!(
        entry("setup_test_fixture").map(|(_, e)| e),
        Some(Some("test".to_string())),
        "helper function inside #[cfg(test)] mod should have entry_type 'test'"
    );

    // Struct inside #[cfg(test)] mod should be tagged
    assert_eq!(
        entry("TestHelper").map(|(_, e)| e),
        Some(Some("test".to_string())),
        "struct inside #[cfg(test)] mod should have entry_type 'test'"
    );

    // Constant inside #[cfg(test)] mod should be tagged
    assert_eq!(
        entry("TEST_CONSTANT").map(|(_, e)| e),
        Some(Some("test".to_string())),
        "constant inside #[cfg(test)] mod should have entry_type 'test'"
    );

    // Test function inside #[cfg(test)] mod should be tagged
    assert_eq!(
        entry("test_inside_cfg_test_module").map(|(_, e)| e),
        Some(Some("test".to_string())),
        "#[test] function inside #[cfg(test)] mod should have entry_type 'test'"
    );

    // Another test module
    assert_eq!(
        entry("integration_tests").map(|(_, e)| e),
        Some(Some("test".to_string())),
        "#[cfg(test)] mod integration_tests should have entry_type 'test'"
    );
}

#[test]
fn test_non_test_module_not_tagged() {
    // Regular modules without #[cfg(test)] should NOT have entry_type
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.entry_type.clone())
    };

    // Regular internal module should NOT have entry_type
    assert_eq!(
        entry("internal"),
        None,
        "regular mod internal should NOT have entry_type"
    );

    // Symbols inside regular module should NOT have entry_type
    assert_eq!(
        entry("INTERNAL_VERSION"),
        None,
        "constant in regular mod should NOT have entry_type"
    );
}

#[test]
fn test_struct_field_containment() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let children_of = |parent: &str| -> Vec<&str> {
        parsed
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::HasField && e.from.as_str().contains(parent))
            .filter_map(|e| e.to.as_str().split(':').nth(2))
            .collect()
    };

    let server_children = children_of("Server");
    assert!(
        server_children.contains(&"host"),
        "Server should contain field 'host', got: {:?}",
        server_children
    );
    assert!(
        server_children.contains(&"port"),
        "Server should contain field 'port', got: {:?}",
        server_children
    );
    assert!(
        server_children.contains(&"handlers"),
        "Server should contain field 'handlers', got: {:?}",
        server_children
    );

    let request_children = children_of("Request");
    assert!(
        request_children.contains(&"path"),
        "Request should contain field 'path', got: {:?}",
        request_children
    );
    assert!(
        request_children.contains(&"method"),
        "Request should contain field 'method', got: {:?}",
        request_children
    );

    let response_children = children_of("Response");
    assert!(
        response_children.contains(&"status"),
        "Response should contain field 'status', got: {:?}",
        response_children
    );
    assert!(
        response_children.contains(&"body"),
        "Response should contain field 'body', got: {:?}",
        response_children
    );

    let container_children = children_of("Container");
    assert!(
        container_children.contains(&"items"),
        "Container should contain field 'items', got: {:?}",
        container_children
    );
    assert!(
        container_children.contains(&"label"),
        "Container should contain field 'label', got: {:?}",
        container_children
    );
}

#[test]
fn test_trait_method_containment() {
    use crate::analysis::types::EdgeKind;

    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Extract name from SymbolId format "symbol:file_path:name:line"
    fn extract_name(sid: &str) -> Option<&str> {
        let s = sid.strip_prefix("symbol:")?;
        let last_colon = s.rfind(':')?;
        let before_last = &s[..last_colon];
        let second_last_colon = before_last.rfind(':')?;
        Some(&before_last[second_last_colon + 1..])
    }

    let children_of = |parent: &str| -> Vec<String> {
        parsed
            .edges
            .iter()
            .filter(|e| matches!(e.kind, EdgeKind::HasMethod | EdgeKind::HasMember))
            .filter(|e| extract_name(e.from.as_str()) == Some(parent))
            .filter_map(|e| extract_name(e.to.as_str()).map(|s| s.to_string()))
            .collect()
    };

    let handler_children = children_of("Handler");
    assert!(
        handler_children.iter().any(|c| c == "handle"),
        "Handler trait should contain method 'handle', got: {:?}",
        handler_children
    );

    let middleware_children = children_of("Middleware");
    assert!(
        middleware_children.iter().any(|c| c == "before"),
        "Middleware trait should contain method 'before', got: {:?}",
        middleware_children
    );
}

#[test]
fn test_module_children_containment() {
    use crate::analysis::types::EdgeKind;

    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Extract name from SymbolId format "symbol:file_path:name:line"
    fn extract_name(sid: &str) -> Option<&str> {
        let s = sid.strip_prefix("symbol:")?;
        let last_colon = s.rfind(':')?;
        let before_last = &s[..last_colon];
        let second_last_colon = before_last.rfind(':')?;
        Some(&before_last[second_last_colon + 1..])
    }

    let children_of = |parent: &str| -> Vec<(String, String)> {
        parsed
            .edges
            .iter()
            .filter(|e| matches!(e.kind, EdgeKind::HasMember))
            .filter(|e| extract_name(e.from.as_str()) == Some(parent))
            .filter_map(|e| {
                let child_name = extract_name(e.to.as_str())?;
                // Find symbol to get its kind
                let sym = parsed.symbols.iter().find(|s| s.name == child_name)?;
                Some((child_name.to_string(), sym.kind.clone()))
            })
            .collect()
    };

    let internal_children = children_of("internal");
    assert!(
        internal_children
            .iter()
            .any(|(name, kind)| name == "helper" && kind == "function"),
        "module 'internal' should contain function 'helper', got: {:?}",
        internal_children
    );
    assert!(
        internal_children
            .iter()
            .any(|(name, kind)| name == "INTERNAL_VERSION" && kind == "const"),
        "module 'internal' should contain const 'INTERNAL_VERSION', got: {:?}",
        internal_children
    );
    assert!(
        internal_children
            .iter()
            .any(|(name, kind)| name == "InternalConfig" && kind == "struct"),
        "module 'internal' should contain struct 'InternalConfig', got: {:?}",
        internal_children
    );
}

// =============================================================================
// Cross-file module containment tests
// =============================================================================

/// Extract containment edges (HasField, HasMethod, HasMember) as (parent_name, child_name) pairs.
fn resolve_containments(parsed_files: &[ParsedFile]) -> Vec<(String, String)> {
    use crate::analysis::types::EdgeKind;

    // Extract name from SymbolId format "symbol:file_path:name:line"
    fn extract_name(sid: &str) -> Option<&str> {
        let s = sid.strip_prefix("symbol:")?;
        let last_colon = s.rfind(':')?;
        let before_last = &s[..last_colon];
        let second_last_colon = before_last.rfind(':')?;
        Some(&before_last[second_last_colon + 1..])
    }

    let mut edges = Vec::new();
    for pf in parsed_files {
        for edge in &pf.edges {
            if matches!(
                edge.kind,
                EdgeKind::HasField | EdgeKind::HasMethod | EdgeKind::HasMember
            ) {
                if let (Some(from_name), Some(to_name)) = (
                    extract_name(edge.from.as_str()),
                    extract_name(edge.to.as_str()),
                ) {
                    edges.push((from_name.to_string(), to_name.to_string()));
                }
            }
        }
    }
    edges
}

#[test]
fn test_file_module_containment_flat_file() {
    // lib.rs declares `mod handlers;` -> handlers.rs has functions
    // After resolve_file_modules, handlers module should contain process and validate
    let lib_code = load_testdata("multifile/lib.rs");
    let handlers_code = load_testdata("multifile/handlers.rs");

    let mut parsed_files = vec![
        Rust::extract(&lib_code, "src/lib.rs"),
        Rust::extract(&handlers_code, "src/handlers.rs"),
    ];
    Rust::resolve_file_modules(&mut parsed_files);

    let edges = resolve_containments(&parsed_files);

    assert!(
        edges.contains(&("handlers".to_string(), "process".to_string())),
        "handlers should contain 'process', got: {:?}",
        edges
    );
    assert!(
        edges.contains(&("handlers".to_string(), "validate".to_string())),
        "handlers should contain 'validate', got: {:?}",
        edges
    );
}

#[test]
fn test_file_module_containment_mod_rs() {
    // lib.rs declares `mod config;` -> config/mod.rs has functions
    // After resolve_file_modules, config module should contain load
    let lib_code = load_testdata("multifile/lib.rs");
    let config_code = load_testdata("multifile/config/mod.rs");

    let mut parsed_files = vec![
        Rust::extract(&lib_code, "src/lib.rs"),
        Rust::extract(&config_code, "src/config/mod.rs"),
    ];
    Rust::resolve_file_modules(&mut parsed_files);

    let edges = resolve_containments(&parsed_files);

    assert!(
        edges.contains(&("config".to_string(), "load".to_string())),
        "config should contain 'load', got: {:?}",
        edges
    );
}

#[test]
fn test_file_module_containment_nested() {
    // config/mod.rs declares `mod defaults;` -> config/defaults.rs has items
    // After resolve_file_modules, defaults module should contain DEFAULT_PORT and default_host
    let config_code = load_testdata("multifile/config/mod.rs");
    let defaults_code = load_testdata("multifile/config/defaults.rs");

    let mut parsed_files = vec![
        Rust::extract(&config_code, "src/config/mod.rs"),
        Rust::extract(&defaults_code, "src/config/defaults.rs"),
    ];
    Rust::resolve_file_modules(&mut parsed_files);

    let edges = resolve_containments(&parsed_files);

    assert!(
        edges.contains(&("defaults".to_string(), "DEFAULT_PORT".to_string())),
        "defaults should contain 'DEFAULT_PORT', got: {:?}",
        edges
    );
    assert!(
        edges.contains(&("defaults".to_string(), "default_host".to_string())),
        "defaults should contain 'default_host', got: {:?}",
        edges
    );
}

#[test]
fn test_file_module_containment_full_tree() {
    // All 4 files together: lib.rs -> handlers.rs, config/mod.rs -> config/defaults.rs
    let lib_code = load_testdata("multifile/lib.rs");
    let handlers_code = load_testdata("multifile/handlers.rs");
    let config_code = load_testdata("multifile/config/mod.rs");
    let defaults_code = load_testdata("multifile/config/defaults.rs");

    let mut parsed_files = vec![
        Rust::extract(&lib_code, "src/lib.rs"),
        Rust::extract(&handlers_code, "src/handlers.rs"),
        Rust::extract(&config_code, "src/config/mod.rs"),
        Rust::extract(&defaults_code, "src/config/defaults.rs"),
    ];
    Rust::resolve_file_modules(&mut parsed_files);

    let edges = resolve_containments(&parsed_files);

    // handlers module contains its functions
    assert!(edges.contains(&("handlers".to_string(), "process".to_string())));
    assert!(edges.contains(&("handlers".to_string(), "validate".to_string())));

    // config module contains load and the defaults submodule declaration
    assert!(edges.contains(&("config".to_string(), "load".to_string())));
    assert!(edges.contains(&("config".to_string(), "defaults".to_string())));

    // defaults module contains its items
    assert!(edges.contains(&("defaults".to_string(), "DEFAULT_PORT".to_string())));
    assert!(edges.contains(&("defaults".to_string(), "default_host".to_string())));

    // run in lib.rs should NOT be contained by any file module
    // (it's a top-level function in the crate root)
    assert!(
        !edges.iter().any(|(_, child)| child == "run"),
        "run should not be contained by any file module (it's in lib.rs crate root)"
    );
}

// =============================================================================
// Type reference extraction tests
// =============================================================================

#[test]
fn test_typeref_debug_all() {
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    // Expect:
    // update_server -> Server (ParamType, via &mut)
    // accept_direct -> Server (ParamType, direct)
    // get_server -> Server (ReturnType)
    // route -> Request (ParamType, via &)
    // route -> Response (ReturnType)
    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "accept_direct", "Server"),
        "accept_direct should accept Server, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "accept_direct")
    );
    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "update_server", "Server"),
        "update_server should accept Server, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "update_server")
    );
    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "get_server", "Server"),
        "get_server should return Server, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "get_server")
    );
    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "route", "Request"),
        "route should accept Request, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "route")
    );
    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "route", "Response"),
        "route should return Response, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "route")
    );
}

#[test]
fn test_param_type_refs() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // fn handle(&self, request: &Request) -> Response
    // should produce ParamType edge from handle to Request
    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "handle", "Request"),
        "handle should have ParamType ref to Request, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "handle")
    );
}

#[test]
fn test_return_type_refs() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // fn handle(&self, request: &Request) -> Response
    // should produce ReturnType edge from handle to Response
    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "handle", "Response"),
        "handle should have ReturnType ref to Response, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "handle")
    );
}

#[test]
fn test_field_type_refs() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // struct Server { handlers: HashMap<String, Box<dyn Handler>> }
    // The field "handlers" should have a FieldType ref to Handler
    assert!(
        has_edge(&parsed, EdgeKind::FieldType, "handlers", "Handler"),
        "handlers field should have FieldType ref to Handler, got: {:?}",
        edges_from(&parsed, EdgeKind::FieldType, "handlers")
    );
}

#[test]
fn test_create_default_config_returns_response() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // pub fn create_default_config() -> Response
    assert!(
        has_edge(
            &parsed,
            EdgeKind::ReturnType,
            "create_default_config",
            "Response"
        ),
        "create_default_config should return Response, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "create_default_config")
    );
}

#[test]
fn test_update_server_accepts_server() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // pub fn update_server(server: &mut Server)
    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "update_server", "Server"),
        "update_server should accept Server, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "update_server")
    );
}

#[test]
fn test_scoped_call_qualifier_type_ref() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Server::new() at line 172 inside test_server_creation should produce TypeRef edge to Server
    assert!(
        has_edge(&parsed, EdgeKind::TypeRef, "test_server_creation", "Server"),
        "test_server_creation should have TypeRef to Server from Server::new(), got: {:?}",
        edges_from(&parsed, EdgeKind::TypeRef, "test_server_creation")
    );
}

#[test]
fn test_scoped_call_qualifier_filters_self() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let has_self_ref = parsed
        .edges
        .iter()
        .any(|e| e.kind == EdgeKind::TypeRef && extract_name(e.to.as_str()) == Some("Self"));
    assert!(
        !has_self_ref,
        "Self::method() should not produce a TypeRef edge"
    );
}

// =============================================================================
// Generic inner type argument extraction tests
// =============================================================================

#[test]
fn test_generic_return_type_inner_arg() {
    // fn health() -> Json<HealthResponse> should extract HealthResponse
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "health", "HealthResponse"),
        "health() -> Json<HealthResponse> should extract HealthResponse as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "health")
    );
}

#[test]
fn test_result_return_type_inner_arg() {
    // fn load_config() -> Result<Config, Error> should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "load_config", "Config"),
        "load_config() -> Result<Config, Error> should extract Config as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "load_config")
    );
}

#[test]
fn test_option_return_type_inner_arg() {
    // fn find_config() -> Option<Config> should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "find_config", "Config"),
        "find_config() -> Option<Config> should extract Config as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "find_config")
    );
}

#[test]
fn test_generic_param_type_inner_arg() {
    // fn process_items(items: Vec<Config>) should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "process_items", "Config"),
        "process_items(items: Vec<Config>) should extract Config as ParamType, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "process_items")
    );
}

#[test]
fn test_ref_generic_param_type_inner_arg() {
    // fn process_items_ref(items: &Vec<Config>) should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "process_items_ref", "Config"),
        "process_items_ref(items: &Vec<Config>) should extract Config as ParamType, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "process_items_ref")
    );
}

#[test]
fn test_generic_field_type_inner_arg() {
    // struct AppState { db: Arc<Database> } should extract Database
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::FieldType, "db", "Database"),
        "AppState.db: Arc<Database> should extract Database as FieldType, got: {:?}",
        edges_from(&parsed, EdgeKind::FieldType, "db")
    );
}

#[test]
fn test_nested_generic_return_type_inner_arg() {
    // fn get_shared_db() -> Arc<Mutex<Database>> should extract Database
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "get_shared_db", "Database"),
        "get_shared_db() -> Arc<Mutex<Database>> should extract Database as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "get_shared_db")
    );
}

#[test]
fn test_impl_method_generic_return_type_inner_arg() {
    // impl AppState { fn get_config(&self) -> Option<Config> } should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "get_config", "Config"),
        "AppState::get_config() -> Option<Config> should extract Config as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "get_config")
    );
}

// =============================================================================
// Abstract type (impl Trait) tests
// =============================================================================

#[test]
fn test_abstract_type_return() {
    // fn get_handler() -> impl Handler should extract Handler
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "get_handler", "Handler"),
        "get_handler() -> impl Handler should extract Handler as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "get_handler")
    );
}

#[test]
fn test_impl_method_abstract_type_return() {
    // impl AppState { fn get_service(&self) -> impl Service } should extract Service
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "get_service", "Service"),
        "AppState::get_service() -> impl Service should extract Service as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "get_service")
    );
}

// =============================================================================
// Array and slice type tests
// =============================================================================

#[test]
fn test_slice_param_type() {
    // fn process_slice(items: &[Config]) should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "process_slice", "Config"),
        "process_slice(items: &[Config]) should extract Config as ParamType, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "process_slice")
    );
}

#[test]
fn test_array_param_type() {
    // fn process_array(items: [Item; 5]) should extract Item
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "process_array", "Item"),
        "process_array(items: [Item; 5]) should extract Item as ParamType, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "process_array")
    );
}

#[test]
fn test_impl_method_slice_param_type() {
    // impl AppState { fn update_configs(&self, configs: &[Config]) } should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ParamType, "update_configs", "Config"),
        "AppState::update_configs(configs: &[Config]) should extract Config as ParamType, got: {:?}",
        edges_from(&parsed, EdgeKind::ParamType, "update_configs")
    );
}

// =============================================================================
// Dynamic type (dyn Trait) tests
// =============================================================================

#[test]
fn test_dyn_trait_return_type() {
    // fn get_boxed_handler() -> Box<dyn Handler> should extract Handler
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(
            &parsed,
            EdgeKind::ReturnType,
            "get_boxed_handler",
            "Handler"
        ),
        "get_boxed_handler() -> Box<dyn Handler> should extract Handler as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "get_boxed_handler")
    );
}

#[test]
fn test_dyn_trait_arc_return_type() {
    // fn get_arc_service() -> Arc<dyn Service> should extract Service
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "get_arc_service", "Service"),
        "get_arc_service() -> Arc<dyn Service> should extract Service as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "get_arc_service")
    );
}

#[test]
fn test_impl_method_dyn_trait_return_type() {
    // impl AppState { fn try_operation(&self) -> Result<(), Box<dyn DynError>> } should extract DynError
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "try_operation", "DynError"),
        "AppState::try_operation() -> Result<(), Box<dyn DynError>> should extract DynError as ReturnType, got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "try_operation")
    );
}

// =============================================================================
// Built-in type filtering tests
// =============================================================================

#[test]
fn test_builtin_types_filtered_from_return() {
    // fn load_config() -> Result<Config, Error> should NOT have Result in refs
    // fn find_config() -> Option<Config> should NOT have Option in refs
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let builtin_return_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == EdgeKind::ReturnType
                && matches!(
                    extract_name(e.to.as_str()),
                    Some("Result" | "Option" | "Box" | "Vec" | "String" | "Arc")
                )
        })
        .collect();

    assert!(
        builtin_return_edges.is_empty(),
        "Built-in types should be filtered from return type refs, found: {:?}",
        builtin_return_edges
            .iter()
            .filter_map(|e| extract_name(e.to.as_str()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_builtin_types_filtered_from_params() {
    // fn get_server(name: String) should NOT have String in refs
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let builtin_param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == EdgeKind::ParamType
                && matches!(
                    extract_name(e.to.as_str()),
                    Some("String" | "Vec" | "HashMap" | "Option" | "Result")
                )
        })
        .collect();

    assert!(
        builtin_param_edges.is_empty(),
        "Built-in types should be filtered from param type refs, found: {:?}",
        builtin_param_edges
            .iter()
            .filter_map(|e| extract_name(e.to.as_str()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_builtin_types_filtered_from_fields() {
    // struct Config { name: String } should NOT have String in refs
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let builtin_field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == EdgeKind::FieldType
                && matches!(
                    extract_name(e.to.as_str()),
                    Some("String" | "u16" | "bool" | "i32")
                )
        })
        .collect();

    assert!(
        builtin_field_edges.is_empty(),
        "Built-in types should be filtered from field type refs, found: {:?}",
        builtin_field_edges
            .iter()
            .filter_map(|e| extract_name(e.to.as_str()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_user_types_preserved_when_builtins_filtered() {
    // fn load_config() -> Result<Config, Error> should still have Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    assert!(
        has_edge(&parsed, EdgeKind::ReturnType, "load_config", "Config"),
        "load_config() -> Result<Config, Error> should extract Config (user type), got: {:?}",
        edges_from(&parsed, EdgeKind::ReturnType, "load_config")
    );
}

// ============================================================================
// Edge-based semantic relationship tests (new architecture)
// ============================================================================

#[test]
fn test_impl_methods_emit_has_method_edges() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // impl Server has methods: new, register, start, listen, route
    let has_method_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasMethod)
        .collect();

    // Server impl (5 methods) + Container<T> impl (3 methods) + trait impls
    assert!(
        has_method_edges.len() >= 5,
        "impl Server should emit at least 5 HasMethod edges, got: {}",
        has_method_edges.len()
    );

    // Verify Server's methods are in there
    let server_methods: Vec<&str> = has_method_edges
        .iter()
        .filter(|e| e.from.as_str().contains("Server"))
        .filter_map(|e| e.to.as_str().split(':').nth(2))
        .collect();

    assert!(server_methods.contains(&"new"), "should have edge to 'new'");
    assert!(
        server_methods.contains(&"start"),
        "should have edge to 'start'"
    );
    assert!(
        server_methods.contains(&"register"),
        "should have edge to 'register'"
    );
}

#[test]
fn test_struct_fields_emit_has_field_edges() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // struct Server { host, port, handlers }
    let server_field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasField && e.from.as_str().contains("Server"))
        .collect();

    assert_eq!(
        server_field_edges.len(),
        3,
        "struct Server should emit 3 HasField edges, got: {:?}",
        server_field_edges
    );

    let field_names: Vec<&str> = server_field_edges
        .iter()
        .filter_map(|e| e.to.as_str().split(':').nth(2))
        .collect();

    assert!(field_names.contains(&"host"), "should have edge to 'host'");
    assert!(field_names.contains(&"port"), "should have edge to 'port'");
    assert!(
        field_names.contains(&"handlers"),
        "should have edge to 'handlers'"
    );
}

#[test]
fn test_trait_impl_emits_implements_edge() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // impl Middleware for Server, impl Handler for Container<T>, etc.
    let implements_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Implements)
        .collect();

    assert!(
        !implements_edges.is_empty(),
        "should have Implements edges for trait impls"
    );

    // Find the Server -> Middleware edge
    let server_middleware = implements_edges
        .iter()
        .find(|e| e.from.as_str().contains("Server") && e.to.as_str().contains("Middleware"));

    assert!(
        server_middleware.is_some(),
        "should have Server implements Middleware edge, got: {:?}",
        implements_edges
    );
}
