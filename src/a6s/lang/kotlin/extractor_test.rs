use crate::a6s::extract::LanguageExtractor;
use crate::a6s::lang::kotlin::extractor::KotlinExtractor;

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/a6s/lang/kotlin/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

#[test]
fn test_language() {
    let extractor = KotlinExtractor;
    assert_eq!(extractor.language(), "kotlin");
}

#[test]
fn test_extensions() {
    let extractor = KotlinExtractor;
    assert_eq!(extractor.extensions(), &["kt", "kts"]);
}

#[test]
fn test_grammar_loads() {
    let extractor = KotlinExtractor;
    let language = extractor.grammar();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .expect("Failed to set Kotlin language");

    let tree = parser
        .parse("fun main() {}", None)
        .expect("Failed to parse simple Kotlin code");
    assert!(tree.root_node().child_count() > 0);
}

#[test]
fn test_queries_parse() {
    let extractor = KotlinExtractor;
    let language = extractor.grammar();

    let sq = extractor.symbol_queries();
    if !sq.is_empty() {
        tree_sitter::Query::new(&language, sq).expect("symbol_queries should parse");
    }

    let tq = extractor.type_ref_queries();
    if !tq.is_empty() {
        tree_sitter::Query::new(&language, tq).expect("type_ref_queries should parse");
    }
}

#[test]
fn test_discoverable_via_extractor() {
    use crate::a6s::extract::Extractor;

    let ext = Extractor::for_language("kotlin");
    assert!(ext.is_some());
    assert_eq!(ext.unwrap().language(), "kotlin");

    let ext_kt = Extractor::for_extension("kt");
    assert!(ext_kt.is_some());
    assert_eq!(ext_kt.unwrap().language(), "kotlin");

    let ext_kts = Extractor::for_extension("kts");
    assert!(ext_kts.is_some());
    assert_eq!(ext_kts.unwrap().language(), "kotlin");
}

// ============================================================================
// Phase 2: Symbol Extraction Tests
// ============================================================================

// --- Subtask 1: extract() core structure ---

#[test]
fn test_extract_basic_class() {
    let extractor = KotlinExtractor;
    let code = load_testdata("symbols.kt");
    let parsed = extractor.extract(&code, "symbols.kt");

    assert_eq!(parsed.language, "kotlin");
    assert_eq!(parsed.file_path, "symbols.kt");

    // Should find MyClass as a class
    let my_class = parsed
        .symbols
        .iter()
        .find(|s| s.name == "MyClass" && s.kind == "class");
    assert!(
        my_class.is_some(),
        "Should extract MyClass as class, got symbols: {:?}",
        parsed
            .symbols
            .iter()
            .map(|s| (&s.name, &s.kind))
            .collect::<Vec<_>>()
    );
    let my_class = my_class.unwrap();
    assert_eq!(my_class.language, "kotlin");
    assert!(my_class.start_line > 0);
    assert!(my_class.end_line >= my_class.start_line);
}

#[test]
fn test_extract_parse_failure() {
    let extractor = KotlinExtractor;
    // Severely broken code — parser should still return something
    let parsed = extractor.extract("{{{{", "broken.kt");
    assert_eq!(parsed.language, "kotlin");
    // Should not panic, may return empty or partial
}

// --- Subtask 2: symbol_queries() ---

#[test]
fn test_extract_classes() {
    let extractor = KotlinExtractor;
    let code = load_testdata("symbols.kt");
    let parsed = extractor.extract(&code, "symbols.kt");

    // Regular class
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "MyClass" && s.kind == "class"),
        "Should extract MyClass"
    );
    // Data class
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Point" && s.kind == "class"),
        "Should extract data class Point"
    );
    // Sealed class
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Result" && s.kind == "class"),
        "Should extract sealed class Result"
    );
    // Abstract class
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Base" && s.kind == "class"),
        "Should extract abstract class Base"
    );
    // Outer class
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Outer" && s.kind == "class"),
        "Should extract Outer"
    );
    // Inner class
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Inner" && s.kind == "class"),
        "Should extract inner class Inner"
    );
    // Factory class (has companion object)
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Factory" && s.kind == "class"),
        "Should extract Factory"
    );
}

#[test]
fn test_extract_interfaces() {
    let extractor = KotlinExtractor;
    let code = load_testdata("symbols.kt");
    let parsed = extractor.extract(&code, "symbols.kt");

    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Repository" && s.kind == "interface"),
        "Should extract interface Repository"
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Predicate" && s.kind == "interface"),
        "Should extract fun interface Predicate"
    );
}

#[test]
fn test_extract_functions() {
    let extractor = KotlinExtractor;
    let code = load_testdata("symbols.kt");
    let parsed = extractor.extract(&code, "symbols.kt");

    // Top-level function (suspend)
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "fetchData" && s.kind == "function"),
        "Should extract suspend function fetchData"
    );

    // Extension function → extension_function
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "wordCount" && s.kind == "extension_function"),
        "Should extract extension function wordCount as extension_function"
    );

    // Operator extension function → extension_function
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "plus" && s.kind == "extension_function"),
        "Should extract operator extension function plus as extension_function"
    );

    // Member function → method
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "doSomething" && s.kind == "method"),
        "Should extract member function doSomething as method"
    );
}

#[test]
fn test_extract_objects() {
    let extractor = KotlinExtractor;
    let code = load_testdata("symbols.kt");
    let parsed = extractor.extract(&code, "symbols.kt");

    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "AppConfig" && s.kind == "object"),
        "Should extract object AppConfig as object"
    );
}

#[test]
fn test_extract_enum() {
    let extractor = KotlinExtractor;
    let code = load_testdata("symbols.kt");
    let parsed = extractor.extract(&code, "symbols.kt");

    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Color" && s.kind == "enum"),
        "Should extract enum class Color"
    );
}

#[test]
fn test_extract_type_alias() {
    let extractor = KotlinExtractor;
    let code = load_testdata("symbols.kt");
    let parsed = extractor.extract(&code, "symbols.kt");

    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "StringMap" && s.kind == "type_alias"),
        "Should extract typealias StringMap"
    );
}

#[test]
fn test_extract_properties() {
    let extractor = KotlinExtractor;
    let code = load_testdata("symbols.kt");
    let parsed = extractor.extract(&code, "symbols.kt");

    // Top-level val → const
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "DEFAULT_NAME" && s.kind == "const"),
        "Should extract top-level val DEFAULT_NAME as const"
    );

    // Top-level var → var
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "globalCounter" && s.kind == "var"),
        "Should extract top-level var globalCounter"
    );

    // const val inside object → const
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "MAX_SIZE" && s.kind == "const"),
        "Should extract const val MAX_SIZE"
    );

    // Class member property → field
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "name" && s.kind == "property"),
        "Should extract class property name as property"
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "count" && s.kind == "property"),
        "Should extract class property count as property"
    );
}

// --- Subtask 3: process_match() — kind detection ---

#[test]
fn test_extract_data_class_kind() {
    let extractor = KotlinExtractor;
    let code = "data class Point(val x: Int, val y: Int)";
    let parsed = extractor.extract(code, "test.kt");

    let point = parsed.symbols.iter().find(|s| s.name == "Point");
    assert!(point.is_some(), "Should extract Point");
    assert_eq!(point.unwrap().kind, "class", "data class should be class");
}

#[test]
fn test_extract_interface_kind() {
    let extractor = KotlinExtractor;
    let code = "interface Foo { fun bar() }";
    let parsed = extractor.extract(code, "test.kt");

    let foo = parsed.symbols.iter().find(|s| s.name == "Foo");
    assert!(foo.is_some(), "Should extract Foo");
    assert_eq!(foo.unwrap().kind, "interface");
}

#[test]
fn test_extract_enum_kind() {
    let extractor = KotlinExtractor;
    let code = "enum class Direction { NORTH, SOUTH, EAST, WEST }";
    let parsed = extractor.extract(code, "test.kt");

    let dir = parsed.symbols.iter().find(|s| s.name == "Direction");
    assert!(dir.is_some(), "Should extract Direction");
    assert_eq!(dir.unwrap().kind, "enum");
}

#[test]
fn test_extract_function_kind() {
    let extractor = KotlinExtractor;
    let code = "fun hello() { }";
    let parsed = extractor.extract(code, "test.kt");

    let hello = parsed.symbols.iter().find(|s| s.name == "hello");
    assert!(hello.is_some(), "Should extract hello");
    assert_eq!(hello.unwrap().kind, "function");
}

#[test]
fn test_extract_property_kind() {
    let extractor = KotlinExtractor;
    let code = "val PI: Double = 3.14\nvar counter: Int = 0";
    let parsed = extractor.extract(code, "test.kt");

    let pi = parsed.symbols.iter().find(|s| s.name == "PI");
    assert!(pi.is_some(), "Should extract PI");
    assert_eq!(pi.unwrap().kind, "const", "top-level val should be const");

    let counter = parsed.symbols.iter().find(|s| s.name == "counter");
    assert!(counter.is_some(), "Should extract counter");
    assert_eq!(counter.unwrap().kind, "var", "top-level var should be var");
}

// --- Subtask 4: type_ref_queries ---

#[test]
fn test_param_type_extraction() {
    let extractor = KotlinExtractor;
    let code = "class User(val name: String)\nfun greet(user: User, config: AppConfig): Unit { }";
    let parsed = extractor.extract(code, "test.kt");

    // Should have ParamType edges
    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();
    assert!(
        !param_edges.is_empty(),
        "Should extract parameter type references"
    );
}

#[test]
fn test_return_type_extraction() {
    let extractor = KotlinExtractor;
    let code = "class Result\nfun compute(): Result { return Result() }";
    let parsed = extractor.extract(code, "test.kt");

    let return_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ReturnType)
        .collect();
    assert!(
        !return_edges.is_empty(),
        "Should extract return type reference"
    );
}

