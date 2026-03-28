// Nushell language parser implementation

use super::types::Kind;
use crate::analysis::parser::{ImplInfo, Language};
use crate::analysis::types::{ReferenceType, SymbolName};
use tree_sitter::Node;

/// Nushell language implementation
pub struct Nushell;

impl Language for Nushell {
    type Kind = Kind;

    fn grammar() -> tree_sitter::Language {
        tree_sitter_nu::LANGUAGE.into()
    }

    fn parse_symbol(node: Node, code: &str) -> Option<(Self::Kind, String)> {
        match node.kind() {
            "decl_def" => extract_def_name(node, code),
            "decl_module" => extract_decl_name(node, code, Kind::Module),
            "decl_alias" => extract_decl_name(node, code, Kind::Alias),
            "decl_extern" => extract_decl_name(node, code, Kind::Extern),
            "stmt_const" => extract_const_name(node, code),
            _ => None,
        }
    }

    fn extract_callee(node: Node, code: &str) -> Option<String> {
        // In Nushell, the `command` node has a `head` field containing the command name.
        // The head can be a `cmd_identifier` (most common).
        for child in node.children(&mut node.walk()) {
            if child.kind() == "cmd_identifier" {
                return Some(code[child.byte_range()].to_string());
            }
        }
        None
    }

    fn parse_impl(_node: Node, _code: &str) -> Option<ImplInfo> {
        // Nushell has no impl blocks
        None
    }

    fn extract_type_references(node: Node, code: &str) -> Vec<(SymbolName, ReferenceType)> {
        let mut refs = Vec::new();

        if node.kind() == "decl_use" {
            // `use <module>` or `use <module> [items]`
            // The `module` field contains the module being imported
            if let Some(module_node) = node.child_by_field_name("module") {
                let name = code[module_node.byte_range()].trim().to_string();
                if !name.is_empty() {
                    refs.push((SymbolName::new(name), ReferenceType::Import));
                }
            }
        }

        refs
    }

    fn extract_signature(node: Node, code: &str) -> Option<String> {
        match node.kind() {
            "decl_def" => {
                // Signature = everything before the body block
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        let sig_end = child.start_byte();
                        let sig = code[node.start_byte()..sig_end].trim();
                        return Some(sig.to_string());
                    }
                }
                Some(code[node.byte_range()].trim().to_string())
            }
            "decl_extern" => {
                // Entire extern declaration is the signature
                let text = code[node.byte_range()].trim();
                if text.len() > 200 {
                    Some(format!("{}...", &text[..200]))
                } else {
                    Some(text.to_string())
                }
            }
            "decl_alias" => Some(code[node.byte_range()].trim().to_string()),
            "stmt_const" => Some(code[node.byte_range()].trim().to_string()),
            "decl_module" => {
                // Just the header, not the body
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        let sig_end = child.start_byte();
                        let sig = code[node.start_byte()..sig_end].trim();
                        return Some(sig.to_string());
                    }
                }
                Some(code[node.byte_range()].trim().to_string())
            }
            _ => None,
        }
    }

    fn name() -> &'static str {
        "nushell"
    }

    fn extensions() -> &'static [&'static str] {
        &["nu"]
    }

    fn call_node_kinds() -> &'static [&'static str] {
        &["command"]
    }
}

/// Extract the name from a `decl_def` node.
/// The name can be in `unquoted_name` (cmd_identifier) or `quoted_name` (val_string).
fn extract_def_name(node: Node, code: &str) -> Option<(Kind, String)> {
    // Check unquoted_name field first
    if let Some(name_node) = node.child_by_field_name("unquoted_name") {
        let name = code[name_node.byte_range()].to_string();
        return Some((Kind::Command, name));
    }
    // Then quoted_name (e.g. `def "my command" [...]`)
    if let Some(name_node) = node.child_by_field_name("quoted_name") {
        let raw = code[name_node.byte_range()].to_string();
        // Strip surrounding quotes
        let name = raw.trim_matches('"').trim_matches('\'').to_string();
        return Some((Kind::Command, name));
    }
    None
}

/// Extract name from `decl_module`, `decl_alias`, or `decl_extern` nodes.
/// These all have `unquoted_name` and `quoted_name` fields.
fn extract_decl_name(node: Node, code: &str, kind: Kind) -> Option<(Kind, String)> {
    if let Some(name_node) = node.child_by_field_name("unquoted_name") {
        return Some((kind, code[name_node.byte_range()].to_string()));
    }
    if let Some(name_node) = node.child_by_field_name("quoted_name") {
        let raw = code[name_node.byte_range()].to_string();
        let name = raw.trim_matches('"').trim_matches('\'').to_string();
        return Some((kind, name));
    }
    None
}

/// Extract name from `stmt_const` node.
/// `const FOO = ...` — the name is an identifier child.
fn extract_const_name(node: Node, code: &str) -> Option<(Kind, String)> {
    for child in node.children(&mut node.walk()) {
        if child.kind() == "identifier" {
            return Some((Kind::Const, code[child.byte_range()].to_string()));
        }
    }
    None
}
