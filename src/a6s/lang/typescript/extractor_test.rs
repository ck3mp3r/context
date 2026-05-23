use super::extractor::TypeScriptExtractor;
use crate::a6s::extract::LanguageExtractor;
use crate::a6s::types::EdgeKind;

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/a6s/lang/typescript/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

#[test]
fn test_language_and_extensions() {
    let ext = TypeScriptExtractor;
    assert_eq!(ext.language(), "typescript");
    assert_eq!(ext.extensions(), &["ts", "tsx"]);
}

#[test]
fn test_empty_extract_returns_parsed_file() {
    let ext = TypeScriptExtractor;
    let result = ext.extract("", "test.ts");
    assert_eq!(result.file_path, "test.ts");
    assert_eq!(result.language, "typescript");
    assert!(result.symbols.is_empty());
}

#[test]
fn test_grammar_loads() {
    let ext = TypeScriptExtractor;
    let language = ext.grammar();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .expect("Failed to set TypeScript language");

    let tree = parser
        .parse("function main() {}", None)
        .expect("Failed to parse simple TypeScript code");
    assert!(tree.root_node().child_count() > 0);
}

#[test]
fn test_discoverable_via_extractor() {
    use crate::a6s::extract::Extractor;

    let ext = Extractor::for_language("typescript");
    assert!(ext.is_some());
    assert_eq!(ext.unwrap().language(), "typescript");

    let ext_ts = Extractor::for_extension("ts");
    assert!(ext_ts.is_some());
    assert_eq!(ext_ts.unwrap().language(), "typescript");

    let ext_tsx = Extractor::for_extension("tsx");
    assert!(ext_tsx.is_some());
    assert_eq!(ext_tsx.unwrap().language(), "typescript");
}

// ============================================================================
// Phase 2: Symbol Extraction Tests
// ============================================================================

#[test]
fn test_extracts_class() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let class = result
        .symbols
        .iter()
        .find(|s| s.name == "UserService" && s.kind == "class")
        .expect("Should find UserService class");
    assert_eq!(class.visibility.as_deref(), Some("pub"));
    assert_eq!(class.signature.as_deref(), Some("class"));
}

#[test]
fn test_extracts_abstract_class() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let class = result
        .symbols
        .iter()
        .find(|s| s.name == "BaseEntity" && s.kind == "class")
        .expect("Should find BaseEntity abstract class");
    assert_eq!(class.visibility.as_deref(), Some("pub"));
    assert!(
        class.signature.as_deref().unwrap().contains("abstract"),
        "Signature should contain 'abstract', got: {:?}",
        class.signature
    );
}

#[test]
fn test_extracts_interface() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let iface = result
        .symbols
        .iter()
        .find(|s| s.name == "Repository" && s.kind == "interface")
        .expect("Should find Repository interface");
    assert_eq!(iface.visibility.as_deref(), Some("pub"));
}

#[test]
fn test_extracts_type_alias() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let pub_type = result
        .symbols
        .iter()
        .find(|s| s.name == "UserId" && s.kind == "type_alias")
        .expect("Should find UserId type alias");
    assert_eq!(pub_type.visibility.as_deref(), Some("pub"));

    let priv_type = result
        .symbols
        .iter()
        .find(|s| s.name == "InternalConfig" && s.kind == "type_alias")
        .expect("Should find InternalConfig type alias");
    assert_eq!(priv_type.visibility.as_deref(), Some("private"));
}

#[test]
fn test_extracts_enum() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let en = result
        .symbols
        .iter()
        .find(|s| s.name == "Status" && s.kind == "enum")
        .expect("Should find Status enum");
    assert_eq!(en.visibility.as_deref(), Some("pub"));
}

#[test]
fn test_extracts_enum_members() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let members: Vec<&str> = result
        .symbols
        .iter()
        .filter(|s| s.kind == "enum_entry")
        .map(|s| s.name.as_str())
        .collect();
    assert!(
        members.contains(&"Active"),
        "Should have Active enum member"
    );
    assert!(
        members.contains(&"Inactive"),
        "Should have Inactive enum member"
    );
    assert!(
        members.contains(&"Pending"),
        "Should have Pending enum member"
    );
}