#[test]
fn test_property_type_extraction() {
    let extractor = KotlinExtractor;
    let code = "class Config\nval cfg: Config = Config()";
    let parsed = extractor.extract(code, "test.kt");

    let field_type_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::FieldType)
        .collect();
    assert!(
        !field_type_edges.is_empty(),
        "Should extract property type reference"
    );
}

// --- Subtask 5: derive_module_path ---

#[test]
fn test_derive_module_path_simple() {
    let extractor = KotlinExtractor;
    let code = "package com.example\nclass Foo";
    let parsed = extractor.extract(code, "Foo.kt");

    let foo = parsed.symbols.iter().find(|s| s.name == "Foo").unwrap();
    assert_eq!(
        foo.module_path.as_deref(),
        Some("com::example"),
        "Should derive module path from package header"
    );
}

#[test]
fn test_derive_module_path_no_package() {
    let extractor = KotlinExtractor;
    let code = "class Foo";
    let parsed = extractor.extract(code, "Foo.kt");

    let foo = parsed.symbols.iter().find(|s| s.name == "Foo").unwrap();
    // No package → empty or None module_path
    assert!(
        foo.module_path.is_none() || foo.module_path.as_deref() == Some(""),
        "No package should yield empty module_path, got: {:?}",
        foo.module_path
    );
}

#[test]
fn test_derive_module_path_kts() {
    let extractor = KotlinExtractor;
    let code = "package com.example.scripts\nval x = 1";
    let parsed = extractor.extract(code, "build.gradle.kts");

    let x = parsed.symbols.iter().find(|s| s.name == "x").unwrap();
    assert_eq!(
        x.module_path.as_deref(),
        Some("com::example::scripts"),
        ".kts files should also derive module path from package header"
    );
}

// --- Visibility tests ---

#[test]
fn test_visibility_default_public() {
    let extractor = KotlinExtractor;
    let code = "class Foo";
    let parsed = extractor.extract(code, "test.kt");

    let foo = parsed.symbols.iter().find(|s| s.name == "Foo").unwrap();
    assert_eq!(foo.visibility, Some("pub".to_string()));
}

#[test]
fn test_visibility_private() {
    let extractor = KotlinExtractor;
    let code = "private class Foo";
    let parsed = extractor.extract(code, "test.kt");

    let foo = parsed.symbols.iter().find(|s| s.name == "Foo").unwrap();
    assert_eq!(foo.visibility, Some("private".to_string()));
}

#[test]
fn test_visibility_internal() {
    let extractor = KotlinExtractor;
    let code = "internal fun helper(): Int = 0";
    let parsed = extractor.extract(code, "test.kt");

    let helper = parsed.symbols.iter().find(|s| s.name == "helper").unwrap();
    assert_eq!(helper.visibility, Some("internal".to_string()));
}

// --- Phase 3: Visibility tests from testdata ---

#[test]
fn test_visibility_public_explicit() {
    let extractor = KotlinExtractor;
    let code = load_testdata("visibility.kt");
    let parsed = extractor.extract(&code, "visibility.kt");

    let cls = parsed
        .symbols
        .iter()
        .find(|s| s.name == "PublicClass")
        .unwrap();
    assert_eq!(
        cls.visibility,
        Some("pub".to_string()),
        "explicit public should map to pub"
    );

    let method = parsed
        .symbols
        .iter()
        .find(|s| s.name == "publicMethod")
        .unwrap();
    assert_eq!(method.visibility, Some("pub".to_string()));

    let prop = parsed
        .symbols
        .iter()
        .find(|s| s.name == "publicProperty")
        .unwrap();
    assert_eq!(prop.visibility, Some("pub".to_string()));
}

#[test]
fn test_visibility_protected() {
    let extractor = KotlinExtractor;
    let code = load_testdata("visibility.kt");
    let parsed = extractor.extract(&code, "visibility.kt");

    let method = parsed
        .symbols
        .iter()
        .find(|s| s.name == "protectedMethod")
        .unwrap();
    assert_eq!(
        method.visibility,
        Some("protected".to_string()),
        "protected should map to protected"
    );

    let prop = parsed
        .symbols
        .iter()
        .find(|s| s.name == "protectedProperty")
        .unwrap();
    assert_eq!(prop.visibility, Some("protected".to_string()));
}

#[test]
fn test_visibility_private_from_testdata() {
    let extractor = KotlinExtractor;
    let code = load_testdata("visibility.kt");
    let parsed = extractor.extract(&code, "visibility.kt");

    let cls = parsed
        .symbols
        .iter()
        .find(|s| s.name == "PrivateClass")
        .unwrap();
    assert_eq!(cls.visibility, Some("private".to_string()));

    let method = parsed
        .symbols
        .iter()
        .find(|s| s.name == "privateMethod")
        .unwrap();
    assert_eq!(method.visibility, Some("private".to_string()));

    let prop = parsed
        .symbols
        .iter()
        .find(|s| s.name == "privateProperty")
        .unwrap();
    assert_eq!(prop.visibility, Some("private".to_string()));
}

#[test]
fn test_visibility_internal_from_testdata() {
    let extractor = KotlinExtractor;
    let code = load_testdata("visibility.kt");
    let parsed = extractor.extract(&code, "visibility.kt");

    let cls = parsed
        .symbols
        .iter()
        .find(|s| s.name == "InternalClass")
        .unwrap();
    assert_eq!(cls.visibility, Some("internal".to_string()));

    let method = parsed
        .symbols
        .iter()
        .find(|s| s.name == "internalMethod")
        .unwrap();
    assert_eq!(method.visibility, Some("internal".to_string()));

    let prop = parsed
        .symbols
        .iter()
        .find(|s| s.name == "internalProperty")
        .unwrap();
    assert_eq!(prop.visibility, Some("internal".to_string()));
}

#[test]
fn test_default_visibility_is_public() {
    let extractor = KotlinExtractor;
    let code = load_testdata("visibility.kt");
    let parsed = extractor.extract(&code, "visibility.kt");

    let cls = parsed
        .symbols
        .iter()
        .find(|s| s.name == "DefaultClass")
        .unwrap();
    assert_eq!(
        cls.visibility,
        Some("pub".to_string()),
        "default visibility should be pub"
    );

    let method = parsed
        .symbols
        .iter()
        .find(|s| s.name == "defaultMethod")
        .unwrap();
    assert_eq!(method.visibility, Some("pub".to_string()));

    let prop = parsed
        .symbols
        .iter()
        .find(|s| s.name == "defaultProperty")
        .unwrap();
    assert_eq!(prop.visibility, Some("pub".to_string()));
}

#[test]
fn test_visibility_mixed_class() {
    let extractor = KotlinExtractor;
    let code = load_testdata("visibility.kt");
    let parsed = extractor.extract(&code, "visibility.kt");

    let pub_fn = parsed.symbols.iter().find(|s| s.name == "pubFun").unwrap();
    assert_eq!(pub_fn.visibility, Some("pub".to_string()));

    let priv_fn = parsed.symbols.iter().find(|s| s.name == "privFun").unwrap();
    assert_eq!(priv_fn.visibility, Some("private".to_string()));

    let prot_fn = parsed.symbols.iter().find(|s| s.name == "protFun").unwrap();
    assert_eq!(prot_fn.visibility, Some("protected".to_string()));

    let int_fn = parsed.symbols.iter().find(|s| s.name == "intFun").unwrap();
    assert_eq!(int_fn.visibility, Some("internal".to_string()));

    let def_fn = parsed
        .symbols
        .iter()
        .find(|s| s.name == "defaultFun")
        .unwrap();
    assert_eq!(def_fn.visibility, Some("pub".to_string()));
}

#[test]
fn test_visibility_top_level_functions() {
    let extractor = KotlinExtractor;
    let code = load_testdata("visibility.kt");
    let parsed = extractor.extract(&code, "visibility.kt");

    let pub_fn = parsed
        .symbols
        .iter()
        .find(|s| s.name == "publicTopFun")
        .unwrap();
    assert_eq!(pub_fn.visibility, Some("pub".to_string()));

    let priv_fn = parsed
        .symbols
        .iter()
        .find(|s| s.name == "privateTopFun")
        .unwrap();
    assert_eq!(priv_fn.visibility, Some("private".to_string()));

    let int_fn = parsed
        .symbols
        .iter()
        .find(|s| s.name == "internalTopFun")
        .unwrap();
    assert_eq!(int_fn.visibility, Some("internal".to_string()));

    let def_fn = parsed
        .symbols
        .iter()
        .find(|s| s.name == "defaultTopFun")
        .unwrap();
    assert_eq!(def_fn.visibility, Some("pub".to_string()));
}

// --- Phase 3: Modifier signature tests ---

#[test]
fn test_modifier_data_class() {
    let extractor = KotlinExtractor;
    let code = "data class Point(val x: Int, val y: Int)";
    let parsed = extractor.extract(code, "test.kt");

    let point = parsed.symbols.iter().find(|s| s.name == "Point").unwrap();
    let sig = point.signature.as_deref().unwrap();
    assert!(
        sig.contains("data"),
        "data class signature should contain 'data', got: {}",
        sig
    );
}

#[test]
fn test_modifier_sealed_class() {
    let extractor = KotlinExtractor;
    let code = "sealed class Result";
    let parsed = extractor.extract(code, "test.kt");

    let result = parsed.symbols.iter().find(|s| s.name == "Result").unwrap();
    let sig = result.signature.as_deref().unwrap();
    assert!(
        sig.contains("sealed"),
        "sealed class signature should contain 'sealed', got: {}",
        sig
    );
}

#[test]
fn test_modifier_abstract_class() {
    let extractor = KotlinExtractor;
    let code = "abstract class Base";
    let parsed = extractor.extract(code, "test.kt");

    let base = parsed.symbols.iter().find(|s| s.name == "Base").unwrap();
    let sig = base.signature.as_deref().unwrap();
    assert!(
        sig.contains("abstract"),
        "abstract class signature should contain 'abstract', got: {}",
        sig
    );
}

