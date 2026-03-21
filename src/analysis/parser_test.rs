// Tests for Tree-sitter parser wrapper

use crate::analysis::parser::{LanguageRegistry, Parser};

#[test]
fn test_create_parser_for_rust() {
    let parser = Parser::new_rust();
    assert!(parser.is_ok());
}

#[test]
fn test_parse_rust_function() {
    let code = r#"
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
    "#;

    let mut parser = Parser::new_rust().unwrap();
    let tree = parser.parse(code);
    assert!(tree.is_some());
    assert!(!tree.unwrap().root_node().has_error());
}

#[test]
fn test_parse_invalid_code() {
    let code = "fn {{{ invalid";
    let mut parser = Parser::new_rust().unwrap();
    let tree = parser.parse(code);
    assert!(tree.is_some());
    assert!(tree.unwrap().root_node().has_error());
}

#[test]
fn test_language_registry() {
    let registry = LanguageRegistry::new();
    assert!(registry.supports_extension("rs"));
    assert!(!registry.supports_extension("py")); // Not in MVP
}

#[test]
fn test_get_parser_for_rust_file() {
    let registry = LanguageRegistry::new();
    let parser = registry.get_parser_for_file("src/main.rs");
    assert!(parser.is_some());
    assert_eq!(parser.unwrap().language_name(), "rust");
}
