use super::analyser::Rust;
use crate::analysis::lang::LanguageAnalyser;
use crate::analysis::pipeline::SymbolRegistry;
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

// =============================================================================
// Cross-file module containment tests
// =============================================================================

/// Simulate Phase 2+3: register symbols, then resolve containment edges.
fn resolve_containments(parsed_files: &[ParsedFile]) -> Vec<(String, String)> {
    let mut registry = SymbolRegistry::new();

    for pf in parsed_files {
        let module_path = Rust.derive_module_path(&pf.file_path);
        for sym in &pf.symbols {
            let sid = sym.symbol_id();
            let qn = QualifiedName::new(&module_path, &sym.name);
            registry.register(qn, sid, &sym.kind, &sym.language);
        }
    }

    let mut edges = Vec::new();
    for pf in parsed_files {
        let module_path = Rust.derive_module_path(&pf.file_path);
        for cont in &pf.containments {
            let parent_qn = QualifiedName::new(&module_path, &cont.parent_name);
            if registry.qualified_map.get(&parent_qn).is_some() {
                let child = &pf.symbols[cont.child_symbol_idx];
                edges.push((cont.parent_name.clone(), child.name.clone()));
            } else {
                panic!(
                    "UNRESOLVABLE containment: parent '{}' (qualified '{}') \
                     not found in registry for child '{}' in file '{}'. \
                     Available qualified names: {:?}",
                    cont.parent_name,
                    parent_qn,
                    pf.symbols[cont.child_symbol_idx].name,
                    pf.file_path,
                    registry
                        .qualified_map
                        .keys()
                        .map(|k: &QualifiedName| k.as_str())
                        .collect::<Vec<_>>()
                );
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

    let refs: Vec<_> = parsed
        .type_refs
        .iter()
        .map(|tr| {
            (
                &parsed.symbols[tr.from_symbol_idx].name,
                &tr.type_name,
                &tr.ref_kind,
            )
        })
        .collect();

    // Expect:
    // update_server -> Server (ParamType, via &mut)
    // accept_direct -> Server (ParamType, direct)
    // get_server -> Server (ReturnType)
    // route -> Request (ParamType, via &)
    // route -> Response (ReturnType)
    assert!(
        refs.iter().any(|(name, ty, kind)| *name == "accept_direct"
            && *ty == "Server"
            && **kind == ReferenceType::ParamType),
        "accept_direct should accept Server\nrefs: {:?}\nsymbols: {:?}",
        refs,
        parsed
            .symbols
            .iter()
            .map(|s| (&s.name, s.start_line, s.end_line))
            .collect::<Vec<_>>()
    );
    assert!(
        refs.iter().any(|(name, ty, kind)| *name == "update_server"
            && *ty == "Server"
            && **kind == ReferenceType::ParamType),
        "update_server should accept Server, got: {:?}",
        refs
    );
    assert!(
        refs.iter().any(|(name, ty, kind)| *name == "get_server"
            && *ty == "Server"
            && **kind == ReferenceType::ReturnType),
        "get_server should return Server, got: {:?}",
        refs
    );
    assert!(
        refs.iter().any(|(name, ty, kind)| *name == "route"
            && *ty == "Request"
            && **kind == ReferenceType::ParamType),
        "route should accept Request, got: {:?}",
        refs
    );
    assert!(
        refs.iter().any(|(name, ty, kind)| *name == "route"
            && *ty == "Response"
            && **kind == ReferenceType::ReturnType),
        "route should return Response, got: {:?}",
        refs
    );
}

#[test]
fn test_param_type_refs() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // fn handle(&self, request: &Request) -> Response
    // should produce Accepts edge from handle to Request
    let handle_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "handle" && s.start_line == 20)
        .expect("handle trait method should exist");

    let has_request_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == handle_idx
            && tr.type_name == "Request"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_request_param,
        "handle should have Accepts ref to Request, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == handle_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_return_type_refs() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // fn handle(&self, request: &Request) -> Response
    // should produce Returns edge from handle to Response
    let handle_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "handle" && s.start_line == 20)
        .expect("handle trait method should exist");

    let has_response_return = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == handle_idx
            && tr.type_name == "Response"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_response_return,
        "handle should have Returns ref to Response, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == handle_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_field_type_refs() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // struct Server { handlers: HashMap<String, Box<dyn Handler>> }
    // The field "handlers" should have a FieldType ref to Handler
    let handlers_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "handlers" && s.kind == "field")
        .expect("handlers field should exist");

    let has_handler_ref = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == handlers_idx
            && tr.type_name == "Handler"
            && tr.ref_kind == ReferenceType::FieldType
    });
    assert!(
        has_handler_ref,
        "handlers field should have FieldType ref to Handler, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == handlers_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_create_default_config_returns_response() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // pub fn create_default_config() -> Response
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "create_default_config")
        .expect("create_default_config should exist");

    let has_response_return = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Response"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_response_return,
        "create_default_config should return Response, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_update_server_accepts_server() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // pub fn update_server(server: &mut Server)
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "update_server")
        .expect("update_server should exist");

    let has_server_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Server"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_server_param,
        "update_server should accept Server, got ALL type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .map(|tr| (
                &parsed.symbols[tr.from_symbol_idx].name,
                &tr.type_name,
                &tr.ref_kind
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_scoped_call_qualifier_type_ref() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    // Server::new() at line 172 inside test_server_creation should produce Usage ref to Server
    let test_fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "test_server_creation")
        .expect("test_server_creation should exist");

    let has_server_usage = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == test_fn_idx
            && tr.type_name == "Server"
            && tr.ref_kind == ReferenceType::Usage
    });
    assert!(
        has_server_usage,
        "test_server_creation should have Usage ref to Server from Server::new(), got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == test_fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_scoped_call_qualifier_filters_self() {
    let code = load_testdata("server.rs");
    let parsed = Rust::extract(&code, "src/server.rs");

    let has_self_usage = parsed
        .type_refs
        .iter()
        .any(|tr| tr.type_name == "Self" && tr.ref_kind == ReferenceType::Usage);
    assert!(
        !has_self_usage,
        "Self::method() should not produce a Usage ref"
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

    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "health")
        .expect("health function should exist");

    let has_health_response = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "HealthResponse"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_health_response,
        "health() -> Json<HealthResponse> should extract HealthResponse as ReturnType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_result_return_type_inner_arg() {
    // fn load_config() -> Result<Config, Error> should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "load_config")
        .expect("load_config function should exist");

    let has_config = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_config,
        "load_config() -> Result<Config, Error> should extract Config as ReturnType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_option_return_type_inner_arg() {
    // fn find_config() -> Option<Config> should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "find_config")
        .expect("find_config function should exist");

    let has_config = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_config,
        "find_config() -> Option<Config> should extract Config as ReturnType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_generic_param_type_inner_arg() {
    // fn process_items(items: Vec<Config>) should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "process_items")
        .expect("process_items function should exist");

    let has_config = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_config,
        "process_items(items: Vec<Config>) should extract Config as ParamType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_ref_generic_param_type_inner_arg() {
    // fn process_items_ref(items: &Vec<Config>) should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "process_items_ref")
        .expect("process_items_ref function should exist");

    let has_config = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_config,
        "process_items_ref(items: &Vec<Config>) should extract Config as ParamType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_generic_field_type_inner_arg() {
    // struct AppState { db: Arc<Database> } should extract Database
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let field_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "db" && s.kind == "field")
        .expect("db field should exist");

    let has_database = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == field_idx
            && tr.type_name == "Database"
            && tr.ref_kind == ReferenceType::FieldType
    });
    assert!(
        has_database,
        "AppState.db: Arc<Database> should extract Database as FieldType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == field_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_nested_generic_return_type_inner_arg() {
    // fn get_shared_db() -> Arc<Mutex<Database>> should extract Database
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "get_shared_db")
        .expect("get_shared_db function should exist");

    let has_database = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Database"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_database,
        "get_shared_db() -> Arc<Mutex<Database>> should extract Database as ReturnType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_impl_method_generic_return_type_inner_arg() {
    // impl AppState { fn get_config(&self) -> Option<Config> } should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let method_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "get_config" && s.kind == "function")
        .expect("get_config method should exist");

    let has_config = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == method_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_config,
        "AppState::get_config() -> Option<Config> should extract Config as ReturnType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == method_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
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

    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "get_handler")
        .expect("get_handler function should exist");

    let has_handler = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Handler"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_handler,
        "get_handler() -> impl Handler should extract Handler as ReturnType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_impl_method_abstract_type_return() {
    // impl AppState { fn get_service(&self) -> impl Service } should extract Service
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let method_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "get_service" && s.kind == "function")
        .expect("get_service method should exist");

    let has_service = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == method_idx
            && tr.type_name == "Service"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_service,
        "AppState::get_service() -> impl Service should extract Service as ReturnType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == method_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
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

    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "process_slice")
        .expect("process_slice function should exist");

    let has_config = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_config,
        "process_slice(items: &[Config]) should extract Config as ParamType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_array_param_type() {
    // fn process_array(items: [Item; 5]) should extract Item
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "process_array")
        .expect("process_array function should exist");

    let has_item = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Item"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_item,
        "process_array(items: [Item; 5]) should extract Item as ParamType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_impl_method_slice_param_type() {
    // impl AppState { fn update_configs(&self, configs: &[Config]) } should extract Config
    let code = load_testdata("typeref.rs");
    let parsed = Rust::extract(&code, "src/typeref.rs");

    let method_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "update_configs" && s.kind == "function")
        .expect("update_configs method should exist");

    let has_config = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == method_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_config,
        "AppState::update_configs(configs: &[Config]) should extract Config as ParamType, got: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == method_idx)
            .map(|tr| (&tr.type_name, &tr.ref_kind))
            .collect::<Vec<_>>()
    );
}