#[test]
fn test_modifier_open_class() {
    let extractor = KotlinExtractor;
    let code = "open class Base";
    let parsed = extractor.extract(code, "test.kt");

    let base = parsed.symbols.iter().find(|s| s.name == "Base").unwrap();
    let sig = base.signature.as_deref().unwrap();
    assert!(
        sig.contains("open"),
        "open class signature should contain 'open', got: {}",
        sig
    );
}

#[test]
fn test_modifier_annotation_class() {
    let extractor = KotlinExtractor;
    let code = "annotation class MyAnnotation";
    let parsed = extractor.extract(code, "test.kt");

    let ann = parsed
        .symbols
        .iter()
        .find(|s| s.name == "MyAnnotation")
        .unwrap();
    let sig = ann.signature.as_deref().unwrap();
    assert!(
        sig.contains("annotation"),
        "annotation class signature should contain 'annotation', got: {}",
        sig
    );
}

#[test]
fn test_modifier_suspend_function() {
    let extractor = KotlinExtractor;
    let code = "suspend fun fetchData(): String = \"\"";
    let parsed = extractor.extract(code, "test.kt");

    let func = parsed
        .symbols
        .iter()
        .find(|s| s.name == "fetchData")
        .unwrap();
    let sig = func.signature.as_deref().unwrap();
    assert!(
        sig.contains("suspend"),
        "suspend function signature should contain 'suspend', got: {}",
        sig
    );
}

#[test]
fn test_modifier_inline_function() {
    let extractor = KotlinExtractor;
    let code = "inline fun <reified T> check(): Boolean = true";
    let parsed = extractor.extract(code, "test.kt");

    let func = parsed.symbols.iter().find(|s| s.name == "check").unwrap();
    let sig = func.signature.as_deref().unwrap();
    assert!(
        sig.contains("inline"),
        "inline function signature should contain 'inline', got: {}",
        sig
    );
}

#[test]
fn test_modifier_infix_function() {
    let extractor = KotlinExtractor;
    let code = "infix fun Int.shl(x: Int): Int = this";
    let parsed = extractor.extract(code, "test.kt");

    let func = parsed.symbols.iter().find(|s| s.name == "shl").unwrap();
    let sig = func.signature.as_deref().unwrap();
    assert!(
        sig.contains("infix"),
        "infix function signature should contain 'infix', got: {}",
        sig
    );
}

#[test]
fn test_modifier_tailrec_function() {
    let extractor = KotlinExtractor;
    let code = "tailrec fun factorial(n: Int, acc: Int): Int = if (n <= 1) acc else factorial(n - 1, n * acc)";
    let parsed = extractor.extract(code, "test.kt");

    let func = parsed
        .symbols
        .iter()
        .find(|s| s.name == "factorial")
        .unwrap();
    let sig = func.signature.as_deref().unwrap();
    assert!(
        sig.contains("tailrec"),
        "tailrec function signature should contain 'tailrec', got: {}",
        sig
    );
}

#[test]
fn test_modifier_operator_function() {
    let extractor = KotlinExtractor;
    let code =
        "data class Vec(val x: Int)\noperator fun Vec.plus(other: Vec): Vec = Vec(x + other.x)";
    let parsed = extractor.extract(code, "test.kt");

    let func = parsed.symbols.iter().find(|s| s.name == "plus").unwrap();
    let sig = func.signature.as_deref().unwrap();
    assert!(
        sig.contains("operator"),
        "operator function signature should contain 'operator', got: {}",
        sig
    );
}

#[test]
fn test_modifier_const_property() {
    let extractor = KotlinExtractor;
    let code = "object Config {\n    const val MAX: Int = 100\n}";
    let parsed = extractor.extract(code, "test.kt");

    let prop = parsed.symbols.iter().find(|s| s.name == "MAX").unwrap();
    let sig = prop.signature.as_deref().unwrap();
    assert!(
        sig.contains("const"),
        "const property signature should contain 'const', got: {}",
        sig
    );
}

#[test]
fn test_modifier_lateinit_property() {
    let extractor = KotlinExtractor;
    let code = "class Foo {\n    lateinit var name: String\n}";
    let parsed = extractor.extract(code, "test.kt");

    let prop = parsed.symbols.iter().find(|s| s.name == "name").unwrap();
    let sig = prop.signature.as_deref().unwrap();
    assert!(
        sig.contains("lateinit"),
        "lateinit property signature should contain 'lateinit', got: {}",
        sig
    );
}

#[test]
fn test_modifier_override_property() {
    let extractor = KotlinExtractor;
    let code = "open class Base { open val x: Int = 0 }\nclass Derived : Base() {\n    override val x: Int = 1\n}";
    let parsed = extractor.extract(code, "test.kt");

    let props: Vec<_> = parsed.symbols.iter().filter(|s| s.name == "x").collect();
    // Find the override one (in Derived, higher start_line)
    let override_prop = props.iter().max_by_key(|s| s.start_line).unwrap();
    let sig = override_prop.signature.as_deref().unwrap();
    assert!(
        sig.contains("override"),
        "override property signature should contain 'override', got: {}",
        sig
    );
}

// --- Member function detection ---

#[test]
fn test_member_function_is_method() {
    let extractor = KotlinExtractor;
    let code = "class Foo {\n    fun bar() { }\n}";
    let parsed = extractor.extract(code, "test.kt");

    let bar = parsed.symbols.iter().find(|s| s.name == "bar").unwrap();
    assert_eq!(
        bar.kind, "method",
        "function inside class body should be method"
    );
}

#[test]
fn test_interface_method() {
    let extractor = KotlinExtractor;
    let code = "interface Foo {\n    fun bar()\n}";
    let parsed = extractor.extract(code, "test.kt");

    let bar = parsed.symbols.iter().find(|s| s.name == "bar").unwrap();
    assert_eq!(
        bar.kind, "interface_method",
        "function inside interface should be interface_method"
    );
}

// ============================================================================
// Phase 4: Import Extraction Tests
// ============================================================================

#[test]
fn test_extract_single_import() {
    let extractor = KotlinExtractor;
    let code = load_testdata("imports.kt");
    let parsed = extractor.extract(&code, "imports.kt");

    // Should have a single import for java.util.List
    let list_import = parsed
        .imports
        .iter()
        .find(|i| i.entry.module_path == "java::util" && i.entry.imported_names == vec!["List"]);
    assert!(
        list_import.is_some(),
        "Should extract single import java.util.List, got imports: {:?}",
        parsed.imports
    );
    assert!(!list_import.unwrap().entry.is_glob);
    assert!(list_import.unwrap().entry.alias.is_none());
}

#[test]
fn test_extract_wildcard_import() {
    let extractor = KotlinExtractor;
    let code = load_testdata("imports.kt");
    let parsed = extractor.extract(&code, "imports.kt");

    let wildcard = parsed
        .imports
        .iter()
        .find(|i| i.entry.module_path == "com::example::utils" && i.entry.is_glob);
    assert!(
        wildcard.is_some(),
        "Should extract wildcard import com.example.utils.*, got imports: {:?}",
        parsed.imports
    );
    assert!(wildcard.unwrap().entry.imported_names.is_empty());
}

#[test]
fn test_extract_aliased_import() {
    let extractor = KotlinExtractor;
    let code = load_testdata("imports.kt");
    let parsed = extractor.extract(&code, "imports.kt");

    let aliased = parsed.imports.iter().find(|i| {
        i.entry.imported_names == vec!["User"] && i.entry.alias == Some("AppUser".to_string())
    });
    assert!(
        aliased.is_some(),
        "Should extract aliased import User as AppUser, got imports: {:?}",
        parsed.imports
    );
    assert_eq!(aliased.unwrap().entry.module_path, "com::example::models");
}

#[test]
fn test_multiple_imports() {
    let extractor = KotlinExtractor;
    let code = load_testdata("imports.kt");
    let parsed = extractor.extract(&code, "imports.kt");

    // imports.kt has: 3 single + 2 wildcard + 2 aliased + 2 nested = 9 import_headers
    // But aliased ones duplicate the single ones (same identifier path, different match)
    // Actually: java.util.List, java.io.File, com.example.models.User,
    //           com.example.utils.*, kotlinx.coroutines.*,
    //           com.example.models.User as AppUser, java.io.File as IoFile,
    //           com.very.deep.nested.package.Helper, org.jetbrains.annotations.NotNull
    // = 9 import headers total
    assert!(
        parsed.imports.len() >= 9,
        "Should extract at least 9 imports, got {}",
        parsed.imports.len()
    );
}

#[test]
fn test_import_module_path_mapping() {
    let extractor = KotlinExtractor;
    let code = load_testdata("imports.kt");
    let parsed = extractor.extract(&code, "imports.kt");

    // Deeply nested: com.very.deep.nested.package.Helper
    let deep = parsed
        .imports
        .iter()
        .find(|i| i.entry.imported_names == vec!["Helper"]);
    assert!(
        deep.is_some(),
        "Should extract deeply nested import, got imports: {:?}",
        parsed.imports
    );
    assert_eq!(
        deep.unwrap().entry.module_path,
        "com::very::deep::nested::package",
        "Dotted path should map to :: separator"
    );
}

// ============================================================================
// Phase 5: Single-File Edge Extraction Tests
// ============================================================================

// --- Subtask 1: HasField edges ---

#[test]
fn test_hasfield_edges_val() {
    let extractor = KotlinExtractor;
    let code = load_testdata("edges.kt");
    let parsed = extractor.extract(&code, "edges.kt");

    let hasfield_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasField)
        .collect();

    // User class has 3 properties: name, age, id
    let user_to_name = hasfield_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("User"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("name"))
    });
    assert!(user_to_name, "Should have HasField edge from User to name");
}

