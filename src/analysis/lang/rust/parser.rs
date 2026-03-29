// Rust language parser implementation

use super::types::Kind;
use crate::analysis::parser::{ImplInfo, Language};
use crate::analysis::types::{ReferenceType, SymbolName};
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

    fn extract_type_references(node: Node, code: &str) -> Vec<(SymbolName, ReferenceType)> {
        let mut refs = Vec::new();

        match node.kind() {
            // Struct fields → FieldType edges
            "struct_item" => {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "field_declaration_list" {
                        collect_type_refs(child, code, &mut refs, ReferenceType::FieldType);
                    }
                }
            }
            // Trait method signatures → TypeAnnotation edges
            // (consistent with Go interface method types)
            "trait_item" => {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "declaration_list" {
                        collect_type_refs(child, code, &mut refs, ReferenceType::TypeAnnotation);
                    }
                }
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

    fn extract_usages(node: Node, code: &str) -> Vec<(SymbolName, usize)> {
        // Only extract usages from function bodies
        if node.kind() != "function_item" {
            return Vec::new();
        }
        let Some(body) = node
            .children(&mut node.walk())
            .find(|c| c.kind() == "block")
        else {
            return Vec::new();
        };

        let mut locals = std::collections::HashSet::new();

        // Collect parameter names
        if let Some(params) = node.child_by_field_name("parameters") {
            collect_rust_param_names(params, code, &mut locals);
        }

        // Collect local bindings from the body
        collect_rust_local_declarations(body, code, &mut locals);

        // Scan the body for identifier usages
        let mut usages = Vec::new();
        let mut seen = std::collections::HashSet::new();
        collect_rust_identifier_usages(body, code, &locals, &mut usages, &mut seen);
        usages
    }

    fn extract_return_types(node: Node, code: &str) -> Vec<SymbolName> {
        if node.kind() != "function_item" {
            return Vec::new();
        }

        // Find the return type node — it's the `type` field after `->`
        let Some(return_type) = node.child_by_field_name("return_type") else {
            return Vec::new();
        };

        let mut types = Vec::new();
        collect_return_type_names(return_type, code, &mut types);
        types
    }

    fn extract_param_types(node: Node, code: &str) -> Vec<SymbolName> {
        if node.kind() != "function_item" {
            return Vec::new();
        }

        let Some(params) = node.child_by_field_name("parameters") else {
            return Vec::new();
        };

        let mut types = Vec::new();
        for child in params.children(&mut params.walk()) {
            if child.kind() == "parameter" {
                // Extract type from the "type" field, skipping self parameters
                if let Some(type_node) = child.child_by_field_name("type") {
                    collect_return_type_names(type_node, code, &mut types);
                }
            }
        }
        types
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
fn collect_type_refs(
    node: Node,
    code: &str,
    refs: &mut Vec<(SymbolName, ReferenceType)>,
    ref_kind: ReferenceType,
) {
    if node.kind() == "type_identifier" {
        let name = code[node.byte_range()].to_string();
        if !BUILTIN_TYPES.contains(&name.as_str()) {
            refs.push((SymbolName::new(name), ref_kind.clone()));
        }
    }
    for child in node.children(&mut node.walk()) {
        collect_type_refs(child, code, refs, ref_kind.clone());
    }
}

/// Recursively collect type_identifier nodes from a return type subtree, skipping builtins
fn collect_return_type_names(node: Node, code: &str, types: &mut Vec<SymbolName>) {
    if node.kind() == "type_identifier" {
        let name = code[node.byte_range()].to_string();
        if !BUILTIN_TYPES.contains(&name.as_str()) {
            types.push(SymbolName::new(name));
        }
        return;
    }
    for child in node.children(&mut node.walk()) {
        collect_return_type_names(child, code, types);
    }
}

// ============================================================================
// Usage extraction helpers
// ============================================================================

/// Built-in identifiers/functions that should not generate usage edges
const BUILTIN_IDENTIFIERS: &[&str] = &[
    "println",
    "print",
    "eprintln",
    "eprint",
    "format",
    "write",
    "writeln",
    "todo",
    "unimplemented",
    "unreachable",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "cfg",
    "include",
    "include_str",
    "include_bytes",
    "env",
    "concat",
    "stringify",
    "line",
    "column",
    "file",
    "module_path",
    "Ok",
    "Err",
    "Some",
    "None",
    "true",
    "false",
];

/// Collect parameter names from a Rust function's parameters node
fn collect_rust_param_names(
    params: Node,
    code: &str,
    locals: &mut std::collections::HashSet<String>,
) {
    for child in params.children(&mut params.walk()) {
        if child.kind() == "parameter" {
            // parameter has a pattern child (identifier or destructuring)
            if let Some(pat) = child.child_by_field_name("pattern") {
                collect_pattern_names(pat, code, locals);
            }
        }
    }
}

/// Collect names from a pattern (identifier, tuple pattern, struct pattern, etc.)
fn collect_pattern_names(node: Node, code: &str, locals: &mut std::collections::HashSet<String>) {
    match node.kind() {
        "identifier" => {
            locals.insert(code[node.byte_range()].to_string());
        }
        "tuple_pattern" | "slice_pattern" | "or_pattern" => {
            for child in node.children(&mut node.walk()) {
                collect_pattern_names(child, code, locals);
            }
        }
        "struct_pattern" => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "field_pattern" {
                    // The name in a field pattern
                    if let Some(pat) = child.child_by_field_name("pattern") {
                        collect_pattern_names(pat, code, locals);
                    } else {
                        // Shorthand: `Struct { field }` — field is both name and binding
                        if let Some(name) = child.child_by_field_name("name") {
                            locals.insert(code[name.byte_range()].to_string());
                        }
                    }
                }
            }
        }
        "ref_pattern" | "mut_pattern" => {
            for child in node.children(&mut node.walk()) {
                collect_pattern_names(child, code, locals);
            }
        }
        "tuple_struct_pattern" => {
            for child in node.children(&mut node.walk()) {
                if child.kind() != "identifier"
                    || child.id()
                        == node
                            .children(&mut node.walk())
                            .next()
                            .map(|c| c.id())
                            .unwrap_or(0)
                {
                    // First identifier is the type name, skip it
                    // Remaining children are the patterns inside
                } else {
                    collect_pattern_names(child, code, locals);
                }
            }
        }
        _ => {}
    }
}

