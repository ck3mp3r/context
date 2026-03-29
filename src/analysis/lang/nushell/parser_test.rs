// Tests for Nushell language parser

use crate::analysis::lang::nushell::{Kind, Nushell};
use crate::analysis::parser::Language;
use tree_sitter::{Node, Parser};

/// Helper: parse code and find first node of given kind
fn parse_and_find<'a>(tree: &'a tree_sitter::Tree, node_kind: &str) -> Option<Node<'a>> {
    fn find_node<'b>(node: Node<'b>, kind: &str) -> Option<Node<'b>> {
        if node.kind() == kind {
            return Some(node);
        }
        for child in node.children(&mut node.walk()) {
            if let Some(found) = find_node(child, kind) {
                return Some(found);
            }
        }
        None
    }
    find_node(tree.root_node(), node_kind)
}

/// Helper: parse code and collect all symbols
fn extract_all_symbols(code: &str) -> Vec<(Kind, String)> {
    let mut parser = Parser::new();
    parser.set_language(&Nushell::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let mut symbols = Vec::new();
    fn collect(node: Node, code: &str, symbols: &mut Vec<(Kind, String)>) {
        if let Some(sym) = Nushell::parse_symbol(node, code) {
            symbols.push(sym);
        }
        for child in node.children(&mut node.walk()) {
            collect(child, code, symbols);
        }
    }
    collect(tree.root_node(), code, &mut symbols);
    symbols
}

// ============================================================================
// Symbol extraction tests
// ============================================================================

#[test]
fn test_parse_simple_def() {
    let code = r#"
def greet [name: string] {
    print $"Hello, ($name)!"
}
"#;
    let symbols = extract_all_symbols(code);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Command, "greet".to_string()));
}

#[test]
fn test_parse_quoted_def() {
    let code = r#"
def "my command" [arg: string] {
    print $arg
}
"#;
    let symbols = extract_all_symbols(code);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Command, "my command".to_string()));
}

#[test]
fn test_parse_module() {
    let code = r#"
module utils {
    export def helper [] { "help" }
}
"#;
    let symbols = extract_all_symbols(code);
    let names: Vec<_> = symbols.iter().map(|s| s.1.as_str()).collect();
    assert!(names.contains(&"utils"), "Should find module 'utils'");
    assert!(
        names.contains(&"helper"),
        "Should find exported def 'helper'"
    );
}

#[test]
fn test_parse_alias() {
    let code = r#"
alias ll = ls -l
"#;
    let symbols = extract_all_symbols(code);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Alias, "ll".to_string()));
}

#[test]
fn test_parse_const() {
    let code = r#"
const MY_VALUE = 42
"#;
    let symbols = extract_all_symbols(code);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Const, "MY_VALUE".to_string()));
}

#[test]
fn test_parse_extern() {
    let code = r#"
extern "git" [
    command: string
]
"#;
    let symbols = extract_all_symbols(code);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Extern, "git".to_string()));
}

#[test]
fn test_parse_mixed_file() {
    let code = r#"
const VERSION = "1.0.0"

module core {
    export def run [] { "running" }
}

def main [] {
    print $VERSION
}

alias v = print $VERSION
"#;
    let symbols = extract_all_symbols(code);
    let names: Vec<_> = symbols.iter().map(|s| s.1.as_str()).collect();
    assert!(names.contains(&"VERSION"), "Should find const VERSION");
    assert!(names.contains(&"core"), "Should find module core");
    assert!(names.contains(&"run"), "Should find def run");
    assert!(names.contains(&"main"), "Should find def main");
    assert!(names.contains(&"v"), "Should find alias v");
}

// ============================================================================
// Signature extraction tests
// ============================================================================

#[test]
fn test_extract_command_signature() {
    let code = r#"
def greet [name: string, --loud(-l)] -> string {
    if $loud {
        $"HELLO, ($name)!"
    } else {
        $"Hello, ($name)!"
    }
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Nushell::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let def_node = parse_and_find(&tree, "decl_def").unwrap();
    let sig = Nushell::extract_signature(def_node, code);
    assert!(sig.is_some(), "Should extract signature");
    let sig = sig.unwrap();
    assert!(
        sig.contains("def greet"),
        "Sig should contain 'def greet', got: {}",
        sig
    );
    assert!(
        sig.contains("name: string"),
        "Sig should contain param types, got: {}",
        sig
    );
    // Signature should NOT contain the body
    assert!(
        !sig.contains("HELLO"),
        "Sig should not contain body, got: {}",
        sig
    );
}

// ============================================================================
// Callee extraction tests
// ============================================================================

#[test]
fn test_extract_callee() {
    let code = r#"
def main [] {
    greet "world"
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Nushell::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let cmd_node = parse_and_find(&tree, "command").unwrap();
    let callee = Nushell::extract_callee(cmd_node, code);
    assert!(callee.is_some(), "Should extract callee");
    assert_eq!(callee.unwrap(), "greet");
}

// ============================================================================
// Module containment tests
// ============================================================================

#[test]
fn test_module_info_inline_module() {
    let code = r#"
module utils {
    export def helper [] { "help" }
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Nushell::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let mod_node = parse_and_find(&tree, "decl_module").unwrap();
    let info = Nushell::module_info(mod_node, code, "main.nu");
    assert!(info.is_some(), "Inline module should return ModuleInfo");
    let info = info.unwrap();
    assert!(info.has_body, "Inline module should have a body");
    assert!(
        info.candidate_paths.is_empty(),
        "Inline module should have no candidate paths"
    );
}

#[test]
fn test_module_info_file_based_use() {
    // `use utils.nu` is not a module declaration — module_info should
    // only fire on `decl_module` nodes, not `use` statements.
    // File-based modules are handled by the file that IS the module.
    // So module_info returns None for non-module nodes.
    let code = "use utils.nu";
    let mut parser = Parser::new();
    parser.set_language(&Nushell::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    // There should be no decl_module node
    let mod_node = parse_and_find(&tree, "decl_module");
    assert!(
        mod_node.is_none(),
        "use statement is not a module declaration"
    );
}

#[test]
fn test_module_info_non_module_node() {
    let code = "def greet [] { 'hello' }";
    let mut parser = Parser::new();
    parser.set_language(&Nushell::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let def_node = parse_and_find(&tree, "decl_def").unwrap();
    let info = Nushell::module_info(def_node, code, "main.nu");
    assert!(info.is_none(), "Non-module node should return None");
}