#[test]
fn test_hasfield_edges_var() {
    let extractor = KotlinExtractor;
    let code = load_testdata("edges.kt");
    let parsed = extractor.extract(&code, "edges.kt");

    let hasfield_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasField)
        .collect();

    let user_to_age = hasfield_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("User"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("age"))
    });
    assert!(
        user_to_age,
        "Should have HasField edge from User to age (var)"
    );
}

#[test]
fn test_hasfield_edges_constructor_param() {
    let extractor = KotlinExtractor;
    let code = load_testdata("edges.kt");
    let parsed = extractor.extract(&code, "edges.kt");

    let hasfield_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasField)
        .collect();

    // Point data class has x and y from constructor
    let point_to_x = hasfield_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("Point"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":x:"))
    });
    assert!(
        point_to_x,
        "Should have HasField edge from Point to x (constructor param), got edges: {:?}",
        hasfield_edges
    );
}

// --- Subtask 2: HasMethod edges ---

#[test]
fn test_hasmethod_edges_regular() {
    let extractor = KotlinExtractor;
    let code = load_testdata("edges.kt");
    let parsed = extractor.extract(&code, "edges.kt");

    let hasmethod_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasMethod)
        .collect();

    let service_to_start = hasmethod_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("Service"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("start"))
    });
    assert!(
        service_to_start,
        "Should have HasMethod edge from Service to start, got edges: {:?}",
        hasmethod_edges
    );
}

#[test]
fn test_hasmethod_edges_interface() {
    let extractor = KotlinExtractor;
    let code = load_testdata("edges.kt");
    let parsed = extractor.extract(&code, "edges.kt");

    let hasmethod_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasMethod)
        .collect();

    let repo_to_findall = hasmethod_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("Repository"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("findAll"))
    });
    assert!(
        repo_to_findall,
        "Should have HasMethod edge from Repository to findAll, got edges: {:?}",
        hasmethod_edges
    );
}

#[test]
fn test_hasmethod_edges_object() {
    let extractor = KotlinExtractor;
    let code = load_testdata("edges.kt");
    let parsed = extractor.extract(&code, "edges.kt");

    let hasmethod_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasMethod)
        .collect();

    let singleton_to_getinstance = hasmethod_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("Singleton"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("getInstance"))
    });
    assert!(
        singleton_to_getinstance,
        "Should have HasMethod edge from Singleton to getInstance, got edges: {:?}",
        hasmethod_edges
    );
}

// --- Subtask 3: HasMember edges ---

#[test]
fn test_hasmember_top_level_function() {
    let extractor = KotlinExtractor;
    let code = load_testdata("edges.kt");
    let parsed = extractor.extract(&code, "edges.kt");

    let hasmember_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasMember)
        .collect();

    let module_to_toplevel = hasmember_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("topLevelFunction"))
    });
    assert!(
        module_to_toplevel,
        "Should have HasMember edge to topLevelFunction, got edges: {:?}",
        hasmember_edges
    );
}

#[test]
fn test_hasmember_top_level_class() {
    let extractor = KotlinExtractor;
    let code = load_testdata("edges.kt");
    let parsed = extractor.extract(&code, "edges.kt");

    let hasmember_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasMember)
        .collect();

    let module_to_class = hasmember_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("TopLevelClass"))
    });
    assert!(
        module_to_class,
        "Should have HasMember edge to TopLevelClass, got edges: {:?}",
        hasmember_edges
    );
}

// --- Subtask 4: Inheritance edges ---

#[test]
fn test_extends_edge_simple() {
    let extractor = KotlinExtractor;
    let code = load_testdata("inheritance.kt");
    let parsed = extractor.extract(&code, "inheritance.kt");

    let extends_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Extends)
        .collect();

    let dog_extends_animal = extends_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("Dog"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Animal")
    });
    assert!(
        dog_extends_animal,
        "Should have Extends edge from Dog to Animal, got edges: {:?}",
        extends_edges
    );
}

#[test]
fn test_implements_edge_single() {
    let extractor = KotlinExtractor;
    let code = load_testdata("inheritance.kt");
    let parsed = extractor.extract(&code, "inheritance.kt");

    let impl_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Implements)
        .collect();

    let task_impl_runnable = impl_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("Task"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Runnable")
    });
    assert!(
        task_impl_runnable,
        "Should have Implements edge from Task to Runnable, got edges: {:?}",
        impl_edges
    );
}

#[test]
fn test_implements_edge_multiple() {
    let extractor = KotlinExtractor;
    let code = load_testdata("inheritance.kt");
    let parsed = extractor.extract(&code, "inheritance.kt");

    let impl_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Implements)
        .collect();

    let worker_impl_runnable = impl_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("Worker"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Runnable")
    });
    let worker_impl_loggable = impl_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("Worker"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Loggable")
    });
    assert!(
        worker_impl_runnable,
        "Worker should implement Runnable, got edges: {:?}",
        impl_edges
    );
    assert!(
        worker_impl_loggable,
        "Worker should implement Loggable, got edges: {:?}",
        impl_edges
    );
}

// --- Subtask 5: Calls edges ---

#[test]
fn test_calls_edge_simple() {
    let extractor = KotlinExtractor;
    let code = load_testdata("functions.kt");
    let parsed = extractor.extract(&code, "functions.kt");

    let calls_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Calls)
        .collect();

    let caller_calls_greet = calls_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("caller"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "greet")
    });
    assert!(
        caller_calls_greet,
        "Should have Calls edge from caller to greet, got edges: {:?}",
        calls_edges
    );
}

#[test]
fn test_calls_edge_method() {
    let extractor = KotlinExtractor;
    let code = load_testdata("functions.kt");
    let parsed = extractor.extract(&code, "functions.kt");

    let calls_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Calls)
        .collect();

    let caller_calls_assist = calls_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("caller"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "assist")
    });
    assert!(
        caller_calls_assist,
        "Should have Calls edge from caller to assist (method call), got edges: {:?}",
        calls_edges
    );
}

#[test]
fn test_calls_edge_constructor() {
    let extractor = KotlinExtractor;
    let code = load_testdata("functions.kt");
    let parsed = extractor.extract(&code, "functions.kt");

    let calls_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Calls)
        .collect();

    let caller_calls_helper = calls_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("caller"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Helper")
    });
    assert!(
        caller_calls_helper,
        "Should have Calls edge from caller to Helper (constructor), got edges: {:?}",
        calls_edges
    );
}

// ============================================================================
// Phase 6: Type Reference Extraction Tests
// ============================================================================

// --- Subtask 1: extract_type_references() ---

#[test]
fn test_param_type_ref() {
    let extractor = KotlinExtractor;
    let code = load_testdata("type_refs.kt");
    let parsed = extractor.extract(&code, "type_refs.kt");

    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();

    // process(user: UserProfile, config: Config)
    let has_userprofile = param_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "UserProfile")
    });
    assert!(
        has_userprofile,
        "Should have ParamType edge for UserProfile parameter, got edges: {:?}",
        param_edges
    );

    let has_config = param_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Config")
    });
    assert!(
        has_config,
        "Should have ParamType edge for Config parameter, got edges: {:?}",
        param_edges
    );
}

#[test]
fn test_return_type_ref() {
    let extractor = KotlinExtractor;
    let code = load_testdata("type_refs.kt");
    let parsed = extractor.extract(&code, "type_refs.kt");

    let return_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ReturnType)
        .collect();

    let has_config = return_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Config")
    });
    assert!(
        has_config,
        "Should have ReturnType edge for Config, got edges: {:?}",
        return_edges
    );

    let has_apperror = return_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "AppError")
    });
    assert!(
        has_apperror,
        "Should have ReturnType edge for AppError, got edges: {:?}",
        return_edges
    );
}

#[test]
fn test_property_type_ref() {
    let extractor = KotlinExtractor;
    let code = load_testdata("type_refs.kt");
    let parsed = extractor.extract(&code, "type_refs.kt");

    let field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::FieldType)
        .collect();

    let has_userprofile = field_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "UserProfile")
    });
    assert!(
        has_userprofile,
        "Should have FieldType edge for UserProfile property, got edges: {:?}",
        field_edges
    );
}

#[test]
fn test_generic_base_type_ref() {
    let extractor = KotlinExtractor;
    let code = load_testdata("type_refs.kt");
    let parsed = extractor.extract(&code, "type_refs.kt");

    // getUsers(): List<UserProfile> should extract UserProfile as a type arg ref
    // We need to verify the edge comes FROM getUsers specifically (not from process())
    let all_type_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == crate::a6s::types::EdgeKind::TypeRef
                || e.kind == crate::a6s::types::EdgeKind::ReturnType
                || e.kind == crate::a6s::types::EdgeKind::ParamType
        })
        .collect();

    // getUsers() should have a TypeRef to UserProfile (from generic arg List<UserProfile>)
    let getusers_to_userprofile = all_type_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("getUsers"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "UserProfile")
    });
    assert!(
        getusers_to_userprofile,
        "Should extract UserProfile from List<UserProfile> in getUsers() return type, got edges: {:?}",
        all_type_edges
    );

    // getMapping(): Map<String, Config> should extract Config from type arg
    let getmapping_to_config = all_type_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("getMapping"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Config")
    });
    assert!(
        getmapping_to_config,
        "Should extract Config from Map<String, Config> in getMapping() return type, got edges: {:?}",
        all_type_edges
    );

    // processItems(items: List<Handler>) should extract Handler from type arg
    let processitems_to_handler = all_type_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("processItems"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Handler")
    });
    assert!(
        processitems_to_handler,
        "Should extract Handler from List<Handler> in processItems() param type, got edges: {:?}",
        all_type_edges
    );
}