#[test]
fn test_extracts_functions() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let pub_fn = result
        .symbols
        .iter()
        .find(|s| s.name == "createUser" && s.kind == "function")
        .expect("Should find createUser function");
    assert_eq!(pub_fn.visibility.as_deref(), Some("pub"));

    let priv_fn = result
        .symbols
        .iter()
        .find(|s| s.name == "internalHelper" && s.kind == "function")
        .expect("Should find internalHelper function");
    assert_eq!(priv_fn.visibility.as_deref(), Some("private"));
}

#[test]
fn test_extracts_methods() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let methods: Vec<&str> = result
        .symbols
        .iter()
        .filter(|s| s.kind == "method")
        .map(|s| s.name.as_str())
        .collect();

    assert!(methods.contains(&"getName"), "Should have getName method");
    assert!(methods.contains(&"create"), "Should have create method");
    assert!(
        methods.contains(&"constructor"),
        "Should have constructor method"
    );

    // Static method should have static in signature
    let create = result
        .symbols
        .iter()
        .find(|s| s.name == "create" && s.kind == "method")
        .unwrap();
    assert!(
        create.signature.as_deref().unwrap_or("").contains("static"),
        "create should be static, got: {:?}",
        create.signature
    );
}

#[test]
fn test_extracts_properties() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let name_prop = result
        .symbols
        .iter()
        .find(|s| s.name == "name" && s.kind == "property")
        .expect("Should find name property");
    assert_eq!(name_prop.visibility.as_deref(), Some("private"));

    let id_prop = result
        .symbols
        .iter()
        .find(|s| s.name == "id" && s.kind == "property")
        .expect("Should find id property");
    assert!(
        id_prop
            .signature
            .as_deref()
            .unwrap_or("")
            .contains("readonly"),
        "id should be readonly, got: {:?}",
        id_prop.signature
    );
}

#[test]
fn test_extracts_const_and_var() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let max = result
        .symbols
        .iter()
        .find(|s| s.name == "MAX_USERS" && s.kind == "const")
        .expect("Should find MAX_USERS const");
    assert_eq!(max.visibility.as_deref(), Some("pub"));

    let secret = result
        .symbols
        .iter()
        .find(|s| s.name == "SECRET_KEY" && s.kind == "const")
        .expect("Should find SECRET_KEY const");
    assert_eq!(secret.visibility.as_deref(), Some("private"));

    let mutable = result
        .symbols
        .iter()
        .find(|s| s.name == "mutableCount" && s.kind == "var")
        .expect("Should find mutableCount var");
    assert_eq!(mutable.visibility.as_deref(), Some("pub"));
}

#[test]
fn test_extracts_arrow_functions() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("functions.ts");
    let result = ext.extract(&code, "functions.ts");

    let fetch = result
        .symbols
        .iter()
        .find(|s| s.name == "fetchData" && s.kind == "function")
        .expect("Should find fetchData arrow function");
    assert!(
        fetch.signature.as_deref().unwrap_or("").contains("arrow"),
        "fetchData should be arrow, got: {:?}",
        fetch.signature
    );
    assert!(
        fetch.signature.as_deref().unwrap_or("").contains("async"),
        "fetchData should be async, got: {:?}",
        fetch.signature
    );

    let add = result
        .symbols
        .iter()
        .find(|s| s.name == "add" && s.kind == "function")
        .expect("Should find add arrow function");
    assert!(
        add.signature.as_deref().unwrap_or("").contains("arrow"),
        "add should be arrow, got: {:?}",
        add.signature
    );
}

#[test]
fn test_extracts_async_function() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("functions.ts");
    let result = ext.extract(&code, "functions.ts");

    let load = result
        .symbols
        .iter()
        .find(|s| s.name == "loadConfig" && s.kind == "function")
        .expect("Should find loadConfig function");
    assert!(
        load.signature.as_deref().unwrap_or("").contains("async"),
        "loadConfig should be async, got: {:?}",
        load.signature
    );
}

#[test]
fn test_extracts_generator() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("functions.ts");
    let result = ext.extract(&code, "functions.ts");

    let gen_fn = result
        .symbols
        .iter()
        .find(|s| s.name == "generateIds" && s.kind == "function")
        .expect("Should find generateIds function");
    assert!(
        gen_fn
            .signature
            .as_deref()
            .unwrap_or("")
            .contains("generator"),
        "generateIds should be generator, got: {:?}",
        gen_fn.signature
    );
}

