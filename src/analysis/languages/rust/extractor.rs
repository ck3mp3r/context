// Rust symbol extraction using tree-sitter-rust
//
// Extracts Rust-specific symbols: functions, structs, traits, impls, enums, etc.

use crate::analysis::extractor::SymbolExtractor;
use crate::analysis::parser::Parser;
use crate::analysis::types::{
    ExtractedRelationship, ExtractedSymbol, InheritanceType, ReferenceType, RelationType,
    SymbolKind,
};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

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
        let mut parser = match Parser::new_rust() {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        let tree = match parser.parse(code) {
            Some(t) => t,
            None => return vec![],
        };

        let mut relationships = Vec::new();

        extract_calls_with_query(tree.root_node(), code, file_path, &mut relationships);
        extract_type_references_with_query(tree.root_node(), code, file_path, &mut relationships);
        extract_trait_impls_with_query(tree.root_node(), code, file_path, &mut relationships);
        extract_symbol_contains_with_query(tree.root_node(), code, file_path, &mut relationships);

        relationships
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
    let signature = extract_function_signature(node, code);
    push_symbol(
        node,
        code,
        file_path,
        SymbolKind::Function,
        signature,
        symbols,
    );
}

fn extract_struct(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    push_symbol(node, code, file_path, SymbolKind::Struct, None, symbols);
}

fn extract_trait(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    push_symbol(node, code, file_path, SymbolKind::Trait, None, symbols);
}

fn extract_enum(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    push_symbol(node, code, file_path, SymbolKind::Enum, None, symbols);
}

/// Helper to create and push a symbol (reduces duplication)
fn push_symbol(
    node: Node,
    code: &str,
    file_path: &str,
    kind: SymbolKind,
    signature: Option<String>,
    symbols: &mut Vec<ExtractedSymbol>,
) {
    let name = get_name(node, code);
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let content = get_text(node, code);

    symbols.push(ExtractedSymbol {
        name,
        kind,
        file_path: file_path.to_string(),
        start_line,
        end_line,
        content,
        signature,
    });
}

fn extract_impl(node: Node, code: &str, file_path: &str, symbols: &mut Vec<ExtractedSymbol>) {
    let target_type = get_impl_target(node, code);
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;
    let content = get_text(node, code);

    // Extract the impl block itself
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

    // Also extract methods from the impl block
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "function_item" {
                extract_function(child, code, file_path, symbols);
            }
        }
    }
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

fn extract_function_signature(node: Node, code: &str) -> Option<String> {
    // Extract function signature up to the opening brace
    // This is a simplified version - just get everything before the body
    let start = node.start_byte();

    // Find the body (block node)
    if let Some(body) = node.child_by_field_name("body") {
        let body_start = body.start_byte();
        // Get text from function start to body start, trim whitespace
        let sig = code[start..body_start].trim_end().to_string();
        Some(sig)
    } else {
        // No body (e.g., trait method declaration)
        Some(get_text(node, code))
    }
}

// Phase 2: Tree-sitter Query-based relationship extraction

/// Extract function calls using Tree-sitter queries
fn extract_calls_with_query(
    root: Node,
    code: &str,
    file_path: &str,
    relationships: &mut Vec<ExtractedRelationship>,
) {
    let query_source = r#"
        (call_expression
          function: (identifier) @callee) @call
          
        (call_expression
          function: (field_expression
            field: (field_identifier) @callee)) @call
            
        (call_expression
          function: (scoped_identifier
            name: (identifier) @callee)) @call
    "#;

    let language = tree_sitter_rust::LANGUAGE.into();
    let query = match Query::new(&language, query_source) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("Failed to create query: {}", e);
            return;
        }
    };

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, code.as_bytes());

    while let Some(m) = matches.next() {
        let mut call_node = None;
        let mut callee_node = None;

        for capture in m.captures {
            let name = query.capture_names()[capture.index as usize];
            match name {
                "call" => call_node = Some(capture.node),
                "callee" => callee_node = Some(capture.node),
                _ => {}
            }
        }

        if let (Some(call), Some(callee)) = (call_node, callee_node)
            && let Some(rel) = build_call_relationship(call, callee, code, file_path)
        {
            relationships.push(rel);
        }
    }
}

fn build_call_relationship(
    call_node: Node,
    callee_node: Node,
    code: &str,
    file_path: &str,
) -> Option<ExtractedRelationship> {
    let callee_name = get_text(callee_node, code);
    let call_site_line = call_node.start_position().row + 1;
    let caller_name = find_containing_function(call_node, code)?;

    let confidence = match call_node.child(0)?.kind() {
        "scoped_identifier" => 1.0,
        "field_expression" => 0.9,
        _ => 0.8,
    };

    Some(ExtractedRelationship {
        from_symbol_id: format!("symbol:{}:{}:?", file_path, caller_name),
        to_symbol_id: format!("symbol:{}:{}:?", file_path, callee_name),
        relation_type: RelationType::Calls { call_site_line },
        confidence,
    })
}