#[test]
fn test_nullable_type_ref() {
    let extractor = KotlinExtractor;
    let code = load_testdata("type_refs.kt");
    let parsed = extractor.extract(&code, "type_refs.kt");

    // findUser(id: Int): UserProfile? should extract UserProfile from nullable return type
    let all_type_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == crate::a6s::types::EdgeKind::ReturnType
                || e.kind == crate::a6s::types::EdgeKind::FieldType
                || e.kind == crate::a6s::types::EdgeKind::TypeRef
        })
        .collect();

    // UserProfile? return type on findUser
    let finduser_to_userprofile = all_type_edges.iter().any(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains("findUser"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "UserProfile")
    });
    assert!(
        finduser_to_userprofile,
        "Should extract UserProfile from nullable return type UserProfile? on findUser, got edges: {:?}",
        all_type_edges
    );

    // var optionalConfig: Config? should extract Config
    let optional_config = all_type_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Config")
    });
    assert!(
        optional_config,
        "Should extract Config from nullable property type Config?, got edges: {:?}",
        all_type_edges
    );
}

// ============================================================================
// Phase 7: Test Detection Tests
// ============================================================================

// --- Subtask 1: Annotation-based test detection ---

#[test]
fn test_annotation_test_detection() {
    let extractor = KotlinExtractor;
    let code = load_testdata("tests.kt");
    let parsed = extractor.extract(&code, "tests.kt");

    // @Test annotated functions should have entry_type = "test"
    let should_process = parsed
        .symbols
        .iter()
        .find(|s| s.name == "shouldProcessData")
        .expect("Should find shouldProcessData");
    assert_eq!(
        should_process.entry_type,
        Some("test".to_string()),
        "@Test annotated function should have entry_type 'test'"
    );

    let should_handle = parsed
        .symbols
        .iter()
        .find(|s| s.name == "shouldHandleErrors")
        .expect("Should find shouldHandleErrors");
    assert_eq!(
        should_handle.entry_type,
        Some("test".to_string()),
        "@Test annotated function should have entry_type 'test'"
    );

    let verify = parsed
        .symbols
        .iter()
        .find(|s| s.name == "verifyBehavior")
        .expect("Should find verifyBehavior");
    assert_eq!(
        verify.entry_type,
        Some("test".to_string()),
        "@Test annotated function should have entry_type 'test'"
    );

    // Regular method should NOT be a test
    let helper = parsed
        .symbols
        .iter()
        .find(|s| s.name == "helperMethod")
        .expect("Should find helperMethod");
    assert_eq!(
        helper.entry_type, None,
        "helperMethod without @Test should not be a test"
    );

    // Regular top-level function should NOT be a test
    let regular = parsed
        .symbols
        .iter()
        .find(|s| s.name == "regularFunction")
        .expect("Should find regularFunction");
    assert_eq!(
        regular.entry_type, None,
        "regularFunction should not be a test"
    );
}

#[test]
fn test_lifecycle_annotation_detection() {
    let extractor = KotlinExtractor;
    let code = load_testdata("tests.kt");
    let parsed = extractor.extract(&code, "tests.kt");

    // @Before, @After, @BeforeClass, @AfterClass should be marked as test
    for name in &["setUp", "tearDown", "setupClass", "teardownClass"] {
        let sym = parsed
            .symbols
            .iter()
            .find(|s| s.name == *name)
            .unwrap_or_else(|| panic!("Should find {}", name));
        assert_eq!(
            sym.entry_type,
            Some("test".to_string()),
            "{} with lifecycle annotation should have entry_type 'test'",
            name
        );
    }
}

// --- Subtask 2: Naming convention detection ---

#[test]
fn test_naming_convention_prefix() {
    let extractor = KotlinExtractor;
    let code = load_testdata("tests.kt");
    let parsed = extractor.extract(&code, "tests.kt");

    // Functions starting with "test" (lowercase) should be detected
    let test_user = parsed
        .symbols
        .iter()
        .find(|s| s.name == "testUserCreation")
        .expect("Should find testUserCreation");
    assert_eq!(
        test_user.entry_type,
        Some("test".to_string()),
        "testUserCreation should be detected as test by naming convention"
    );

    let test_calc = parsed
        .symbols
        .iter()
        .find(|s| s.name == "testCalculation")
        .expect("Should find testCalculation");
    assert_eq!(
        test_calc.entry_type,
        Some("test".to_string()),
        "testCalculation should be detected as test by naming convention"
    );
}

#[test]
fn test_naming_convention_suffix() {
    let extractor = KotlinExtractor;
    let code = load_testdata("tests.kt");
    let parsed = extractor.extract(&code, "tests.kt");

    // Functions ending with "Test" (capital T) should be detected
    let user_test = parsed
        .symbols
        .iter()
        .find(|s| s.name == "userCreationTest")
        .expect("Should find userCreationTest");
    assert_eq!(
        user_test.entry_type,
        Some("test".to_string()),
        "userCreationTest should be detected as test by suffix convention"
    );

    let calc_test = parsed
        .symbols
        .iter()
        .find(|s| s.name == "calculateTest")
        .expect("Should find calculateTest");
    assert_eq!(
        calc_test.entry_type,
        Some("test".to_string()),
        "calculateTest should be detected as test by suffix convention"
    );

    // "testing" should NOT be detected
    let testing = parsed
        .symbols
        .iter()
        .find(|s| s.name == "testing")
        .expect("Should find testing");
    assert_eq!(
        testing.entry_type, None,
        "'testing' should NOT be detected as test"
    );

    // "contest" should NOT be detected
    let contest = parsed
        .symbols
        .iter()
        .find(|s| s.name == "contest")
        .expect("Should find contest");
    assert_eq!(
        contest.entry_type, None,
        "'contest' should NOT be detected as test"
    );
}

// --- Subtask 3: File categorization ---

#[test]
fn test_file_categorization_test_dir() {
    let extractor = KotlinExtractor;
    let code = "fun regularFunction() {}";
    let parsed = extractor.extract(code, "src/test/kotlin/com/example/MyTest.kt");

    assert_eq!(
        parsed.file_category,
        Some("test_file".to_string()),
        "File in /test/ directory should be categorized as test_file"
    );
}

#[test]
fn test_file_categorization_contains_tests() {
    let extractor = KotlinExtractor;
    // File NOT in test dir, but contains @Test annotated function
    let code = "@Test\nfun shouldWork() {}";
    let parsed = extractor.extract(code, "src/main/kotlin/MyService.kt");

    assert_eq!(
        parsed.file_category,
        Some("contains_tests".to_string()),
        "File with test functions but not in test dir should be 'contains_tests'"
    );
}

#[test]
fn test_file_categorization_regular() {
    let extractor = KotlinExtractor;
    let code = "fun regularFunction() {}";
    let parsed = extractor.extract(code, "src/main/kotlin/MyService.kt");

    assert!(
        parsed.file_category.is_none(),
        "Regular file without tests should not have file_category set"
    );
}

// --- Subtask 2: Unresolved references ---

#[test]
fn test_type_ref_uses_unresolved() {
    let extractor = KotlinExtractor;
    let code = load_testdata("type_refs.kt");
    let parsed = extractor.extract(&code, "type_refs.kt");

    let type_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == crate::a6s::types::EdgeKind::ParamType
                || e.kind == crate::a6s::types::EdgeKind::ReturnType
                || e.kind == crate::a6s::types::EdgeKind::FieldType
                || e.kind == crate::a6s::types::EdgeKind::TypeRef
        })
        .collect();

    // All type ref targets should be Unresolved
    for edge in &type_edges {
        assert!(
            matches!(&edge.to, crate::a6s::types::SymbolRef::Unresolved { .. }),
            "Type ref target should be SymbolRef::Unresolved, got: {:?}",
            edge.to
        );
    }

    // Should have at least some type ref edges
    assert!(
        !type_edges.is_empty(),
        "Should have some type reference edges"
    );
}

#[test]
fn test_type_ref_context() {
    let extractor = KotlinExtractor;
    let code = load_testdata("type_refs.kt");
    let parsed = extractor.extract(&code, "type_refs.kt");

    let type_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == crate::a6s::types::EdgeKind::ParamType
                || e.kind == crate::a6s::types::EdgeKind::ReturnType
                || e.kind == crate::a6s::types::EdgeKind::FieldType
                || e.kind == crate::a6s::types::EdgeKind::TypeRef
        })
        .collect();

    // Verify unresolved refs include file_path context
    for edge in &type_edges {
        if let crate::a6s::types::SymbolRef::Unresolved { file_path, .. } = &edge.to {
            assert_eq!(
                file_path, "type_refs.kt",
                "Unresolved type ref should include file_path context"
            );
        }
    }
}

// ============================================================================
// Phase 8: Cross-File Resolution Tests
// ============================================================================

use crate::a6s::types::{
    EdgeKind, ImportEntry, ParsedFile, QualifiedName, RawEdge, RawImport, RawSymbol, SymbolId,
    SymbolRef,
};

/// Helper to create a RawSymbol for testing
fn make_symbol(
    name: &str,
    kind: &str,
    file_path: &str,
    start_line: usize,
    module_path: Option<&str>,
) -> RawSymbol {
    RawSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        file_path: file_path.to_string(),
        start_line,
        end_line: start_line + 5,
        signature: None,
        language: "kotlin".to_string(),
        visibility: Some("pub".to_string()),
        entry_type: None,
        module_path: module_path.map(|s| s.to_string()),
    }
}

/// Helper to create a ParsedFile for testing
fn make_parsed_file(file_path: &str, symbols: Vec<RawSymbol>) -> ParsedFile {
    ParsedFile {
        file_path: file_path.to_string(),
        language: "kotlin".to_string(),
        symbols,
        edges: Vec::new(),
        imports: Vec::new(),
        file_category: None,
    }
}

// --- Subtask 1: Build symbol index ---

