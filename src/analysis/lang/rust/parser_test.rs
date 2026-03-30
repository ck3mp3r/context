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