#[test]
fn test_extracts_interface_methods() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let iface_methods: Vec<&str> = result
        .symbols
        .iter()
        .filter(|s| s.kind == "interface_method")
        .map(|s| s.name.as_str())
        .collect();

    assert!(
        iface_methods.contains(&"findById"),
        "Should have findById interface method"
    );
    assert!(
        iface_methods.contains(&"save"),
        "Should have save interface method"
    );
    assert!(
        iface_methods.contains(&"delete"),
        "Should have delete interface method"
    );
}

#[test]
fn test_extracts_abstract_method() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");

    let get_id = result
        .symbols
        .iter()
        .find(|s| s.name == "getId" && s.kind == "method")
        .expect("Should find getId abstract method");
    assert!(
        get_id
            .signature
            .as_deref()
            .unwrap_or("")
            .contains("abstract"),
        "getId should be abstract, got: {:?}",
        get_id.signature
    );
}

// ============================================================================
// Phase 3: Structural and Inheritance Edge Tests
// ============================================================================

fn has_edge(
    result: &crate::a6s::types::ParsedFile,
    from_name: &str,
    to_name: &str,
    kind: EdgeKind,
) -> bool {
    result.edges.iter().any(|e| {
        let from_match = match &e.from {
            crate::a6s::types::SymbolRef::Resolved(id) => {
                id.as_str().contains(&format!(":{from_name}:"))
            }
            crate::a6s::types::SymbolRef::Unresolved { name, .. } => name == from_name,
        };
        let to_match = match &e.to {
            crate::a6s::types::SymbolRef::Resolved(id) => {
                id.as_str().contains(&format!(":{to_name}:"))
            }
            crate::a6s::types::SymbolRef::Unresolved { name, .. } => name == to_name,
        };
        from_match && to_match && e.kind == kind
    })
}

#[test]
fn test_class_has_method_edges() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("edges.ts");
    let result = ext.extract(&code, "edges.ts");

    assert!(
        has_edge(&result, "Calculator", "constructor", EdgeKind::HasMethod),
        "Calculator should HasMethod constructor"
    );
    assert!(
        has_edge(&result, "Calculator", "add", EdgeKind::HasMethod),
        "Calculator should HasMethod add"
    );
    assert!(
        has_edge(&result, "Calculator", "getResult", EdgeKind::HasMethod),
        "Calculator should HasMethod getResult"
    );
}

#[test]
fn test_class_has_field_edges() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("edges.ts");
    let result = ext.extract(&code, "edges.ts");

    assert!(
        has_edge(&result, "Calculator", "value", EdgeKind::HasField),
        "Calculator should HasField value"
    );
}

#[test]
fn test_extends_edge() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("inheritance.ts");
    let result = ext.extract(&code, "inheritance.ts");

    assert!(
        has_edge(&result, "User", "BaseModel", EdgeKind::Extends),
        "User should Extends BaseModel"
    );
}

#[test]
fn test_implements_edge() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("inheritance.ts");
    let result = ext.extract(&code, "inheritance.ts");

    assert!(
        has_edge(&result, "BaseModel", "Serializable", EdgeKind::Implements),
        "BaseModel should Implements Serializable"
    );
    assert!(
        has_edge(&result, "User", "Identifiable", EdgeKind::Implements),
        "User should Implements Identifiable"
    );
}

#[test]
fn test_interface_extends_edge() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("inheritance.ts");
    let result = ext.extract(&code, "inheritance.ts");

    assert!(
        has_edge(
            &result,
            "ReadonlyRepository",
            "Identifiable",
            EdgeKind::Extends
        ),
        "ReadonlyRepository should Extends Identifiable"
    );
}

#[test]
fn test_interface_has_method_edges() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("inheritance.ts");
    let result = ext.extract(&code, "inheritance.ts");

    assert!(
        has_edge(&result, "Serializable", "serialize", EdgeKind::HasMethod),
        "Serializable should HasMethod serialize"
    );
    assert!(
        has_edge(&result, "Identifiable", "getId", EdgeKind::HasMethod),
        "Identifiable should HasMethod getId"
    );
}

#[test]
fn test_has_member_edges_top_level() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("edges.ts");
    let result = ext.extract(&code, "edges.ts");

    // Top-level symbols should have HasMember edges from the file module
    let file_member_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasMember)
        .collect();
    assert!(
        !file_member_edges.is_empty(),
        "Should have HasMember edges for top-level symbols"
    );

    // Calculator should be a member of the file module
    assert!(
        file_member_edges.iter().any(|e| {
            match &e.to {
                crate::a6s::types::SymbolRef::Resolved(id) => id.as_str().contains(":Calculator:"),
                _ => false,
            }
        }),
        "Calculator should be a HasMember of the file module"
    );
}

