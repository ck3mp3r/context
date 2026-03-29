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

    fn parse_impl(node: Node, code: &str) -> Option<ImplInfo> {
        // Go doesn't have impl blocks, but uses conformance check patterns:
        //   var _ Interface = (*Type)(nil)
        //   var _ Interface = &Type{}
        // These are var_spec nodes with name "_" and an explicit type (the interface)
        if node.kind() != "var_spec" {
            return None;
        }

        // Must be blank identifier
        let name = node.child_by_field_name("name")?;
        if node_text(name, code) != "_" {
            return None;
        }

        // The type field is the interface name
        let type_node = node.child_by_field_name("type")?;
        let interface_name = extract_type_name(type_node, code)?;

        // The value field contains the concrete type, wrapped in expression_list
        let value_node = node.child_by_field_name("value")?;
        // Unwrap expression_list to get the actual expression
        let expr = if value_node.kind() == "expression_list" {
            value_node.child(0)?
        } else {
            value_node
        };
        let concrete_type = extract_conformance_type(expr, code)?;

        Some(ImplInfo {
            target_type: concrete_type,
            trait_name: Some(interface_name),
        })
    }

    fn extract_type_references(node: Node, code: &str) -> Vec<(SymbolName, ReferenceType)> {
        let mut refs = Vec::new();
        if node.kind() == "type_declaration" {
            // Determine if this is a struct or interface to assign correct edge type
            for child in node.children(&mut node.walk()) {
                if child.kind() == "type_spec"
                    && let Some(type_body) = child.child_by_field_name("type")
                {
                    let ref_kind = match type_body.kind() {
                        "struct_type" => ReferenceType::FieldType,
                        "interface_type" => ReferenceType::TypeAnnotation,
                        _ => ReferenceType::FieldType,
                    };
                    collect_type_identifiers_with_kind(child, code, &mut refs, &ref_kind);
                }
            }
        }
        // function/method type refs are handled by extract_return_types
        // and extract_param_types — don't duplicate them here
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

    fn extract_return_types(node: Node, code: &str) -> Vec<SymbolName> {
        match node.kind() {
            "function_declaration" | "method_declaration" => {}
            _ => return Vec::new(),
        }

        let Some(result) = node.child_by_field_name("result") else {
            return Vec::new();
        };

        let mut types = Vec::new();
        collect_return_type_identifiers(result, code, &mut types);
        types
    }

    fn extract_param_types(node: Node, code: &str) -> Vec<SymbolName> {
        match node.kind() {
            "function_declaration" | "method_declaration" => {}
            _ => return Vec::new(),
        }

        let Some(params) = node.child_by_field_name("parameters") else {
            return Vec::new();
        };

        let mut types = Vec::new();
        // Reuse the same collector as return types — it handles
        // type_identifier, pointer_type, and parameter_declaration
        collect_return_type_identifiers(params, code, &mut types);
        types
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

/// Extract a type name from a type node (type_identifier or pointer_type or qualified_type)
fn extract_type_name(node: Node, code: &str) -> Option<String> {
    match node.kind() {
        "type_identifier" => Some(node_text(node, code)),
        "pointer_type" => {
            // *Type — recurse into the inner type
            for child in node.children(&mut node.walk()) {
                if child.kind() == "type_identifier" {
                    return Some(node_text(child, code));
                }
            }
            None
        }
        "qualified_type" => {
            // pkg.Type — extract the type part
            for child in node.children(&mut node.walk()) {
                if child.kind() == "type_identifier" {
                    return Some(node_text(child, code));
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the concrete type from a conformance check value expression.
/// Handles:
///   (*Type)(nil)  — call_expression wrapping parenthesized unary_expression (*Type)
///   &Type{}       — unary_expression with address-of composite literal
fn extract_conformance_type(node: Node, code: &str) -> Option<String> {
    match node.kind() {
        // (*FileBasedCache)(nil) is a call_expression
        "call_expression" => {
            let func = node.child_by_field_name("function")?;
            extract_conformance_type(func, code)
        }
        // (*FileBasedCache) is a parenthesized_expression containing unary_expression *Type
        "parenthesized_expression" => {
            for child in node.children(&mut node.walk()) {
                match child.kind() {
                    // tree-sitter-go represents *Type in expressions as unary_expression
                    "unary_expression" => {
                        if let Some(operand) = child.child_by_field_name("operand")
                            && operand.kind() == "identifier"
                        {
                            return Some(node_text(operand, code));
                        }
                    }
                    "type_identifier" | "identifier" => {
                        return Some(node_text(child, code));
                    }
                    _ => {}
                }
            }
            None
        }
        // &FileBasedCache{} is a unary_expression with & operator
        "unary_expression" => {
            if let Some(operand) = node.child_by_field_name("operand")
                && operand.kind() == "composite_literal"
            {
                // composite_literal has a type field
                if let Some(type_node) = operand.child_by_field_name("type") {
                    return extract_type_name(type_node, code);
                }
            }
            None
        }
        _ => None,
    }
}

/// Recursively collect type_identifier nodes from a Go return type subtree.
/// Handles single types, pointer types, and parameter_list (multiple/named returns).
/// Skips builtin types.
fn collect_return_type_identifiers(node: Node, code: &str, types: &mut Vec<SymbolName>) {
    match node.kind() {
        "type_identifier" => {
            let name = node_text(node, code);
            if !is_builtin_type(&name) {
                types.push(SymbolName::new(name));
            }
        }
        "pointer_type" => {
            // *Config — recurse to get the inner type
            for child in node.children(&mut node.walk()) {
                collect_return_type_identifiers(child, code, types);
            }
        }
        "parameter_list" => {
            // (Config, error) or (result Config, err error)
            for child in node.children(&mut node.walk()) {
                collect_return_type_identifiers(child, code, types);
            }
        }
        "parameter_declaration" => {
            // Named return: `result Config` — extract from type field
            if let Some(type_node) = node.child_by_field_name("type") {
                collect_return_type_identifiers(type_node, code, types);
            }
        }
        _ => {}
    }
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

/// Recursively collect type_identifier references from a node's subtree,
/// assigning the given reference kind.
/// Skips the "name" field of type_spec to avoid self-references.
fn collect_type_identifiers_with_kind(
    node: Node,
    code: &str,
    refs: &mut Vec<(SymbolName, ReferenceType)>,
    ref_kind: &ReferenceType,
) {
    if node.kind() == "type_identifier" {
        let name = node_text(node, code);
        // Skip built-in types
        if !is_builtin_type(&name) {
            refs.push((SymbolName::new(name), ref_kind.clone()));
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
        collect_type_identifiers_with_kind(child, code, refs, ref_kind);
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