#[test]
fn test_build_symbol_index_single_file() {
    let extractor = KotlinExtractor;
    let mut files = vec![{
        let pf = make_parsed_file(
            "src/models.kt",
            vec![
                make_symbol("User", "class", "src/models.kt", 3, Some("com::example")),
                make_symbol("Config", "class", "src/models.kt", 10, Some("com::example")),
            ],
        );
        pf
    }];

    let (resolved_edges, _) = extractor.resolve_cross_file(&mut files);
    // No edges to resolve → just verify it doesn't panic
    assert!(resolved_edges.is_empty());
}

#[test]
fn test_build_symbol_index_multiple_files() {
    let extractor = KotlinExtractor;
    let mut files = vec![
        make_parsed_file(
            "src/models.kt",
            vec![make_symbol(
                "User",
                "class",
                "src/models.kt",
                3,
                Some("com::example::models"),
            )],
        ),
        make_parsed_file(
            "src/service.kt",
            vec![make_symbol(
                "UserService",
                "class",
                "src/service.kt",
                5,
                Some("com::example::service"),
            )],
        ),
    ];

    // Add an unresolved edge from UserService to User
    files[1].edges.push(RawEdge {
        from: SymbolRef::resolved(SymbolId::new("src/service.kt", "UserService", 5)),
        to: SymbolRef::unresolved("User", "src/service.kt"),
        kind: EdgeKind::TypeRef,
        line: Some(6),
    });

    // Add an import for User in service.kt
    files[1].imports.push(RawImport {
        file_path: "src/service.kt".to_string(),
        entry: ImportEntry::named_import("com::example::models", vec!["User".to_string()]),
    });

    let (resolved_edges, _) = extractor.resolve_cross_file(&mut files);

    // Should resolve User via import
    assert!(
        !resolved_edges.is_empty(),
        "Should resolve cross-file edge via import"
    );
    let user_edge = resolved_edges
        .iter()
        .find(|e| e.to.as_str().contains("User") && e.from.as_str().contains("UserService"));
    assert!(
        user_edge.is_some(),
        "Should resolve TypeRef from UserService to User, got: {:?}",
        resolved_edges
    );
}

// --- Subtask 2: resolve_name() ---

#[test]
fn test_resolve_name_same_package() {
    use std::collections::HashMap;

    let sym_id = SymbolId::new("src/models.kt", "User", 3);
    let qname = QualifiedName::new("com::example", "User");
    let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
    symbol_index.insert(qname, sym_id.clone());
    let bare_index: HashMap<String, Vec<SymbolId>> =
        [("User".to_string(), vec![sym_id.clone()])].into();

    let imports: Vec<&RawImport> = vec![];

    // Same package should resolve
    let result =
        KotlinExtractor::resolve_name("User", "com::example", &symbol_index, &bare_index, &imports);
    assert!(result.is_some(), "Should resolve same-package symbol");
    assert_eq!(result.unwrap().as_str(), sym_id.as_str());
}

#[test]
fn test_resolve_name_imported_symbol() {
    use std::collections::HashMap;

    let sym_id = SymbolId::new("src/models.kt", "User", 3);
    let qname = QualifiedName::new("com::example::models", "User");
    let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
    symbol_index.insert(qname, sym_id.clone());
    let bare_index: HashMap<String, Vec<SymbolId>> =
        [("User".to_string(), vec![sym_id.clone()])].into();

    let import = RawImport {
        file_path: "src/service.kt".to_string(),
        entry: ImportEntry::named_import("com::example::models", vec!["User".to_string()]),
    };
    let imports: Vec<&RawImport> = vec![&import];

    // Different package, but imported → should resolve
    let result = KotlinExtractor::resolve_name(
        "User",
        "com::example::service",
        &symbol_index,
        &bare_index,
        &imports,
    );
    assert!(result.is_some(), "Should resolve imported symbol");
    assert_eq!(result.unwrap().as_str(), sym_id.as_str());
}

#[test]
fn test_resolve_name_bare_unique() {
    use std::collections::HashMap;

    let sym_id = SymbolId::new("src/models.kt", "UniqueClass", 3);
    let qname = QualifiedName::new("com::example::models", "UniqueClass");
    let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
    symbol_index.insert(qname, sym_id.clone());
    let bare_index: HashMap<String, Vec<SymbolId>> =
        [("UniqueClass".to_string(), vec![sym_id.clone()])].into();

    let imports: Vec<&RawImport> = vec![];

    // Different package, no import, but unique bare name → should resolve
    let result = KotlinExtractor::resolve_name(
        "UniqueClass",
        "com::other",
        &symbol_index,
        &bare_index,
        &imports,
    );
    assert!(result.is_some(), "Should resolve unique bare name");
}

#[test]
fn test_resolve_name_bare_ambiguous() {
    use std::collections::HashMap;

    let sym1 = SymbolId::new("src/a.kt", "Config", 3);
    let sym2 = SymbolId::new("src/b.kt", "Config", 5);
    let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
    symbol_index.insert(QualifiedName::new("pkg::a", "Config"), sym1.clone());
    symbol_index.insert(QualifiedName::new("pkg::b", "Config"), sym2.clone());
    let bare_index: HashMap<String, Vec<SymbolId>> =
        [("Config".to_string(), vec![sym1, sym2])].into();

    let imports: Vec<&RawImport> = vec![];

    // Ambiguous bare name → should NOT resolve
    let result =
        KotlinExtractor::resolve_name("Config", "pkg::c", &symbol_index, &bare_index, &imports);
    assert!(result.is_none(), "Should not resolve ambiguous bare name");
}

#[test]
fn test_resolve_name_priority_same_pkg_over_import() {
    use std::collections::HashMap;

    let local_id = SymbolId::new("src/local.kt", "Helper", 3);
    let remote_id = SymbolId::new("src/remote.kt", "Helper", 10);

    let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
    symbol_index.insert(QualifiedName::new("com::local", "Helper"), local_id.clone());
    symbol_index.insert(
        QualifiedName::new("com::remote", "Helper"),
        remote_id.clone(),
    );
    let bare_index: HashMap<String, Vec<SymbolId>> = [(
        "Helper".to_string(),
        vec![local_id.clone(), remote_id.clone()],
    )]
    .into();

    let import = RawImport {
        file_path: "src/local.kt".to_string(),
        entry: ImportEntry::named_import("com::remote", vec!["Helper".to_string()]),
    };
    let imports: Vec<&RawImport> = vec![&import];

    // Same-package should win over import
    let result =
        KotlinExtractor::resolve_name("Helper", "com::local", &symbol_index, &bare_index, &imports);
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().as_str(),
        local_id.as_str(),
        "Same-package should have priority over imported"
    );
}

// --- Subtask 3: Resolve Calls and TypeRef cross-file ---

#[test]
fn test_resolve_calls_cross_file() {
    let extractor = KotlinExtractor;
    let mut files = vec![
        make_parsed_file(
            "src/utils.kt",
            vec![make_symbol(
                "helper",
                "function",
                "src/utils.kt",
                3,
                Some("com::example"),
            )],
        ),
        {
            let mut pf = make_parsed_file(
                "src/main.kt",
                vec![make_symbol(
                    "main",
                    "function",
                    "src/main.kt",
                    1,
                    Some("com::example"),
                )],
            );
            pf.edges.push(RawEdge {
                from: SymbolRef::resolved(SymbolId::new("src/main.kt", "main", 1)),
                to: SymbolRef::unresolved("helper", "src/main.kt"),
                kind: EdgeKind::Calls,
                line: Some(3),
            });
            pf
        },
    ];

    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    let call_edge = resolved
        .iter()
        .find(|e| e.kind == EdgeKind::Calls && e.to.as_str().contains("helper"));
    assert!(
        call_edge.is_some(),
        "Should resolve Calls edge to helper cross-file, got: {:?}",
        resolved
    );
}

#[test]
fn test_resolve_typeref_cross_file() {
    let extractor = KotlinExtractor;
    let mut files = vec![
        make_parsed_file(
            "src/models.kt",
            vec![make_symbol(
                "User",
                "class",
                "src/models.kt",
                3,
                Some("com::example"),
            )],
        ),
        {
            let mut pf = make_parsed_file(
                "src/service.kt",
                vec![make_symbol(
                    "getUser",
                    "function",
                    "src/service.kt",
                    1,
                    Some("com::example"),
                )],
            );
            pf.edges.push(RawEdge {
                from: SymbolRef::resolved(SymbolId::new("src/service.kt", "getUser", 1)),
                to: SymbolRef::unresolved("User", "src/service.kt"),
                kind: EdgeKind::ReturnType,
                line: Some(1),
            });
            pf
        },
    ];

    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    let type_edge = resolved
        .iter()
        .find(|e| e.kind == EdgeKind::ReturnType && e.to.as_str().contains("User"));
    assert!(
        type_edge.is_some(),
        "Should resolve ReturnType edge to User cross-file, got: {:?}",
        resolved
    );
}

#[test]
fn test_skip_builtins_during_resolution() {
    let extractor = KotlinExtractor;
    let mut files = vec![{
        let mut pf = make_parsed_file(
            "src/main.kt",
            vec![make_symbol(
                "process",
                "function",
                "src/main.kt",
                1,
                Some("com::example"),
            )],
        );
        pf.edges.push(RawEdge {
            from: SymbolRef::resolved(SymbolId::new("src/main.kt", "process", 1)),
            to: SymbolRef::unresolved("String", "src/main.kt"),
            kind: EdgeKind::ParamType,
            line: Some(1),
        });
        pf
    }];

    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // String is a builtin — should not produce a resolved edge
    let string_edge = resolved.iter().find(|e| e.to.as_str().contains("String"));
    assert!(
        string_edge.is_none(),
        "Should skip builtin type String during resolution"
    );
}

// --- Subtask 4: Import resolution ---