// ============================================================================
// Phase 4: Type References and Call Edge Tests
// ============================================================================

#[test]
fn test_param_type_refs() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("type_refs.ts");
    let result = ext.extract(&code, "type_refs.ts");

    assert!(
        has_edge(&result, "constructor", "Logger", EdgeKind::ParamType),
        "constructor should have ParamType to Logger"
    );
    assert!(
        has_edge(&result, "process", "Request", EdgeKind::ParamType),
        "process should have ParamType to Request"
    );
    assert!(
        has_edge(&result, "transform", "Buffer", EdgeKind::ParamType),
        "transform should have ParamType to Buffer"
    );
    assert!(
        has_edge(&result, "transform", "TextEncoder", EdgeKind::ParamType),
        "transform should have ParamType to TextEncoder"
    );
}

#[test]
fn test_return_type_refs() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("type_refs.ts");
    let result = ext.extract(&code, "type_refs.ts");

    assert!(
        has_edge(&result, "process", "Response", EdgeKind::ReturnType),
        "process should have ReturnType to Response"
    );
    assert!(
        has_edge(&result, "transform", "Uint8Array", EdgeKind::ReturnType),
        "transform should have ReturnType to Uint8Array"
    );
}

#[test]
fn test_field_type_refs() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("type_refs.ts");
    let result = ext.extract(&code, "type_refs.ts");

    assert!(
        has_edge(&result, "logger", "Logger", EdgeKind::FieldType),
        "logger field should have FieldType to Logger"
    );
}

#[test]
fn test_generic_type_ref() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("type_refs.ts");
    let result = ext.extract(&code, "type_refs.ts");

    // Array<Item> — Array is builtin, but Item should produce a TypeRef
    assert!(
        has_edge(&result, "getItems", "Item", EdgeKind::TypeRef),
        "getItems should have TypeRef to Item from Array<Item>"
    );
}

#[test]
fn test_calls_edges() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("type_refs.ts");
    let result = ext.extract(&code, "type_refs.ts");

    assert!(
        has_edge(&result, "process", "log", EdgeKind::Calls),
        "process should Calls log"
    );
    assert!(
        has_edge(&result, "transform", "encode", EdgeKind::Calls),
        "transform should Calls encode"
    );
}

// ============================================================================
// Phase 5: Import Tests
// ============================================================================

#[test]
fn test_named_imports() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("imports.ts");
    let result = ext.extract(&code, "imports.ts");

    let named = result
        .imports
        .iter()
        .find(|i| i.entry.module_path == "symbols" && !i.entry.is_glob)
        .expect("Should find named import from symbols");
    assert!(
        named
            .entry
            .imported_names
            .contains(&"UserService".to_string())
    );
    assert!(named.entry.imported_names.contains(&"UserId".to_string()));
}

#[test]
fn test_aliased_import() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("imports.ts");
    let result = ext.extract(&code, "imports.ts");

    let aliased = result
        .imports
        .iter()
        .find(|i| i.entry.module_path == "edges" && i.entry.alias == Some("Calc".to_string()))
        .expect("Should find aliased import Calculator as Calc");
    assert!(
        aliased
            .entry
            .imported_names
            .contains(&"Calculator".to_string())
    );
}

#[test]
fn test_namespace_import() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("imports.ts");
    let result = ext.extract(&code, "imports.ts");

    let ns = result
        .imports
        .iter()
        .find(|i| i.entry.is_glob && i.entry.module_path == "edges")
        .expect("Should find namespace import from edges");
    assert_eq!(ns.entry.alias, Some("MathUtils".to_string()));
}

#[test]
fn test_default_import() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("imports.ts");
    let result = ext.extract(&code, "imports.ts");

    let default = result
        .imports
        .iter()
        .find(|i| {
            i.entry.module_path == "default-module"
                && i.entry.alias == Some("DefaultExport".to_string())
        })
        .expect("Should find default import from default-module");
    assert!(
        default
            .entry
            .imported_names
            .contains(&"default".to_string())
    );
}

#[test]
fn test_side_effect_import_ignored() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("imports.ts");
    let result = ext.extract(&code, "imports.ts");

    let side_effect = result
        .imports
        .iter()
        .any(|i| i.entry.module_path == "side-effect-module");
    assert!(
        !side_effect,
        "Side-effect import should not produce an ImportEntry"
    );
}

