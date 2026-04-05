//! Helper functions for Go code analysis.
//!
//! Contains utility functions for determining visibility, entry types,
//! and finding symbols within parsed files.

use crate::analysis::types::{ParsedFile, SymbolId};

/// Go built-in types that should not produce type reference edges.
pub const GO_BUILTINS: &[&str] = &[
    "string",
    "int",
    "int8",
    "int16",
    "int32",
    "int64",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "uintptr",
    "float32",
    "float64",
    "complex64",
    "complex128",
    "bool",
    "byte",
    "rune",
    "error",
    "any",
    "comparable",
];

/// Check if a type name is a Go built-in.
pub fn is_go_builtin(name: &str) -> bool {
    GO_BUILTINS.contains(&name)
}

/// Determine Go visibility from a name.
/// Names starting with uppercase are public, others are private.
pub fn go_visibility(name: &str) -> Option<String> {
    name.chars()
        .next()
        .map(|c| {
            if c.is_uppercase() {
                "public"
            } else {
                "private"
            }
        })
        .map(|s| s.to_string())
}

/// Determine the entry type for a Go function based on naming conventions.
/// - `main` -> "main"
/// - `init` -> "init"
/// - `TestXxx` -> "test"
/// - `BenchmarkXxx` -> "benchmark"
/// - `FuzzXxx` -> "fuzz"
/// - `ExampleXxx` -> "example"
pub fn go_entry_type(name: &str) -> Option<String> {
    match name {
        "main" => Some("main".to_string()),
        "init" => Some("init".to_string()),
        n if n.starts_with("Test")
            && n.len() > 4
            && n[4..].starts_with(|c: char| c.is_uppercase()) =>
        {
            Some("test".to_string())
        }
        "TestMain" => Some("test".to_string()),
        n if n.starts_with("Benchmark")
            && n.len() > 9
            && n[9..].starts_with(|c: char| c.is_uppercase()) =>
        {
            Some("benchmark".to_string())
        }
        n if n.starts_with("Fuzz")
            && n.len() > 4
            && n[4..].starts_with(|c: char| c.is_uppercase()) =>
        {
            Some("fuzz".to_string())
        }
        n if n.starts_with("Example") => Some("example".to_string()),
        _ => None,
    }
}

/// Find symbol ID by name and line (symbol must contain the line).
/// Returns a SymbolId for the symbol with the given name containing the given line.
pub fn find_symbol_id(
    parsed: &ParsedFile,
    file_path: &str,
    name: &str,
    line: usize,
) -> Option<SymbolId> {
    parsed.symbols.iter().find_map(|s| {
        if s.name == name && s.start_line <= line && s.end_line >= line {
            Some(SymbolId::new(file_path, &s.name, s.start_line))
        } else {
            None
        }
    })
}

/// Find the enclosing function/method/var symbol ID for a given line.
/// Returns a SymbolId for the innermost symbol containing the given line.
/// Prefers function/method over var/const when both contain the line.
pub fn find_enclosing_symbol_id(
    parsed: &ParsedFile,
    file_path: &str,
    line: usize,
) -> Option<SymbolId> {
    // First try to find enclosing function/method
    let func_match = parsed.symbols.iter().find(|s| {
        (s.kind == "function" || s.kind == "method") && s.start_line <= line && s.end_line >= line
    });

    if let Some(s) = func_match {
        return Some(SymbolId::new(file_path, &s.name, s.start_line));
    }

    // Fall back to var/const (for calls inside anonymous functions)
    parsed.symbols.iter().find_map(|s| {
        if (s.kind == "var" || s.kind == "const") && s.start_line <= line && s.end_line >= line {
            Some(SymbolId::new(file_path, &s.name, s.start_line))
        } else {
            None
        }
    })
}

/// Extract the receiver type name from a method receiver parameter_list node.
///
/// Handles these Go receiver patterns:
/// - `(c Cache)` -> "Cache" (value receiver)
/// - `(c *Cache)` -> "Cache" (pointer receiver)
/// - `(c pkg.Cache)` -> "Cache" (qualified type)
/// - `(c *pkg.Cache)` -> "Cache" (pointer to qualified)
///
/// Returns the type name without pointer or package qualifier.
pub fn extract_receiver_type(recv_node: tree_sitter::Node, code: &str) -> Option<String> {
    // recv_node is a parameter_list: (c *Cache)
    // Find the parameter_declaration inside
    let param_decl = recv_node
        .children(&mut recv_node.walk())
        .find(|n| n.kind() == "parameter_declaration")?;

    // Find the type field of the parameter_declaration
    let type_node = param_decl.children(&mut param_decl.walk()).find(|n| {
        matches!(
            n.kind(),
            "type_identifier" | "pointer_type" | "qualified_type"
        )
    })?;

    extract_type_name_from_node(type_node, code)
}

/// Extract the base type name from a type node, stripping pointers and qualifiers.
fn extract_type_name_from_node(node: tree_sitter::Node, code: &str) -> Option<String> {
    match node.kind() {
        "type_identifier" => {
            // Direct type: Cache
            let name = &code[node.byte_range()];
            Some(name.to_string())
        }
        "pointer_type" => {
            // Pointer type: *Cache or *pkg.Cache
            // Find the inner type
            let inner = node
                .children(&mut node.walk())
                .find(|n| matches!(n.kind(), "type_identifier" | "qualified_type"))?;
            extract_type_name_from_node(inner, code)
        }
        "qualified_type" => {
            // Qualified type: pkg.Cache
            // Get the name field (the type identifier after the dot)
            let name_node = node.child_by_field_name("name")?;
            let name = &code[name_node.byte_range()];
            Some(name.to_string())
        }
        _ => None,
    }
}