#[test]
fn test_import_resolution_single() {
    let extractor = KotlinExtractor;
    let mut files = vec![
        make_parsed_file(
            "src/models.kt",
            vec![make_symbol(
                "User",
                "class",
                "src/models.kt",
                3,
                Some("com::example::models"),
            )],
        ),
        {
            let mut pf = make_parsed_file(
                "src/service.kt",
                vec![make_symbol(
                    "serve",
                    "function",
                    "src/service.kt",
                    1,
                    Some("com::example::service"),
                )],
            );
            pf.imports.push(RawImport {
                file_path: "src/service.kt".to_string(),
                entry: ImportEntry::named_import("com::example::models", vec!["User".to_string()]),
            });
            pf.edges.push(RawEdge {
                from: SymbolRef::resolved(SymbolId::new("src/service.kt", "serve", 1)),
                to: SymbolRef::unresolved("User", "src/service.kt"),
                kind: EdgeKind::TypeRef,
                line: Some(2),
            });
            pf
        },
    ];

    let (resolved_edges, resolved_imports) = extractor.resolve_cross_file(&mut files);

    // Import should be resolved
    assert!(
        !resolved_imports.is_empty(),
        "Should resolve import of User"
    );

    // Edge should also be resolved via import
    let edge = resolved_edges
        .iter()
        .find(|e| e.to.as_str().contains("User"));
    assert!(
        edge.is_some(),
        "Should resolve edge to User via import, got: {:?}",
        resolved_edges
    );
}

#[test]
fn test_import_resolution_wildcard() {
    let extractor = KotlinExtractor;
    let mut files = vec![
        make_parsed_file(
            "src/models.kt",
            vec![
                make_symbol(
                    "User",
                    "class",
                    "src/models.kt",
                    3,
                    Some("com::example::models"),
                ),
                make_symbol(
                    "Config",
                    "class",
                    "src/models.kt",
                    10,
                    Some("com::example::models"),
                ),
            ],
        ),
        {
            let mut pf = make_parsed_file(
                "src/service.kt",
                vec![make_symbol(
                    "serve",
                    "function",
                    "src/service.kt",
                    1,
                    Some("com::example::service"),
                )],
            );
            // Wildcard import: import com.example.models.*
            pf.imports.push(RawImport {
                file_path: "src/service.kt".to_string(),
                entry: ImportEntry::glob_import("com::example::models"),
            });
            pf.edges.push(RawEdge {
                from: SymbolRef::resolved(SymbolId::new("src/service.kt", "serve", 1)),
                to: SymbolRef::unresolved("User", "src/service.kt"),
                kind: EdgeKind::TypeRef,
                line: Some(2),
            });
            pf
        },
    ];

    let (resolved_edges, _) = extractor.resolve_cross_file(&mut files);

    let edge = resolved_edges
        .iter()
        .find(|e| e.to.as_str().contains("User"));
    assert!(
        edge.is_some(),
        "Should resolve User via wildcard import, got: {:?}",
        resolved_edges
    );
}

#[test]
fn test_import_resolution_aliased() {
    let extractor = KotlinExtractor;
    let mut files = vec![
        make_parsed_file(
            "src/models.kt",
            vec![make_symbol(
                "User",
                "class",
                "src/models.kt",
                3,
                Some("com::example::models"),
            )],
        ),
        {
            let mut pf = make_parsed_file(
                "src/service.kt",
                vec![make_symbol(
                    "serve",
                    "function",
                    "src/service.kt",
                    1,
                    Some("com::example::service"),
                )],
            );
            // Aliased import: import com.example.models.User as AppUser
            let mut entry =
                ImportEntry::named_import("com::example::models", vec!["User".to_string()]);
            entry.alias = Some("AppUser".to_string());
            pf.imports.push(RawImport {
                file_path: "src/service.kt".to_string(),
                entry,
            });
            // Code uses the alias "AppUser"
            pf.edges.push(RawEdge {
                from: SymbolRef::resolved(SymbolId::new("src/service.kt", "serve", 1)),
                to: SymbolRef::unresolved("AppUser", "src/service.kt"),
                kind: EdgeKind::TypeRef,
                line: Some(2),
            });
            pf
        },
    ];

    let (resolved_edges, _) = extractor.resolve_cross_file(&mut files);

    let edge = resolved_edges
        .iter()
        .find(|e| e.to.as_str().contains("User"));
    assert!(
        edge.is_some(),
        "Should resolve aliased AppUser to User, got: {:?}",
        resolved_edges
    );
}

// --- Subtask 5: resolve_cross_file() integration ---

#[test]
fn test_resolve_cross_file_end_to_end() {
    let extractor = KotlinExtractor;

    // File 1: com.example.models - defines User, Config
    let models = make_parsed_file(
        "src/models.kt",
        vec![
            make_symbol(
                "User",
                "class",
                "src/models.kt",
                3,
                Some("com::example::models"),
            ),
            make_symbol(
                "Config",
                "class",
                "src/models.kt",
                10,
                Some("com::example::models"),
            ),
        ],
    );

    // File 2: com.example.service - uses User (imported), calls helper (same pkg)
    let mut service = make_parsed_file(
        "src/service.kt",
        vec![
            make_symbol(
                "UserService",
                "class",
                "src/service.kt",
                3,
                Some("com::example::service"),
            ),
            make_symbol(
                "processUser",
                "function",
                "src/service.kt",
                10,
                Some("com::example::service"),
            ),
        ],
    );
    service.imports.push(RawImport {
        file_path: "src/service.kt".to_string(),
        entry: ImportEntry::named_import("com::example::models", vec!["User".to_string()]),
    });
    service.edges.push(RawEdge {
        from: SymbolRef::resolved(SymbolId::new("src/service.kt", "processUser", 10)),
        to: SymbolRef::unresolved("User", "src/service.kt"),
        kind: EdgeKind::ParamType,
        line: Some(10),
    });

    // File 3: com.example.service - defines helper (same package as service)
    let utils = make_parsed_file(
        "src/utils.kt",
        vec![make_symbol(
            "helper",
            "function",
            "src/utils.kt",
            1,
            Some("com::example::service"),
        )],
    );
    // Add a call from processUser to helper
    service.edges.push(RawEdge {
        from: SymbolRef::resolved(SymbolId::new("src/service.kt", "processUser", 10)),
        to: SymbolRef::unresolved("helper", "src/service.kt"),
        kind: EdgeKind::Calls,
        line: Some(12),
    });

    let mut files = vec![models, service, utils];
    let (resolved_edges, resolved_imports) = extractor.resolve_cross_file(&mut files);

    // 1) User should be resolved via import
    let user_edge = resolved_edges
        .iter()
        .find(|e| e.kind == EdgeKind::ParamType && e.to.as_str().contains("User"));
    assert!(
        user_edge.is_some(),
        "Should resolve User via import, got: {:?}",
        resolved_edges
    );

    // 2) helper should be resolved via same-package lookup
    let helper_edge = resolved_edges
        .iter()
        .find(|e| e.kind == EdgeKind::Calls && e.to.as_str().contains("helper"));
    assert!(
        helper_edge.is_some(),
        "Should resolve helper via same-package lookup, got: {:?}",
        resolved_edges
    );

    // 3) Should have resolved the User import
    assert!(!resolved_imports.is_empty(), "Should have resolved imports");

    // 4) Already-resolved edges should pass through
    let already_resolved_count = resolved_edges
        .iter()
        .filter(|e| e.from.as_str().contains("processUser"))
        .count();
    assert!(
        already_resolved_count >= 2,
        "Should have at least 2 edges from processUser (ParamType + Calls)"
    );
}

#[test]
fn test_resolve_cross_file_skips_non_kotlin() {
    let extractor = KotlinExtractor;
    let mut files = vec![ParsedFile {
        file_path: "main.go".to_string(),
        language: "go".to_string(),
        symbols: vec![],
        edges: vec![],
        imports: vec![],
        file_category: None,
    }];

    let (resolved, imports) = extractor.resolve_cross_file(&mut files);
    assert!(resolved.is_empty());
    assert!(imports.is_empty());
}

#[test]
fn test_resolve_cross_file_already_resolved_passthrough() {
    let extractor = KotlinExtractor;
    let mut files = vec![{
        let mut pf = make_parsed_file(
            "src/main.kt",
            vec![
                make_symbol("Foo", "class", "src/main.kt", 1, Some("com::example")),
                make_symbol("bar", "function", "src/main.kt", 5, Some("com::example")),
            ],
        );
        // Already-resolved edge
        pf.edges.push(RawEdge {
            from: SymbolRef::resolved(SymbolId::new("src/main.kt", "bar", 5)),
            to: SymbolRef::resolved(SymbolId::new("src/main.kt", "Foo", 1)),
            kind: EdgeKind::ReturnType,
            line: Some(5),
        });
        pf
    }];

    let (resolved, _) = extractor.resolve_cross_file(&mut files);
    assert_eq!(
        resolved.len(),
        1,
        "Already-resolved edge should pass through"
    );
    assert!(resolved[0].to.as_str().contains("Foo"));
}

// ============================================================================
// Phase 9: Integration & Complex Cases
// ============================================================================

// --- Subtask 1: Complex cases + visibility filtering ---

#[test]
fn test_nested_class_extraction() {
    let extractor = KotlinExtractor;
    let code = load_testdata("complex.kt");
    let parsed = extractor.extract(&code, "complex.kt");

    // Outer class
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Outer" && s.kind == "class"),
        "Should extract Outer class"
    );
    // Nested class
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Nested" && s.kind == "class"),
        "Should extract nested class Nested"
    );
    // Inner class
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Inner" && s.kind == "class"),
        "Should extract inner class Inner"
    );
    // Methods inside nested/inner
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "nestedMethod" && s.kind == "method"),
        "Should extract nestedMethod"
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "innerMethod" && s.kind == "method"),
        "Should extract innerMethod"
    );
    // Inner class should have "inner" in signature
    let inner = parsed.symbols.iter().find(|s| s.name == "Inner").unwrap();
    let sig = inner.signature.as_deref().unwrap_or("");
    assert!(
        sig.contains("inner"),
        "Inner class signature should contain 'inner', got: {}",
        sig
    );
}

