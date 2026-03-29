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

    fn extract_usages(node: Node, code: &str) -> Vec<(SymbolName, usize)> {
        // Only extract usages from function/method bodies
        let body = match node.kind() {
            "function_declaration" | "method_declaration" => node.child_by_field_name("body"),
            _ => None,
        };
        let Some(body) = body else {
            return Vec::new();
        };

        // Collect local declarations to exclude them from usages
        let mut locals = std::collections::HashSet::new();

        // Collect parameter names
        if let Some(params) = node.child_by_field_name("parameters") {
            collect_param_names(params, code, &mut locals);
        }
        // Collect receiver name for methods
        if let Some(receiver) = node.child_by_field_name("receiver") {
            collect_param_names(receiver, code, &mut locals);
        }

        // Collect local variable declarations from the body
        collect_local_declarations(body, code, &mut locals);

        // Scan the body for identifier usages, excluding locals and builtins
        let mut usages = Vec::new();
        let mut seen = std::collections::HashSet::new();
        collect_identifier_usages(body, code, &locals, &mut usages, &mut seen);
        usages
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

/// Check if an identifier is a Go built-in function or value
fn is_builtin_identifier(name: &str) -> bool {
    matches!(
        name,
        "append"
            | "cap"
            | "clear"
            | "close"
            | "complex"
            | "copy"
            | "delete"
            | "imag"
            | "len"
            | "make"
            | "max"
            | "min"
            | "new"
            | "panic"
            | "print"
            | "println"
            | "real"
            | "recover"
            | "true"
            | "false"
            | "nil"
            | "iota"
    )
}

/// Collect parameter names from a parameter_list node
fn collect_param_names(
    param_list: Node,
    code: &str,
    locals: &mut std::collections::HashSet<String>,
) {
    for child in param_list.children(&mut param_list.walk()) {
        if child.kind() == "parameter_declaration" {
            for param_child in child.children(&mut child.walk()) {
                if param_child.kind() == "identifier" {
                    locals.insert(node_text(param_child, code));
                }
            }
        }
    }
}

/// Recursively collect local variable/constant declarations from a block
fn collect_local_declarations(
    node: Node,
    code: &str,
    locals: &mut std::collections::HashSet<String>,
) {
    match node.kind() {
        // short_var_declaration: `x, y := expr`
        "short_var_declaration" => {
            if let Some(left) = node.child_by_field_name("left") {
                for child in left.children(&mut left.walk()) {
                    if child.kind() == "identifier" {
                        locals.insert(node_text(child, code));
                    }
                }
            }
            return;
        }
        // var_spec inside a function: `var x int = 5`
        "var_spec" => {
            if let Some(name) = node.child_by_field_name("name") {
                locals.insert(node_text(name, code));
            }
            return;
        }
        // range clause: `for k, v := range items`
        "range_clause" => {
            if let Some(left) = node.child_by_field_name("left") {
                for child in left.children(&mut left.walk()) {
                    if child.kind() == "identifier" {
                        locals.insert(node_text(child, code));
                    }
                }
            }
            return;
        }
        _ => {}
    }

    for child in node.children(&mut node.walk()) {
        if child.kind() != "func_literal" {
            collect_local_declarations(child, code, locals);
        }
    }
}

/// Collect identifier usages from a function body, excluding locals and builtins.
/// Only collects `identifier` nodes (not `type_identifier` — those are types).
/// Deduplicates by name (one usage edge per referenced symbol, not per occurrence).
fn collect_identifier_usages(
    node: Node,
    code: &str,
    locals: &std::collections::HashSet<String>,
    usages: &mut Vec<(SymbolName, usize)>,
    seen: &mut std::collections::HashSet<String>,
) {
    if node.kind() == "identifier" {
        let name = node_text(node, code);
        if !locals.contains(&name)
            && !is_builtin_identifier(&name)
            && !is_builtin_type(&name)
            && !seen.contains(&name)
            && name.len() > 1
            && !is_definition_position(node)
        {
            seen.insert(name.clone());
            let line = node.start_position().row + 1;
            usages.push((SymbolName::new(name), line));
        }
        return;
    }

    // Don't recurse into nested function literals
    if node.kind() == "func_literal" {
        return;
    }

    for child in node.children(&mut node.walk()) {
        collect_identifier_usages(child, code, locals, usages, seen);
    }
}

/// Check if an identifier is in a definition/assignment position
fn is_definition_position(node: Node) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    match parent.kind() {
        "assignment_statement" => {
            if let Some(left) = parent.child_by_field_name("left") {
                is_ancestor_of(left, node)
            } else {
                false
            }
        }
        "selector_expression" => {
            if let Some(field) = parent.child_by_field_name("field") {
                node.id() == field.id()
            } else {
                false
            }
        }
        "labeled_statement" => {
            if let Some(label) = parent.child_by_field_name("label") {
                node.id() == label.id()
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Check if `ancestor` is an ancestor of (or is) `node`
fn is_ancestor_of(ancestor: Node, node: Node) -> bool {
    if ancestor.id() == node.id() {
        return true;
    }
    for child in ancestor.children(&mut ancestor.walk()) {
        if is_ancestor_of(child, node) {
            return true;
        }
    }
    false
}
