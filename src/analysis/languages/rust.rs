// Rust symbol extraction using tree-sitter-rust
//
// Extracts Rust-specific symbols: functions, structs, traits, impls, enums, etc.

use crate::analysis::extractor::SymbolExtractor;
use crate::analysis::parser::Parser;
use crate::analysis::types::{ExtractedRelationship, ExtractedSymbol, SymbolKind};
use tree_sitter::Node;

/// Rust-specific symbol extractor
pub struct RustExtractor;

impl SymbolExtractor for RustExtractor {
    fn extract_symbols(&self, code: &str, file_path: &str) -> Vec<ExtractedSymbol> {
        let mut parser = match Parser::new_rust() {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        let tree = match parser.parse(code) {
            Some(t) => t,
            None => return vec![],
        };

        let mut symbols = Vec::new();
        walk_node(tree.root_node(), code, file_path, &mut symbols);
        symbols
    }

    fn extract_relationships(&self, code: &str, file_path: &str) -> Vec<ExtractedRelationship> {
        // Phase 2: Implement call graph, references, etc.
        let _ = (code, file_path);
        vec![]
    }
}

impl RustExtractor {
    /// Convenience method for backward compatibility with tests
    pub fn extract(code: &str, file_path: &str) -> Vec<ExtractedSymbol> {
        let extractor = RustExtractor;
        extractor.extract_symbols(code, file_path)
    }
}

// Helper functions (module-level to avoid nested function issues with Self)

fn walk_node(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    match node.kind() {
        "function_item" => {
            extract_function(node, code, file_path, symbols);
        }
        "struct_item" => {
            extract_struct(node, code, file_path, symbols);
        }
        "trait_item" => {
            extract_trait(node, code, file_path, symbols);
        }
        "impl_item" => {
            extract_impl(node, code, file_path, symbols);
        }
        "enum_item" => {
            extract_enum(node, code, file_path, symbols);
        }
        _ => {
            // Recurse into children
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(child, code, file_path, symbols);
            }
        }
    }
}

fn extract_function(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    let name = get_name(node, code);
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let content = get_text(node, code);

    symbols.push(ExtractedSymbol {
        name,
        kind: SymbolKind::Function,
        file_path: file_path.to_string(),
        start_line,
        end_line,
        content,
        signature: None, // TODO: Extract function signature
    });
}

fn extract_struct(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    let name = get_name(node, code);
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let content = get_text(node, code);

    symbols.push(ExtractedSymbol {
        name,
        kind: SymbolKind::Struct,
        file_path: file_path.to_string(),
        start_line,
        end_line,
        content,
        signature: None,
    });
}

fn extract_trait(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    let name = get_name(node, code);
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let content = get_text(node, code);

    symbols.push(ExtractedSymbol {
        name,
        kind: SymbolKind::Trait,
        file_path: file_path.to_string(),
        start_line,
        end_line,
        content,
        signature: None,
    });
}

fn extract_impl(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    let target_type = get_impl_target(node, code);
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let content = get_text(node, code);

    symbols.push(ExtractedSymbol {
        name: format!("impl {}", target_type),
        kind: SymbolKind::Impl {
            target_type: target_type.clone(),
        },
        file_path: file_path.to_string(),
        start_line,
        end_line,
        content,
        signature: None,
    });
}

fn extract_enum(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    let name = get_name(node, code);
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let content = get_text(node, code);

    symbols.push(ExtractedSymbol {
        name,
        kind: SymbolKind::Enum,
        file_path: file_path.to_string(),
        start_line,
        end_line,
        content,
        signature: None,
    });
}

fn get_name(node: Node, code: &str) -> String {
    node.child_by_field_name("name")
        .map(|n| get_text(n, code))
        .unwrap_or_else(|| "<anonymous>".to_string())
}

fn get_impl_target(node: Node, code: &str) -> String {
    node.child_by_field_name("type")
        .map(|n| get_text(n, code))
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn get_text(node: Node, code: &str) -> String {
    code[node.byte_range()].to_string()
}