/// Extract pattern names from a let_condition or let_chain
fn collect_let_condition_names(
    node: Node,
    code: &str,
    locals: &mut std::collections::HashSet<String>,
) {
    match node.kind() {
        "let_condition" => {
            if let Some(pat) = node.child_by_field_name("pattern") {
                collect_pattern_names(pat, code, locals);
            }
        }
        "let_chain" => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "let_condition"
                    && let Some(pat) = child.child_by_field_name("pattern")
                {
                    collect_pattern_names(pat, code, locals);
                }
            }
        }
        _ => {}
    }
}

/// Recursively collect local variable declarations from a Rust block
fn collect_rust_local_declarations(
    node: Node,
    code: &str,
    locals: &mut std::collections::HashSet<String>,
) {
    match node.kind() {
        "let_declaration" => {
            if let Some(pat) = node.child_by_field_name("pattern") {
                collect_pattern_names(pat, code, locals);
            }
            return;
        }
        "for_expression" => {
            // `for item in iter { ... }` — item is a pattern
            if let Some(pat) = node.child_by_field_name("pattern") {
                collect_pattern_names(pat, code, locals);
            }
            // Recurse into body but not the pattern
            if let Some(body) = node.child_by_field_name("body") {
                collect_rust_local_declarations(body, code, locals);
            }
            return;
        }
        "if_expression" => {
            // `if let Some(x) = expr { ... }` — x is local
            if let Some(condition) = node.child_by_field_name("condition") {
                collect_let_condition_names(condition, code, locals);
            }
            // Continue recursing for nested blocks
        }
        "match_arm" => {
            if let Some(pat) = node.child_by_field_name("pattern") {
                collect_pattern_names(pat, code, locals);
            }
        }
        _ => {}
    }

    for child in node.children(&mut node.walk()) {
        // Don't recurse into closures (they have their own scope)
        if child.kind() != "closure_expression" {
            collect_rust_local_declarations(child, code, locals);
        }
    }
}

/// Collect identifier usages from a Rust function body
fn collect_rust_identifier_usages(
    node: Node,
    code: &str,
    locals: &std::collections::HashSet<String>,
    usages: &mut Vec<(SymbolName, usize)>,
    seen: &mut std::collections::HashSet<String>,
) {
    if node.kind() == "identifier" {
        let name = code[node.byte_range()].to_string();
        if !locals.contains(&name)
            && !BUILTIN_IDENTIFIERS.contains(&name.as_str())
            && !BUILTIN_TYPES.contains(&name.as_str())
            && !seen.contains(&name)
            && name.len() > 1
            && !is_rust_definition_position(node)
        {
            seen.insert(name.clone());
            let line = node.start_position().row + 1;
            usages.push((SymbolName::new(name), line));
        }
        return;
    }

    // Don't recurse into closures
    if node.kind() == "closure_expression" {
        return;
    }

    for child in node.children(&mut node.walk()) {
        collect_rust_identifier_usages(child, code, locals, usages, seen);
    }
}

/// Check if a Rust identifier is in a definition position
fn is_rust_definition_position(node: Node) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    match parent.kind() {
        // Part of a let pattern
        "let_declaration" => {
            if let Some(pat) = parent.child_by_field_name("pattern") {
                node.id() == pat.id() || is_rust_ancestor_of(pat, node)
            } else {
                false
            }
        }
        // Field access: `obj.field` — field is not a usage of a standalone identifier
        "field_expression" => {
            if let Some(field) = parent.child_by_field_name("field") {
                node.id() == field.id()
            } else {
                false
            }
        }
        // Scoped identifier: `module::item` — only the last segment matters
        // but we let these through since they reference module-level items
        _ => false,
    }
}

/// Check if `ancestor` contains `node`
fn is_rust_ancestor_of(ancestor: Node, node: Node) -> bool {
    if ancestor.id() == node.id() {
        return true;
    }
    for child in ancestor.children(&mut ancestor.walk()) {
        if is_rust_ancestor_of(child, node) {
            return true;
        }
    }
    false
}