#[test]
fn test_type_only_import() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("imports.ts");
    let result = ext.extract(&code, "imports.ts");

    let type_import = result
        .imports
        .iter()
        .find(|i| i.entry.module_path == "type_refs")
        .expect("Should find type-only import from type_refs");
    assert!(
        type_import
            .entry
            .imported_names
            .contains(&"Logger".to_string())
    );
}

#[test]
fn test_cross_file_named_import_resolution() {
    let ext = TypeScriptExtractor;

    // Parse "symbols" file
    let symbols_code = load_testdata("symbols.ts");
    let symbols_parsed = ext.extract(&symbols_code, "symbols.ts");

    // Parse "imports" file
    let imports_code = load_testdata("imports.ts");
    let imports_parsed = ext.extract(&imports_code, "imports.ts");

    let mut files = vec![symbols_parsed, imports_parsed];
    let (_resolved_edges, resolved_imports) = ext.resolve_cross_file(&mut files);

    // Should resolve UserService import from symbols.ts
    let has_user_service = resolved_imports
        .iter()
        .any(|ri| ri.target_symbol_id.as_str().contains(":UserService:"));
    assert!(
        has_user_service,
        "Should resolve UserService import, got: {:?}",
        resolved_imports
    );
}

// ============================================================================
// Phase 6: Test Detection, Decorators, TSX
// ============================================================================

#[test]
fn test_file_category_test_file() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("tests.ts");

    let result = ext.extract(&code, "src/__tests__/user.test.ts");
    assert_eq!(
        result.file_category.as_deref(),
        Some("test_file"),
        "Files in __tests__/ should be test_file"
    );

    let result2 = ext.extract(&code, "user.spec.ts");
    assert_eq!(
        result2.file_category.as_deref(),
        Some("test_file"),
        "*.spec.ts should be test_file"
    );

    let result3 = ext.extract(&code, "user.test.tsx");
    assert_eq!(
        result3.file_category.as_deref(),
        Some("test_file"),
        "*.test.tsx should be test_file"
    );
}

#[test]
fn test_file_category_regular() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("symbols.ts");
    let result = ext.extract(&code, "symbols.ts");
    assert_eq!(
        result.file_category, None,
        "Regular .ts files should have no file_category"
    );
}

#[test]
fn test_declaration_file_detection() {
    let ext = TypeScriptExtractor;
    let result = ext.extract("declare module 'foo' {}", "types.d.ts");
    assert_eq!(
        result.file_category.as_deref(),
        Some("declaration"),
        "*.d.ts files should be 'declaration'"
    );
}

#[test]
fn test_tsx_parses_without_error() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("component.tsx");
    let result = ext.extract(&code, "component.tsx");
    assert!(
        !result.symbols.is_empty(),
        "TSX should parse and produce symbols"
    );
}

#[test]
fn test_tsx_extracts_symbols() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("component.tsx");
    let result = ext.extract(&code, "component.tsx");

    let button = result
        .symbols
        .iter()
        .find(|s| s.name == "Button" && s.kind == "function");
    assert!(
        button.is_some(),
        "Should extract Button arrow function component"
    );

    let class_comp = result
        .symbols
        .iter()
        .find(|s| s.name == "ClassComponent" && s.kind == "class");
    assert!(class_comp.is_some(), "Should extract ClassComponent class");

    let render = result
        .symbols
        .iter()
        .find(|s| s.name == "render" && s.kind == "method");
    assert!(render.is_some(), "Should extract render method");
}

#[test]
fn test_tsx_class_extends() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("component.tsx");
    let result = ext.extract(&code, "component.tsx");

    // ClassComponent extends React.Component<ButtonProps>
    assert!(
        has_edge(
            &result,
            "ClassComponent",
            "React.Component",
            EdgeKind::Extends
        ),
        "ClassComponent should Extends React.Component, edges: {:?}",
        result
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Extends)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_complex_extracts_decorated_class() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("complex.ts");
    let result = ext.extract(&code, "complex.ts");

    let auth = result
        .symbols
        .iter()
        .find(|s| s.name == "AuthService" && s.kind == "class");
    assert!(
        auth.is_some(),
        "Should extract AuthService class (decorated)"
    );

    let authenticate = result
        .symbols
        .iter()
        .find(|s| s.name == "authenticate" && s.kind == "method");
    assert!(
        authenticate.is_some(),
        "Should extract authenticate method (decorated)"
    );
}