fn find_containing_function(mut node: Node, code: &str) -> Option<String> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "function_item" {
            return parent
                .child_by_field_name("name")
                .map(|n| get_text(n, code));
        }
        node = parent;
    }
    None
}

/// Extract type references using Tree-sitter queries
fn extract_type_references_with_query(
    root: Node,
    code: &str,
    file_path: &str,
    relationships: &mut Vec<ExtractedRelationship>,
) {
    let query_source = r#"
        (function_item
          name: (identifier) @func.name
          parameters: (parameters
            (parameter
              type: (type_identifier) @param.type)))
              
        (function_item
          name: (identifier) @func.name
          parameters: (parameters
            (parameter
              type: (generic_type
                (type_identifier) @param.type))))
        
        (function_item
          name: (identifier) @func.name
          return_type: (type_identifier) @return.type)
          
        (function_item
          name: (identifier) @func.name
          return_type: (generic_type
            (type_identifier) @return.type))
        
        (let_declaration
          type: (type_identifier) @let.type)
          
        (let_declaration
          type: (generic_type
            (type_identifier) @let.type))
    "#;

    let language = tree_sitter_rust::LANGUAGE.into();
    let query = match Query::new(&language, query_source) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("Failed to create type reference query: {}", e);
            return;
        }
    };

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, code.as_bytes());

    while let Some(m) = matches.next() {
        let mut func_name = None;
        let mut type_refs = Vec::new();

        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            match capture_name {
                "func.name" => func_name = Some(get_text(capture.node, code)),
                "param.type" | "return.type" => {
                    type_refs.push(get_text(capture.node, code));
                }
                "let.type" => {
                    type_refs.push(get_text(capture.node, code));
                    if func_name.is_none()
                        && let Some(func) = find_containing_function(capture.node, code)
                    {
                        func_name = Some(func);
                    }
                }
                _ => {}
            }
        }

        if let Some(from_name) = func_name {
            for type_name in type_refs {
                relationships.push(ExtractedRelationship {
                    from_symbol_id: format!("symbol:{}:{}:?", file_path, from_name),
                    to_symbol_id: format!("symbol:{}:{}:?", file_path, type_name),
                    relation_type: RelationType::References {
                        reference_type: ReferenceType::TypeAnnotation,
                    },
                    confidence: 1.0,
                });
            }
        }
    }
}

/// Extract trait implementations using Tree-sitter queries
fn extract_trait_impls_with_query(
    root: Node,
    code: &str,
    file_path: &str,
    relationships: &mut Vec<ExtractedRelationship>,
) {
    let query_source = r#"
        (impl_item
          trait: (type_identifier) @trait.name
          "for"
          type: (type_identifier) @impl.type)
    "#;

    let language = tree_sitter_rust::LANGUAGE.into();
    let query = match Query::new(&language, query_source) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("Failed to create trait impl query: {}", e);
            return;
        }
    };

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, code.as_bytes());

    while let Some(m) = matches.next() {
        let mut trait_name = None;
        let mut impl_type = None;

        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            match capture_name {
                "trait.name" => trait_name = Some(get_text(capture.node, code)),
                "impl.type" => impl_type = Some(get_text(capture.node, code)),
                _ => {}
            }
        }

        if let (Some(impl_type_name), Some(trait_name_val)) = (impl_type, trait_name) {
            relationships.push(ExtractedRelationship {
                from_symbol_id: format!("symbol:{}:{}:?", file_path, impl_type_name),
                to_symbol_id: format!("symbol:{}:{}:?", file_path, trait_name_val),
                relation_type: RelationType::Inherits {
                    inheritance_type: InheritanceType::Implements,
                },
                confidence: 1.0,
            });
        }
    }
}

/// Extract symbol containment using Tree-sitter queries
/// Links methods to their containing impl blocks
fn extract_symbol_contains_with_query(
    root: Node,
    code: &str,
    file_path: &str,
    relationships: &mut Vec<ExtractedRelationship>,
) {
    let query_source = r#"
        (impl_item
          type: (type_identifier) @impl.type
          body: (declaration_list
            (function_item
              name: (identifier) @method.name)))
    "#;

    let language = tree_sitter_rust::LANGUAGE.into();
    let query = match Query::new(&language, query_source) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("Failed to create symbol contains query: {}", e);
            return;
        }
    };

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, code.as_bytes());

    while let Some(m) = matches.next() {
        let mut impl_type = None;
        let mut method_names = Vec::new();

        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            match capture_name {
                "impl.type" => impl_type = Some(get_text(capture.node, code)),
                "method.name" => method_names.push(get_text(capture.node, code)),
                _ => {}
            }
        }

        if let Some(impl_type_name) = impl_type {
            for method_name in method_names {
                relationships.push(ExtractedRelationship {
                    from_symbol_id: format!("symbol:{}:impl {}:?", file_path, impl_type_name),
                    to_symbol_id: format!("symbol:{}:{}:?", file_path, method_name),
                    relation_type: RelationType::Contains,
                    confidence: 1.0,
                });
            }
        }
    }
}