#[test]
fn test_extension_function_edges() {
    let extractor = KotlinExtractor;
    let code = load_testdata("complex.kt");
    let parsed = extractor.extract(&code, "complex.kt");

    // Extension functions should be kind "extension_function"
    let word_count = parsed.symbols.iter().find(|s| s.name == "wordCount");
    assert!(
        word_count.is_some(),
        "Should extract wordCount extension function"
    );
    assert_eq!(
        word_count.unwrap().kind,
        "extension_function",
        "Extension function should be extension_function"
    );

    let ext_on_outer = parsed.symbols.iter().find(|s| s.name == "extensionOnOuter");
    assert!(
        ext_on_outer.is_some(),
        "Should extract extensionOnOuter extension function"
    );
    assert_eq!(
        ext_on_outer.unwrap().kind,
        "extension_function",
        "Extension function should be extension_function"
    );
}

#[test]
fn test_sealed_hierarchy() {
    let extractor = KotlinExtractor;
    let code = load_testdata("complex.kt");
    let parsed = extractor.extract(&code, "complex.kt");

    // Sealed class
    let shape = parsed
        .symbols
        .iter()
        .find(|s| s.name == "Shape" && s.kind == "class");
    assert!(shape.is_some(), "Should extract sealed class Shape");
    let shape_sig = shape.unwrap().signature.as_deref().unwrap_or("");
    assert!(
        shape_sig.contains("sealed"),
        "Shape should have sealed in signature, got: {}",
        shape_sig
    );

    // Sealed subclasses
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Circle" && s.kind == "class"),
        "Should extract Circle subclass"
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Rectangle" && s.kind == "class"),
        "Should extract Rectangle subclass"
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "Unknown" && s.kind == "object"),
        "Should extract Unknown object subclass"
    );

    // Circle and Rectangle should extend Shape
    let extends_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Extends)
        .collect();
    let circle_extends_shape = extends_edges.iter().any(|e| {
        matches!(&e.from, SymbolRef::Resolved(id) if id.as_str().contains("Circle"))
            && matches!(&e.to, SymbolRef::Unresolved { name, .. } if name == "Shape")
    });
    assert!(
        circle_extends_shape,
        "Circle should extend Shape, got edges: {:?}",
        extends_edges
    );
}

#[test]
fn test_companion_object_members() {
    let extractor = KotlinExtractor;
    let code = load_testdata("complex.kt");
    let parsed = extractor.extract(&code, "complex.kt");

    // Companion object should be extracted
    let companion = parsed
        .symbols
        .iter()
        .find(|s| s.name == "Companion" && s.signature.as_deref() == Some("companion object"));
    assert!(
        companion.is_some(),
        "Should extract Companion object, got symbols: {:?}",
        parsed
            .symbols
            .iter()
            .map(|s| (&s.name, &s.kind, &s.signature))
            .collect::<Vec<_>>()
    );

    // Named companion object
    let loader = parsed
        .symbols
        .iter()
        .find(|s| s.name == "Loader" && s.signature.as_deref() == Some("companion object"));
    assert!(
        loader.is_some(),
        "Should extract named companion object Loader"
    );

    // Methods inside companion should be extracted
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "create" && s.kind == "method"),
        "Should extract create() from companion object"
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "load" && s.kind == "method"),
        "Should extract load() from named companion object"
    );
}

#[test]
fn test_private_not_resolved_cross_file() {
    let extractor = KotlinExtractor;

    // File 1: defines a private class
    let file1 = make_parsed_file(
        "src/helpers.kt",
        vec![{
            let mut s = make_symbol(
                "InternalHelper",
                "class",
                "src/helpers.kt",
                3,
                Some("com::example"),
            );
            s.visibility = Some("private".to_string());
            s
        }],
    );

    // File 2: tries to reference InternalHelper
    let mut file2 = make_parsed_file(
        "src/consumer.kt",
        vec![make_symbol(
            "Consumer",
            "class",
            "src/consumer.kt",
            1,
            Some("com::example"),
        )],
    );
    file2.edges.push(RawEdge {
        from: SymbolRef::resolved(SymbolId::new("src/consumer.kt", "Consumer", 1)),
        to: SymbolRef::unresolved("InternalHelper", "src/consumer.kt"),
        kind: EdgeKind::TypeRef,
        line: Some(2),
    });

    let mut files = vec![file1, file2];
    let (resolved_edges, _) = extractor.resolve_cross_file(&mut files);

    // Private symbol should NOT be resolved from a different file
    let helper_edge = resolved_edges
        .iter()
        .find(|e| e.to.as_str().contains("InternalHelper"));
    assert!(
        helper_edge.is_none(),
        "Private symbol should NOT be resolved cross-file, got: {:?}",
        resolved_edges
    );
}

#[test]
fn test_internal_same_package_resolved() {
    let extractor = KotlinExtractor;

    // File 1: defines an internal class in com::example
    let file1 = make_parsed_file(
        "src/helpers.kt",
        vec![{
            let mut s = make_symbol(
                "PackageHelper",
                "class",
                "src/helpers.kt",
                3,
                Some("com::example"),
            );
            s.visibility = Some("internal".to_string());
            s
        }],
    );

    // File 2: same package, references PackageHelper
    let mut file2_same = make_parsed_file(
        "src/consumer.kt",
        vec![make_symbol(
            "Consumer",
            "class",
            "src/consumer.kt",
            1,
            Some("com::example"),
        )],
    );
    file2_same.edges.push(RawEdge {
        from: SymbolRef::resolved(SymbolId::new("src/consumer.kt", "Consumer", 1)),
        to: SymbolRef::unresolved("PackageHelper", "src/consumer.kt"),
        kind: EdgeKind::TypeRef,
        line: Some(2),
    });

    let mut files = vec![file1.clone(), file2_same];
    let (resolved_edges, _) = extractor.resolve_cross_file(&mut files);

    let helper_edge = resolved_edges
        .iter()
        .find(|e| e.to.as_str().contains("PackageHelper"));
    assert!(
        helper_edge.is_some(),
        "Internal symbol should be resolved within same package, got: {:?}",
        resolved_edges
    );

    // File 3: different package, references PackageHelper
    let mut file3_diff = make_parsed_file(
        "src/other.kt",
        vec![make_symbol(
            "Other",
            "class",
            "src/other.kt",
            1,
            Some("com::other"),
        )],
    );
    file3_diff.edges.push(RawEdge {
        from: SymbolRef::resolved(SymbolId::new("src/other.kt", "Other", 1)),
        to: SymbolRef::unresolved("PackageHelper", "src/other.kt"),
        kind: EdgeKind::TypeRef,
        line: Some(2),
    });

    let mut files2 = vec![file1, file3_diff];
    let (resolved_edges2, _) = extractor.resolve_cross_file(&mut files2);

    let helper_edge2 = resolved_edges2
        .iter()
        .find(|e| e.to.as_str().contains("PackageHelper"));
    assert!(
        helper_edge2.is_none(),
        "Internal symbol should NOT be resolved from different package, got: {:?}",
        resolved_edges2
    );
}

// --- Subtask 2: Error handling ---

#[test]
fn test_malformed_code_no_panic() {
    let extractor = KotlinExtractor;

    // Various malformed inputs
    let cases = vec![
        "{{{{",
        "class {",
        "fun (x: ) {",
        "import .",
        "package",
        "class Foo : { fun }",
        "sealed class { data class () }",
        "fun foo(): = ",
        "}}}",
        "class Foo {\n  fun bar(\n}",
        "\0\0\0", // null bytes
        "fun foo() { val x = \"unterminated string",
    ];

    for (i, code) in cases.iter().enumerate() {
        let parsed = extractor.extract(code, &format!("malformed_{}.kt", i));
        assert_eq!(
            parsed.language, "kotlin",
            "Should return kotlin language for case {}",
            i
        );
        // Just verify no panic — symbols may or may not be extracted
    }
}

#[test]
fn test_empty_file_no_panic() {
    let extractor = KotlinExtractor;

    let parsed = extractor.extract("", "empty.kt");
    assert_eq!(parsed.language, "kotlin");
    assert_eq!(parsed.file_path, "empty.kt");
    assert!(
        parsed.symbols.is_empty(),
        "Empty file should have no symbols"
    );
    assert!(parsed.edges.is_empty(), "Empty file should have no edges");
    assert!(
        parsed.imports.is_empty(),
        "Empty file should have no imports"
    );
}

#[test]
fn test_missing_import_stays_unresolved() {
    let extractor = KotlinExtractor;

    let mut file = make_parsed_file(
        "src/main.kt",
        vec![make_symbol(
            "process",
            "function",
            "src/main.kt",
            1,
            Some("com::example"),
        )],
    );
    // Reference to a symbol that doesn't exist anywhere
    file.edges.push(RawEdge {
        from: SymbolRef::resolved(SymbolId::new("src/main.kt", "process", 1)),
        to: SymbolRef::unresolved("NonExistentClass", "src/main.kt"),
        kind: EdgeKind::TypeRef,
        line: Some(2),
    });

    let mut files = vec![file];
    let (resolved_edges, _) = extractor.resolve_cross_file(&mut files);

    // NonExistentClass should NOT appear in resolved edges
    let missing_edge = resolved_edges
        .iter()
        .find(|e| e.to.as_str().contains("NonExistentClass"));
    assert!(
        missing_edge.is_none(),
        "Unresolvable symbol should stay unresolved (not in resolved edges), got: {:?}",
        resolved_edges
    );
}
