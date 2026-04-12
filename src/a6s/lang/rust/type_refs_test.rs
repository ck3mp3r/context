use super::extractor::RustExtractor;
use crate::a6s::extract::LanguageExtractor;
use crate::a6s::types::EdgeKind;

/// Helper to extract name from SymbolId string (format: "symbol:path:name:line")
fn extract_name(symbol_id: &crate::a6s::types::SymbolId) -> Option<String> {
    let s = symbol_id.as_str().strip_prefix("symbol:")?;
    let parts: Vec<&str> = s.rsplitn(2, ':').collect();
    if parts.len() < 2 {
        return None;
    }
    let without_line = parts[1];
    let name_parts: Vec<&str> = without_line.rsplitn(2, ':').collect();
    if name_parts.is_empty() {
        return None;
    }
    Some(name_parts[0].to_string())
}

/// Test 1: Extract parameter type reference (direct type)
#[test]
fn test_extracts_parameter_type_reference() {
    let extractor = RustExtractor;
    let code = r#"
struct Config {}
fn process(cfg: Config) {}
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should find ParamType edge: process → Config
    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::ParamType))
        .collect();

    assert_eq!(
        param_edges.len(),
        1,
        "Expected 1 ParamType edge, found {}",
        param_edges.len()
    );

    // Verify edge points from function to type
    let edge = &param_edges[0];
    match &edge.from {
        crate::a6s::types::SymbolRef::Resolved(id) => {
            let name = extract_name(id).expect("Failed to extract from name");
            assert_eq!(name, "process", "Edge should be from 'process'");
        }
        _ => panic!("Expected resolved from reference"),
    }
    match &edge.to {
        crate::a6s::types::SymbolRef::Resolved(id) => {
            let name = extract_name(id).expect("Failed to extract to name");
            assert_eq!(name, "Config", "Edge should be to 'Config'");
        }
        _ => panic!("Expected resolved to reference"),
    }
}

/// Test 2: Extract return type reference
#[test]
fn test_extracts_return_type_reference() {
    let extractor = RustExtractor;
    let code = r#"
struct Config {}
fn get_config() -> Config { Config {} }
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should find ReturnType edge: get_config → Config
    let return_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::ReturnType))
        .collect();

    assert_eq!(
        return_edges.len(),
        1,
        "Expected 1 ReturnType edge, found {}",
        return_edges.len()
    );

    let edge = &return_edges[0];
    match &edge.from {
        crate::a6s::types::SymbolRef::Resolved(id) => {
            let name = extract_name(id).expect("Failed to extract from name");
            assert_eq!(name, "get_config", "Edge should be from 'get_config'");
        }
        _ => panic!("Expected resolved from reference"),
    }
    match &edge.to {
        crate::a6s::types::SymbolRef::Resolved(id) => {
            let name = extract_name(id).expect("Failed to extract to name");
            assert_eq!(name, "Config", "Edge should be to 'Config'");
        }
        _ => panic!("Expected resolved to reference"),
    }
}

/// Test 3: Extract field type reference
#[test]
fn test_extracts_field_type_reference() {
    let extractor = RustExtractor;
    let code = r#"
struct Config {}
struct Service {
    config: Config,
}
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should find FieldType edge: config field → Config type
    let field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::FieldType))
        .collect();

    assert_eq!(
        field_edges.len(),
        1,
        "Expected 1 FieldType edge, found {}",
        field_edges.len()
    );

    let edge = &field_edges[0];
    match &edge.from {
        crate::a6s::types::SymbolRef::Resolved(id) => {
            let name = extract_name(id).expect("Failed to extract from name");
            assert_eq!(name, "config", "Edge should be from 'config' field");
        }
        _ => panic!("Expected resolved from reference"),
    }
    match &edge.to {
        crate::a6s::types::SymbolRef::Resolved(id) => {
            let name = extract_name(id).expect("Failed to extract to name");
            assert_eq!(name, "Config", "Edge should be to 'Config'");
        }
        _ => panic!("Expected resolved to reference"),
    }
}

/// Test 4: Extract generic type argument (Vec<Config>)
#[test]
fn test_handles_generic_type_reference() {
    let extractor = RustExtractor;
    let code = r#"
struct Config {}
fn process(items: Vec<Config>) {}
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should find ParamType edge to Config (inner type)
    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::ParamType))
        .collect();

    // Should have at least one edge to Config
    let config_edges: Vec<_> = param_edges
        .iter()
        .filter(|e| match &e.to {
            crate::a6s::types::SymbolRef::Resolved(id) => {
                extract_name(id).map_or(false, |name| name == "Config")
            }
            _ => false,
        })
        .collect();

    assert!(
        !config_edges.is_empty(),
        "Expected edge to Config type inside Vec<Config>"
    );
}