#[test]
fn test_complex_field_type_ref() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("complex.ts");
    let result = ext.extract(&code, "complex.ts");

    assert!(
        has_edge(&result, "logger", "Logger", EdgeKind::FieldType),
        "logger should have FieldType to Logger"
    );
}

// ============================================================================
// Phase 7: Integration Tests
// ============================================================================

fn load_project_file(name: &str) -> String {
    let path = format!(
        "{}/src/a6s/lang/typescript/testdata/project/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

fn extract_project() -> (TypeScriptExtractor, Vec<crate::a6s::types::ParsedFile>) {
    let ext = TypeScriptExtractor;
    let files = vec![
        ("project/models.ts", load_project_file("models.ts")),
        ("project/user.ts", load_project_file("user.ts")),
        ("project/service.ts", load_project_file("service.ts")),
        ("project/index.ts", load_project_file("index.ts")),
    ];
    let parsed: Vec<_> = files
        .iter()
        .map(|(path, code)| ext.extract(code, path))
        .collect();
    (ext, parsed)
}

#[test]
fn test_integration_all_files_parse() {
    let (_, parsed) = extract_project();
    for pf in &parsed {
        assert!(!pf.file_path.is_empty());
        assert_eq!(pf.language, "typescript");
    }
    assert_eq!(parsed.len(), 4);
}

#[test]
fn test_integration_symbols_across_files() {
    let (_, parsed) = extract_project();

    // models.ts: Entity, Repository interfaces
    let models = &parsed[0];
    assert!(
        models
            .symbols
            .iter()
            .any(|s| s.name == "Entity" && s.kind == "interface")
    );
    assert!(
        models
            .symbols
            .iter()
            .any(|s| s.name == "Repository" && s.kind == "interface")
    );

    // user.ts: User interface, UserRepository class
    let user = &parsed[1];
    assert!(
        user.symbols
            .iter()
            .any(|s| s.name == "User" && s.kind == "interface")
    );
    assert!(
        user.symbols
            .iter()
            .any(|s| s.name == "UserRepository" && s.kind == "class")
    );

    // service.ts: UserService class
    let service = &parsed[2];
    assert!(
        service
            .symbols
            .iter()
            .any(|s| s.name == "UserService" && s.kind == "class")
    );
}

#[test]
fn test_integration_cross_file_resolution() {
    let (ext, mut parsed) = extract_project();
    let (_edges, imports) = ext.resolve_cross_file(&mut parsed);

    // user.ts imports Entity, Repository from models.ts
    let has_entity_import = imports
        .iter()
        .any(|ri| ri.target_symbol_id.as_str().contains(":Entity:"));
    assert!(
        has_entity_import,
        "Should resolve Entity import from models.ts"
    );

    let has_repo_import = imports
        .iter()
        .any(|ri| ri.target_symbol_id.as_str().contains(":Repository:"));
    assert!(
        has_repo_import,
        "Should resolve Repository import from models.ts"
    );
}

#[test]
fn test_integration_inheritance_edges() {
    let (_, parsed) = extract_project();

    // User extends Entity
    let user_file = &parsed[1];
    assert!(
        has_edge(user_file, "User", "Entity", EdgeKind::Extends),
        "User should Extends Entity"
    );

    // UserRepository implements Repository
    assert!(
        has_edge(
            user_file,
            "UserRepository",
            "Repository",
            EdgeKind::Implements
        ),
        "UserRepository should Implements Repository, edges: {:?}",
        user_file
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Implements)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_integration_empty_file() {
    let ext = TypeScriptExtractor;
    let code = load_project_file("empty.ts");
    let result = ext.extract(&code, "project/empty.ts");
    assert_eq!(result.language, "typescript");
    // Empty file should not panic
}

#[test]
fn test_integration_syntax_error_file() {
    let ext = TypeScriptExtractor;
    let code = load_project_file("broken.ts");
    let result = ext.extract(&code, "project/broken.ts");
    // Should still extract what it can without panicking
    assert_eq!(result.language, "typescript");
    // The class should still be found even with syntax error
    let broken_class = result
        .symbols
        .iter()
        .find(|s| s.name == "Broken" && s.kind == "class");
    assert!(
        broken_class.is_some(),
        "Should extract Broken class despite syntax error"
    );
}

#[test]
fn test_integration_barrel_file() {
    let (_, parsed) = extract_project();

    // index.ts is a barrel file — should have imports
    let index = &parsed[3];
    assert!(
        !index.imports.is_empty(),
        "Barrel file should have imports/re-exports"
    );
}

// ============================================================================
// Bug fix: Inline type annotation members should NOT be extracted as symbols
// ============================================================================

#[test]
fn test_no_symbols_from_inline_object_types() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("inline_types.ts");
    let result = ext.extract(&code, "inline_types.ts");

    // The `dispose` inside `{ dispose?(): void }` inline type should NOT be extracted
    // Only the interface method and class method `dispose` should be extracted
    let dispose_symbols: Vec<_> = result
        .symbols
        .iter()
        .filter(|s| s.name == "dispose")
        .collect();

    // Should have exactly 2: one from Disposable interface, one from Resource class
    assert_eq!(
        dispose_symbols.len(),
        2,
        "Should only extract dispose from interface and class, not inline types. Got: {:?}",
        dispose_symbols
            .iter()
            .map(|s| format!("{}:{} line {}", s.kind, s.name, s.start_line))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_no_symbols_from_type_alias_object_literal() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("inline_types.ts");
    let result = ext.extract(&code, "inline_types.ts");

    // `process` and `cleanup` inside ComplexType's nested object type should NOT be extracted
    let process_symbols: Vec<_> = result
        .symbols
        .iter()
        .filter(|s| s.name == "process")
        .collect();
    assert!(
        process_symbols.is_empty(),
        "Should not extract process from nested object type literal. Got: {:?}",
        process_symbols
            .iter()
            .map(|s| format!("{}:{} line {}", s.kind, s.name, s.start_line))
            .collect::<Vec<_>>()
    );

    let cleanup_symbols: Vec<_> = result
        .symbols
        .iter()
        .filter(|s| s.name == "cleanup")
        .collect();
    assert!(
        cleanup_symbols.is_empty(),
        "Should not extract cleanup from nested object type literal. Got: {:?}",
        cleanup_symbols
            .iter()
            .map(|s| format!("{}:{} line {}", s.kind, s.name, s.start_line))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_no_duplicate_symbol_ids() {
    let ext = TypeScriptExtractor;
    let code = load_testdata("inline_types.ts");
    let result = ext.extract(&code, "inline_types.ts");

    // Check for duplicate symbol IDs (same file:name:line)
    let mut seen = std::collections::HashSet::new();
    for sym in &result.symbols {
        let id = format!("{}:{}:{}", sym.file_path, sym.name, sym.start_line);
        assert!(seen.insert(id.clone()), "Duplicate symbol ID: {}", id);
    }
}

// ============================================================================
// Import-Aware Cross-File Resolution Tests
// ============================================================================

#[test]
fn test_resolve_cross_file_ambiguous_name_with_import() {
    use crate::a6s::types::{ImportEntry, RawImport};

    let ext = TypeScriptExtractor;

    // File 1: calls helper(), has named import from src/utils
    let code1 = r#"
import { helper } from '../utils';
export function handler() { helper(); }
"#;
    let mut file1 = ext.extract(code1, "src/api/handler.ts");
    file1.imports.push(RawImport {
        file_path: "src/api/handler.ts".to_string(),
        entry: ImportEntry::named_import("src/utils", vec!["helper".to_string()]),
    });

    // File 2: defines helper()
    let code2 = r#"export function helper() {}"#;
    let file2 = ext.extract(code2, "src/utils.ts");

    // File 3: ALSO defines helper()
    let code3 = r#"export function helper() {}"#;
    let file3 = ext.extract(code3, "src/core/helpers.ts");

    let mut files = [file1, file2, file3];
    let (resolved, _) = ext.resolve_cross_file(&mut files);

    let calls: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .collect();
    assert!(
        calls.iter().any(|e| e.to.as_str().contains(":helper:")),
        "Should resolve ambiguous name via import, got: {:?}",
        calls
    );
}

#[test]
fn test_resolve_cross_file_alias_import() {
    use crate::a6s::types::{ImportEntry, RawImport};

    let ext = TypeScriptExtractor;

    // File 1: calls dbHelper(), aliased import
    let code1 = r#"
import { helper as dbHelper } from '../utils';
export function handler() { dbHelper(); }
"#;
    let mut file1 = ext.extract(code1, "src/api/handler.ts");
    let mut entry = ImportEntry::named_import("src/utils", vec!["helper".to_string()]);
    entry.alias = Some("dbHelper".to_string());
    file1.imports.push(RawImport {
        file_path: "src/api/handler.ts".to_string(),
        entry,
    });

    // File 2: defines helper()
    let code2 = r#"export function helper() {}"#;
    let file2 = ext.extract(code2, "src/utils.ts");

    let mut files = [file1, file2];
    let (resolved, _) = ext.resolve_cross_file(&mut files);

    let calls: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .collect();
    assert!(
        calls.iter().any(|e| e.to.as_str().contains(":helper:")),
        "Should resolve aliased import dbHelper → helper, got: {:?}",
        calls
    );
}

#[test]
fn test_resolve_cross_file_glob_import() {
    use crate::a6s::types::{ImportEntry, RawImport};

    let ext = TypeScriptExtractor;

    // File 1: calls helper(), has namespace import from src/utils
    let code1 = r#"
import * as utils from '../utils';
export function handler() { helper(); }
"#;
    let mut file1 = ext.extract(code1, "src/api/handler.ts");
    file1.imports.push(RawImport {
        file_path: "src/api/handler.ts".to_string(),
        entry: ImportEntry::glob_import("src/utils"),
    });

    // File 2: defines helper()
    let code2 = r#"export function helper() {}"#;
    let file2 = ext.extract(code2, "src/utils.ts");

    // File 3: ALSO defines helper()
    let code3 = r#"export function helper() {}"#;
    let file3 = ext.extract(code3, "src/core/helpers.ts");

    let mut files = [file1, file2, file3];
    let (resolved, _) = ext.resolve_cross_file(&mut files);

    let calls: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .collect();
    assert!(
        calls.iter().any(|e| e.to.as_str().contains(":helper:")),
        "Should resolve via glob import despite ambiguity, got: {:?}",
        calls
    );
}

#[test]
fn test_resolve_cross_file_returns_resolved_imports() {
    use crate::a6s::types::{ImportEntry, RawImport};

    let ext = TypeScriptExtractor;

    // File 1: has import for helper
    let code1 = r#"
import { helper } from '../utils';
export function handler() {}
"#;
    let mut file1 = ext.extract(code1, "src/api/handler.ts");
    file1.imports.push(RawImport {
        file_path: "src/api/handler.ts".to_string(),
        entry: ImportEntry::named_import("src/utils", vec!["helper".to_string()]),
    });

    // File 2: defines helper()
    let code2 = r#"export function helper() {}"#;
    let file2 = ext.extract(code2, "src/utils.ts");

    let mut files = [file1, file2];
    let (_resolved, imports) = ext.resolve_cross_file(&mut files);

    assert!(
        !imports.is_empty(),
        "Should return resolved imports, got empty"
    );
}

#[test]
fn test_resolve_cross_file_import_priority_over_bare() {
    use crate::a6s::types::{ImportEntry, RawImport};

    let ext = TypeScriptExtractor;

    // File 1: calls run(), has import from src/module_a
    let code1 = r#"
import { run } from '../module_a/runner';
export function main() { run(); }
"#;
    let mut file1 = ext.extract(code1, "src/app/main.ts");
    file1.imports.push(RawImport {
        file_path: "src/app/main.ts".to_string(),
        entry: ImportEntry::named_import("src/module_a/runner", vec!["run".to_string()]),
    });

    // File 2: defines run()
    let code2 = r#"export function run() {}"#;
    let file2 = ext.extract(code2, "src/module_a/runner.ts");

    // File 3: ALSO defines run()
    let code3 = r#"export function run() {}"#;
    let file3 = ext.extract(code3, "src/module_b/runner.ts");

    // File 4: ALSO defines run()
    let code4 = r#"export function run() {}"#;
    let file4 = ext.extract(code4, "src/module_c/runner.ts");

    let mut files = [file1, file2, file3, file4];
    let (resolved, _) = ext.resolve_cross_file(&mut files);

    let calls: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .collect();
    assert!(
        calls.iter().any(|e| e.to.as_str().contains(":run:")),
        "Should resolve via import despite 3 bare candidates, got: {:?}",
        calls
    );
    assert!(
        calls.iter().any(|e| e.to.as_str().contains("module_a")),
        "Should resolve to module_a's run specifically, got: {:?}",
        calls
    );
}
