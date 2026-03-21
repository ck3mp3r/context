// Rust symbol extraction using tree-sitter-rust
//
// Extracts Rust-specific symbols: functions, structs, traits, impls, enums, etc.

use crate::analysis::parser::Parser;
use crate::analysis::types::{ExtractedRelationship, ExtractedSymbol, SymbolKind};
use tree_sitter::Node;

/// Rust-specific symbol extractor
pub struct RustExtractor;

impl RustExtractor {
    /// Extract symbols from Rust source code
    pub fn extract(code: &str) -> Vec<ExtractedSymbol> {
        let mut parser = match Parser::new_rust() {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        let tree = match parser.parse(code) {
            Some(t) => t,
            None => return vec![],
        };

        let mut symbols = Vec::new();
        Self::walk_node(tree.root_node(), code, &mut symbols);
        symbols
    }

    fn walk_node(node: Node, code: &str, symbols: &mut Vec<ExtractedSymbol>) {
        match node.kind() {
            "function_item" => {
                Self::extract_function(node, code, symbols);
            }
            "struct_item" => {
                Self::extract_struct(node, code, symbols);
            }
            "trait_item" => {
                Self::extract_trait(node, code, symbols);
            }
            "impl_item" => {
                Self::extract_impl(node, code, symbols);
            }
            "enum_item" => {
                Self::extract_enum(node, code, symbols);
            }
            _ => {
                // Recurse into children
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    Self::walk_node(child, code, symbols);
                }
            }
        }
    }

    fn extract_function(node: Node, code: &str, symbols: &mut Vec<ExtractedSymbol>) {
        let name = Self::get_name(node, code);
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        let content = Self::get_text(node, code);

        symbols.push(ExtractedSymbol {
            name,
            kind: SymbolKind::Function,
            file_path: String::new(), // Set by caller
            start_line,
            end_line,
            content,
            signature: None, // TODO: Extract function signature
        });
    }

    fn extract_struct(node: Node, code: &str, symbols: &mut Vec<ExtractedSymbol>) {
        let name = Self::get_name(node, code);
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        let content = Self::get_text(node, code);

        symbols.push(ExtractedSymbol {
            name,
            kind: SymbolKind::Struct,
            file_path: String::new(),
            start_line,
            end_line,
            content,
            signature: None,
        });
    }

    fn extract_trait(node: Node, code: &str, symbols: &mut Vec<ExtractedSymbol>) {
        let name = Self::get_name(node, code);
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        let content = Self::get_text(node, code);

        symbols.push(ExtractedSymbol {
            name,
            kind: SymbolKind::Trait,
            file_path: String::new(),
            start_line,
            end_line,
            content,
            signature: None,
        });
    }

    fn extract_impl(node: Node, code: &str, symbols: &mut Vec<ExtractedSymbol>) {
        let target_type = Self::get_impl_target(node, code);
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        let content = Self::get_text(node, code);

        symbols.push(ExtractedSymbol {
            name: format!("impl {}", target_type),
            kind: SymbolKind::Impl {
                target_type: target_type.clone(),
            },
            file_path: String::new(),
            start_line,
            end_line,
            content,
            signature: None,
        });
    }

    fn extract_enum(node: Node, code: &str, symbols: &mut Vec<ExtractedSymbol>) {
        let name = Self::get_name(node, code);
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        let content = Self::get_text(node, code);

        symbols.push(ExtractedSymbol {
            name,
            kind: SymbolKind::Enum,
            file_path: String::new(),
            start_line,
            end_line,
            content,
            signature: None,
        });
    }

    fn get_name(node: Node, code: &str) -> String {
        node.child_by_field_name("name")
            .map(|n| Self::get_text(n, code))
            .unwrap_or_else(|| "<anonymous>".to_string())
    }

    fn get_impl_target(node: Node, code: &str) -> String {
        node.child_by_field_name("type")
            .map(|n| Self::get_text(n, code))
            .unwrap_or_else(|| "<unknown>".to_string())
    }

    fn get_text(node: Node, code: &str) -> String {
        code[node.byte_range()].to_string()
    }

    /// Extract relationships (TODO: implement)
    pub fn extract_relationships(_code: &str) -> Vec<ExtractedRelationship> {
        // TODO: Implement relationship extraction
        vec![]
    }
}