/// Test 5: Extract reference type (&Config)
#[test]
fn test_handles_reference_type() {
    let extractor = RustExtractor;
    let code = r#"
struct Config {}
fn read(cfg: &Config) {}
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should find ParamType edge to Config (not the & wrapper)
    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::ParamType))
        .collect();

    assert_eq!(
        param_edges.len(),
        1,
        "Expected 1 ParamType edge for &Config"
    );

    let edge = &param_edges[0];
    match &edge.to {
        crate::a6s::types::SymbolRef::Resolved(id) => {
            let name = extract_name(id).expect("Failed to extract to name");
            assert_eq!(name, "Config", "Edge should be to 'Config', not '&Config'");
        }
        _ => panic!("Expected resolved to reference"),
    }
}

/// Test 6: Extract mutable reference type (&mut Config)
#[test]
fn test_handles_mutable_reference() {
    let extractor = RustExtractor;
    let code = r#"
struct Config {}
fn modify(cfg: &mut Config) {}
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should find ParamType edge to Config
    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::ParamType))
        .collect();

    assert_eq!(
        param_edges.len(),
        1,
        "Expected 1 ParamType edge for &mut Config"
    );

    let edge = &param_edges[0];
    match &edge.to {
        crate::a6s::types::SymbolRef::Resolved(id) => {
            let name = extract_name(id).expect("Failed to extract to name");
            assert_eq!(name, "Config", "Edge should be to 'Config'");
        }
        _ => panic!("Expected resolved to reference"),
    }
}

/// Test 7: Extract Option<Config> type
#[test]
fn test_handles_option_type() {
    let extractor = RustExtractor;
    let code = r#"
struct Config {}
fn maybe_get() -> Option<Config> { None }
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should find ReturnType edge to Config (inner type)
    let return_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::ReturnType))
        .collect();

    let config_edges: Vec<_> = return_edges
        .iter()
        .filter(|e| match &e.to {
            crate::a6s::types::SymbolRef::Resolved(id) => {
                extract_name(id).map_or(false, |name| name == "Config")
            }
            _ => false,
        })
        .collect();

    assert!(
        !config_edges.is_empty(),
        "Expected edge to Config type inside Option<Config>"
    );
}

/// Test 8: Extract Result<Config, Error> types
#[test]
fn test_handles_result_type() {
    let extractor = RustExtractor;
    let code = r#"
struct Config {}
struct Error {}
fn load() -> Result<Config, Error> { Ok(Config {}) }
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should find ReturnType edges to both Config and Error
    let return_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::ReturnType))
        .collect();

    let config_edges: Vec<_> = return_edges
        .iter()
        .filter(|e| match &e.to {
            crate::a6s::types::SymbolRef::Resolved(id) => {
                extract_name(id).map_or(false, |name| name == "Config")
            }
            _ => false,
        })
        .collect();

    let error_edges: Vec<_> = return_edges
        .iter()
        .filter(|e| match &e.to {
            crate::a6s::types::SymbolRef::Resolved(id) => {
                extract_name(id).map_or(false, |name| name == "Error")
            }
            _ => false,
        })
        .collect();

    assert!(
        !config_edges.is_empty(),
        "Expected edge to Config type in Result<Config, Error>"
    );
    assert!(
        !error_edges.is_empty(),
        "Expected edge to Error type in Result<Config, Error>"
    );
}

/// Test 9: Extract multiple parameter types
#[test]
fn test_handles_multiple_params() {
    let extractor = RustExtractor;
    let code = r#"
struct TypeA {}
struct TypeB {}
fn combine(a: TypeA, b: TypeB) {}
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should find ParamType edges to both TypeA and TypeB
    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::ParamType))
        .collect();

    let type_a_edges: Vec<_> = param_edges
        .iter()
        .filter(|e| match &e.to {
            crate::a6s::types::SymbolRef::Resolved(id) => {
                extract_name(id).map_or(false, |name| name == "TypeA")
            }
            _ => false,
        })
        .collect();

    let type_b_edges: Vec<_> = param_edges
        .iter()
        .filter(|e| match &e.to {
            crate::a6s::types::SymbolRef::Resolved(id) => {
                extract_name(id).map_or(false, |name| name == "TypeB")
            }
            _ => false,
        })
        .collect();

    assert!(!type_a_edges.is_empty(), "Expected edge to TypeA");
    assert!(!type_b_edges.is_empty(), "Expected edge to TypeB");
}

/// Test 10: Ignore primitive types (no edges created)
#[test]
fn test_ignores_primitive_types() {
    let extractor = RustExtractor;
    let code = r#"
fn calculate(x: i32, y: u64, z: f64, s: bool) -> usize { 42 }
"#;
    let parsed = extractor.extract(code, "test.rs");

    // Should have NO type reference edges (primitives aren't symbols)
    let type_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                EdgeKind::ParamType | EdgeKind::ReturnType | EdgeKind::FieldType
            )
        })
        .collect();

    assert_eq!(
        type_edges.len(),
        0,
        "Expected no type edges for primitive types, found {}",
        type_edges.len()
    );
}
