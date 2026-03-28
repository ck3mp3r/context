// Rust language parser implementation

use super::types::Kind;
use crate::analysis::parser::{ImplInfo, Language};
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
            "function_item" => extract_name(node, code, "identifier", Kind::Function),
            "struct_item" => extract_name(node, code, "type_identifier", Kind::Struct),
            "enum_item" => extract_name(node, code, "type_identifier", Kind::Enum),
            "trait_item" => extract_name(node, code, "type_identifier", Kind::Trait),
            "const_item" => extract_name(node, code, "identifier", Kind::Const),
            "static_item" => extract_name(node, code, "identifier", Kind::Static),
            "type_item" => extract_name(node, code, "type_identifier", Kind::Type),
            "mod_item" => extract_name(node, code, "identifier", Kind::Mod),
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
                // Scoped call: Foo::bar() - we want the last segment (bar)
                "scoped_identifier" => {
                    let mut last_ident = None;
                    for subchild in child.children(&mut child.walk()) {
                        if subchild.kind() == "identifier" {
                            last_ident = Some(code[subchild.byte_range()].to_string());
                        }
                    }
                    return last_ident;
                }
                // Method call: obj.method()
                "field_expression" => {
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

    fn parse_impl(node: Node, code: &str) -> Option<ImplInfo> {
        if node.kind() != "impl_item" {
            return None;
        }

        let mut target_type = None;
        let mut trait_name = None;
        let mut has_for = false;

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "for" => {
                    has_for = true;
                }
                "type_identifier" if has_for || (trait_name.is_none() && target_type.is_none()) => {
                    target_type = Some(code[child.byte_range()].to_string());
                }
                "generic_type" if has_for || (trait_name.is_none() && target_type.is_none()) => {
                    if let Some(name) = extract_type_name_from_generic(child, code) {
                        target_type = Some(name);
                    }
                }
                _ => {}
            }
        }

        // If we saw `for`, the first type_identifier was the trait
        if has_for && target_type.is_some() {
            // Walk again to get trait name (first type_identifier or generic_type before `for`)
            let mut found_for = false;
            for child in node.children(&mut node.walk()) {
                if child.kind() == "for" {
                    found_for = true;
                } else if !found_for {
                    match child.kind() {
                        "type_identifier" => {
                            trait_name = Some(code[child.byte_range()].to_string());
                            break;
                        }
                        "generic_type" => {
                            if let Some(name) = extract_type_name_from_generic(child, code) {
                                trait_name = Some(name);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        target_type.map(|t| ImplInfo {
            target_type: t,
            trait_name,
        })
    }

    fn extract_type_references(node: Node, code: &str) -> Vec<(String, String)> {
        let mut refs = Vec::new();

        // Only extract from function/method signatures and struct fields
        match node.kind() {
            "function_item" => {
                // Look at parameters and return type for type_identifier
                collect_type_refs(node, code, &mut refs, "type_annotation");
            }
            "struct_item" => {
                // Look at field types
                collect_type_refs(node, code, &mut refs, "field_type");
            }
            _ => {}
        }

        refs
    }

    fn extract_signature(node: Node, code: &str) -> Option<String> {
        match node.kind() {
            "function_item" => {
                // Signature = everything before the body block
                // Find the block (function body) and take everything before it
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        let sig_end = child.start_byte();
                        let sig = code[node.start_byte()..sig_end].trim();
                        return Some(sig.to_string());
                    }
                }
                // No body (shouldn't happen for function_item, but safety)
                Some(code[node.byte_range()].to_string())
            }
            "struct_item" => {
                // For structs, include the whole declaration
                let text = &code[node.byte_range()];
                // Truncate very long struct definitions
                if text.len() > 200 {
                    Some(format!("{}...", &text[..200]))
                } else {
                    Some(text.to_string())
                }
            }
            "enum_item" => {
                let text = &code[node.byte_range()];
                if text.len() > 200 {
                    Some(format!("{}...", &text[..200]))
                } else {
                    Some(text.to_string())
                }
            }
            "trait_item" => {
                // Just the trait header, not the body
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "declaration_list" {
                        let sig_end = child.start_byte();
                        let sig = code[node.start_byte()..sig_end].trim();
                        return Some(sig.to_string());
                    }
                }
                Some(code[node.byte_range()].to_string())
            }
            "type_item" => Some(code[node.byte_range()].trim().to_string()),
            "const_item" => Some(code[node.byte_range()].trim().to_string()),
            "static_item" => Some(code[node.byte_range()].trim().to_string()),
            _ => None,
        }
    }

    fn name() -> &'static str {
        "rust"
    }

    fn extensions() -> &'static [&'static str] {
        &["rs"]
    }
}

/// Helper: extract the first child node of `child_kind` as the symbol name
fn extract_name(node: Node, code: &str, child_kind: &str, kind: Kind) -> Option<(Kind, String)> {
    for child in node.children(&mut node.walk()) {
        if child.kind() == child_kind {
            let name = code[child.byte_range()].to_string();
            return Some((kind, name));
        }
    }
    None
}

/// Extract the base type_identifier from a generic_type node.
/// e.g. `SqliteProjectRepository<'a>` -> "SqliteProjectRepository"
fn extract_type_name_from_generic(node: Node, code: &str) -> Option<String> {
    for child in node.children(&mut node.walk()) {
        if child.kind() == "type_identifier" {
            return Some(code[child.byte_range()].to_string());
        }
    }
    None
}

/// Built-in types that should not generate References edges
const BUILTIN_TYPES: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32",
    "f64", "bool", "char", "str", "String", "Self", "Vec", "Option", "Result", "Box", "Rc", "Arc",
    "HashMap", "HashSet", "BTreeMap", "BTreeSet", "Cow", "Pin", "Fn", "FnMut", "FnOnce",
];

/// Recursively collect type_identifier nodes from a subtree, skipping builtins
fn collect_type_refs(node: Node, code: &str, refs: &mut Vec<(String, String)>, ref_kind: &str) {
    if node.kind() == "type_identifier" {
        let name = code[node.byte_range()].to_string();
        if !BUILTIN_TYPES.contains(&name.as_str()) {
            refs.push((name, ref_kind.to_string()));
        }
    }
    for child in node.children(&mut node.walk()) {
        collect_type_refs(child, code, refs, ref_kind);
    }
}
