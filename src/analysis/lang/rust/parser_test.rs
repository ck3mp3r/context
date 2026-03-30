use super::parser::Rust;
use crate::analysis::types::*;

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/analysis/lang/rust/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
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

    let server_methods: Vec<&str> = parsed
        .containments
        .iter()
        .filter(|c| c.parent_name == "Server")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
        .collect();

    assert!(server_methods.contains(&"new"));
    assert!(server_methods.contains(&"register"));
    assert!(server_methods.contains(&"start"));
    assert!(server_methods.contains(&"listen"));
    assert!(server_methods.contains(&"route"));
    assert!(server_methods.contains(&"before"));
}

#[test]
fn test_server_heritage() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    assert!(parsed.heritage.iter().any(|h| {
        h.type_name == "Server"
            && h.parent_name == "Middleware"
            && h.kind == InheritanceType::Implements
    }));
}

#[test]
fn test_server_calls() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Free calls
    let free: Vec<&str> = parsed
        .calls
        .iter()
        .filter(|c| c.call_form == CallForm::Free)
        .map(|c| c.callee_name.as_str())
        .collect();
    assert!(
        free.contains(&"format"),
        "should capture format!() macro calls: {:?}",
        free
    );
    assert!(
        free.contains(&"println"),
        "should capture println!() macro calls: {:?}",
        free
    );

    // Scoped calls
    let scoped: Vec<(&str, Option<&str>)> = parsed
        .calls
        .iter()
        .filter(|c| c.call_form == CallForm::Scoped)
        .map(|c| (c.callee_name.as_str(), c.qualifier.as_deref()))
        .collect();
    assert!(scoped.contains(&("new", Some("HashMap"))));

    // Method calls
    let methods: Vec<&str> = parsed
        .calls
        .iter()
        .filter(|c| c.call_form == CallForm::Method)
        .map(|c| c.callee_name.as_str())
        .collect();
    assert!(methods.contains(&"insert"));
    assert!(methods.contains(&"get"));
    assert!(methods.contains(&"handle"));
    assert!(methods.contains(&"listen"));
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
        .containments
        .iter()
        .filter(|c| c.parent_name == "Container" || c.parent_name == "Container<T>")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
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
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Concrete trait, concrete type: impl Middleware for Server (already tested)
    assert!(
        parsed.heritage.iter().any(|h| h.type_name == "Server"
            && h.parent_name == "Middleware"
            && h.kind == InheritanceType::Implements),
        "concrete×concrete: impl Middleware for Server"
    );

    // Generic trait, concrete type: impl Serializer<String> for Server
    assert!(
        parsed.heritage.iter().any(|h| h.type_name == "Server"
            && h.parent_name == "Serializer"
            && h.kind == InheritanceType::Implements),
        "generic_trait×concrete: impl Serializer<String> for Server, got: {:?}",
        parsed.heritage
    );

    // Concrete trait, generic type: impl<T> Handler for Container<T>
    assert!(
        parsed.heritage.iter().any(|h| h.parent_name == "Handler"
            && (h.type_name == "Container" || h.type_name == "Container<T>")
            && h.kind == InheritanceType::Implements),
        "concrete×generic: impl<T> Handler for Container<T>, got: {:?}",
        parsed.heritage
    );

    // Generic trait, generic type: impl<T> Serializer<Vec<T>> for Container<T>
    assert!(
        parsed.heritage.iter().any(|h| h.parent_name == "Serializer"
            && (h.type_name == "Container" || h.type_name == "Container<T>")
            && h.kind == InheritanceType::Implements),
        "generic×generic: impl<T> Serializer<Vec<T>> for Container<T>, got: {:?}",
        parsed.heritage
    );
}

#[test]
fn test_generic_function_call() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // collect::<Vec<_>>() should be captured (it's a generic method call)
    assert!(
        parsed.calls.iter().any(|c| c.callee_name == "collect"),
        "should capture collect::<Vec<_>>() generic function call, got calls: {:?}",
        parsed
            .calls
            .iter()
            .map(|c| &c.callee_name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_struct_expression_call() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Response { status: 200, body: "OK".to_string() } should be captured as constructor
    assert!(
        parsed
            .calls
            .iter()
            .any(|c| c.callee_name == "Response" && c.call_form == CallForm::Free),
        "should capture Response {{ ... }} struct expression as constructor call, got: {:?}",
        parsed
            .calls
            .iter()
            .map(|c| (&c.callee_name, &c.call_form))
            .collect::<Vec<_>>()
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
fn test_rust_write_access() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    assert!(
        parsed
            .write_accesses
            .iter()
            .any(|w| w.property == "port" && w.receiver == "server"),
        "should capture server.port = 9090 write access, got: {:?}",
        parsed.write_accesses
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
fn test_struct_field_containment() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let children_of = |parent: &str| -> Vec<&str> {
        parsed
            .containments
            .iter()
            .filter(|c| c.parent_name == parent)
            .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
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
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let children_of = |parent: &str| -> Vec<&str> {
        parsed
            .containments
            .iter()
            .filter(|c| c.parent_name == parent)
            .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
            .collect()
    };

    let handler_children = children_of("Handler");
    assert!(
        handler_children.contains(&"handle"),
        "Handler trait should contain method 'handle', got: {:?}",
        handler_children
    );

    let middleware_children = children_of("Middleware");
    assert!(
        middleware_children.contains(&"before"),
        "Middleware trait should contain method 'before', got: {:?}",
        middleware_children
    );
}

#[test]
fn test_module_children_containment() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let children_of = |parent: &str| -> Vec<(&str, &str)> {
        parsed
            .containments
            .iter()
            .filter(|c| c.parent_name == parent)
            .map(|c| {
                let sym = &parsed.symbols[c.child_symbol_idx];
                (sym.name.as_str(), sym.kind.as_str())
            })
            .collect()
    };

    let internal_children = children_of("internal");
    assert!(
        internal_children
            .iter()
            .any(|(name, kind)| *name == "helper" && *kind == "function"),
        "module 'internal' should contain function 'helper', got: {:?}",
        internal_children
    );
    assert!(
        internal_children
            .iter()
            .any(|(name, kind)| *name == "INTERNAL_VERSION" && *kind == "const"),
        "module 'internal' should contain const 'INTERNAL_VERSION', got: {:?}",
        internal_children
    );
    assert!(
        internal_children
            .iter()
            .any(|(name, kind)| *name == "InternalConfig" && *kind == "struct"),
        "module 'internal' should contain struct 'InternalConfig', got: {:?}",
        internal_children
    );
}
