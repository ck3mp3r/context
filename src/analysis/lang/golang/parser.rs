// Go language parser implementation

use super::types::Kind;
use crate::analysis::parser::{ImplInfo, Language};
use crate::analysis::types::{ReferenceType, SymbolName};
use tree_sitter::Node;

/// Go language implementation
pub struct Go;

impl Language for Go {
    type Kind = Kind;

    fn grammar() -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn parse_symbol(node: Node, code: &str) -> Option<(Self::Kind, String)> {
        let result = match node.kind() {
            "function_declaration" => {
                let name = node.child_by_field_name("name")?;
                Some((Kind::Function, node_text(name, code)))
            }
            "method_declaration" => {
                let name = node.child_by_field_name("name")?;
                Some((Kind::Method, node_text(name, code)))
            }
            "type_declaration" => parse_type_declaration(node, code),
            "const_spec" => {
                let name = node.child_by_field_name("name")?;
                Some((Kind::Const, node_text(name, code)))
            }
            "var_spec" => {
                let name = node.child_by_field_name("name")?;
                Some((Kind::Var, node_text(name, code)))
            }
            _ => None,
        };

        // Filter out Go's blank identifier `_` (used for interface conformance checks, etc.)
        result.filter(|(_, name)| name != "_")
    }

    fn extract_callee(node: Node, code: &str) -> Option<String> {
        // Go call_expression has a "function" field
        let func = node.child_by_field_name("function")?;
        Some(node_text(func, code))
    }

    fn parse_impl(_node: Node, _code: &str) -> Option<ImplInfo> {
        // Go doesn't have impl blocks — methods use receiver syntax
        None
    }

    fn extract_type_references(node: Node, code: &str) -> Vec<(SymbolName, ReferenceType)> {
        let mut refs = Vec::new();
        collect_type_identifiers(node, code, &mut refs);
        refs
    }

    fn name() -> &'static str {
        "go"
    }

    fn extensions() -> &'static [&'static str] {
        &["go"]
    }

    fn extract_signature(node: Node, code: &str) -> Option<String> {
        match node.kind() {
            "function_declaration" => extract_function_signature(node, code),
            "method_declaration" => extract_method_signature(node, code),
            _ => None,
        }
    }
}

/// Extract text from a node
fn node_text(node: Node, code: &str) -> String {
    code[node.byte_range()].to_string()
}

/// Parse a type_declaration to determine if it's a struct, interface, or type alias.
/// type_declaration contains one or more type_spec children.
fn parse_type_declaration(node: Node, code: &str) -> Option<(Kind, String)> {
    // type_declaration wraps type_spec(s)
    // For single declarations: type Foo struct { ... }
    // For grouped declarations: type ( Foo struct { ... }; Bar int )
    // We handle type_spec at the child level, but tree-sitter gives us
    // type_declaration as the top node. Look for the first type_spec.
    for child in node.children(&mut node.walk()) {
        if child.kind() == "type_spec" {
            return parse_type_spec(child, code);
        }
    }
    None
}

/// Parse a single type_spec: `name type_body`
fn parse_type_spec(node: Node, code: &str) -> Option<(Kind, String)> {
    let name = node.child_by_field_name("name")?;
    let name_str = node_text(name, code);

    let type_node = node.child_by_field_name("type")?;
    let kind = match type_node.kind() {
        "struct_type" => Kind::Struct,
        "interface_type" => Kind::Interface,
        _ => Kind::TypeAlias,
    };

    Some((kind, name_str))
}

/// Extract function signature: `func name(params) return_type`
fn extract_function_signature(node: Node, code: &str) -> Option<String> {
    let name = node.child_by_field_name("name")?;
    let params = node.child_by_field_name("parameters")?;

    let mut sig = format!("func {}{}", node_text(name, code), node_text(params, code));

    if let Some(result) = node.child_by_field_name("result") {
        sig.push(' ');
        sig.push_str(&node_text(result, code));
    }

    Some(sig)
}

/// Extract method signature: `func (receiver) name(params) return_type`
fn extract_method_signature(node: Node, code: &str) -> Option<String> {
    let name = node.child_by_field_name("name")?;
    let params = node.child_by_field_name("parameters")?;

    // Receiver is the first parameter_list before the name
    let receiver = node.child_by_field_name("receiver")?;

    let mut sig = format!(
        "func {} {}{}",
        node_text(receiver, code),
        node_text(name, code),
        node_text(params, code),
    );

    if let Some(result) = node.child_by_field_name("result") {
        sig.push(' ');
        sig.push_str(&node_text(result, code));
    }

    Some(sig)
}

/// Recursively collect type_identifier references from a node's subtree.
/// Skips the "name" field of type_spec to avoid self-references.
fn collect_type_identifiers(node: Node, code: &str, refs: &mut Vec<(SymbolName, ReferenceType)>) {
    if node.kind() == "type_identifier" {
        let name = node_text(node, code);
        // Skip built-in types
        if !is_builtin_type(&name) {
            refs.push((SymbolName::new(name), ReferenceType::Usage));
        }
        return;
    }

    for child in node.children(&mut node.walk()) {
        // Skip the "name" field of type_spec declarations to avoid self-refs
        if node.kind() == "type_spec"
            && let Some(name_node) = node.child_by_field_name("name")
            && child.id() == name_node.id()
        {
            continue;
        }
        collect_type_identifiers(child, code, refs);
    }
}

/// Check if a type name is a Go built-in type
fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "byte"
            | "complex64"
            | "complex128"
            | "error"
            | "float32"
            | "float64"
            | "int"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "rune"
            | "string"
            | "uint"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "uintptr"
            | "any"
            | "comparable"
    )
}
