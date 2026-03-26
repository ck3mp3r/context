// Rust language parser implementation

use super::types::Kind;
use crate::analysis::parser::Language;
use tree_sitter::Node;

/// Rust language implementation
pub struct Rust;

impl Language for Rust {
    type Kind = Kind;

    fn grammar() -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn parse_symbol(node: Node, code: &str) -> Option<(Self::Kind, String)> {
        match node.kind() {
            "function_item" => {
                // Extract function name
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "identifier" {
                        let name = code[child.byte_range()].to_string();
                        return Some((Kind::Function, name));
                    }
                }
                None
            }
            "struct_item" => {
                // Extract struct name
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "type_identifier" {
                        let name = code[child.byte_range()].to_string();
                        return Some((Kind::Struct, name));
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn extract_callee(node: Node, code: &str) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                // Simple function call: foo()
                "identifier" => {
                    return Some(code[child.byte_range()].to_string());
                }
                // Scoped call: Foo::bar()
                "scoped_identifier" => {
                    // Get the last identifier (the function name)
                    for subchild in child.children(&mut child.walk()) {
                        if subchild.kind() == "identifier" {
                            return Some(code[subchild.byte_range()].to_string());
                        }
                    }
                }
                // Method call: obj.method()
                "field_expression" => {
                    // Get the field name (method name)
                    for subchild in child.children(&mut child.walk()) {
                        if subchild.kind() == "field_identifier" {
                            return Some(code[subchild.byte_range()].to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn name() -> &'static str {
        "rust"
    }

    fn extensions() -> &'static [&'static str] {
        &["rs"]
    }
}
