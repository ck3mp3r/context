// Tests for Go language parser

use crate::analysis::lang::golang::{Go, Kind};
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
    parser.set_language(&Go::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let mut symbols = Vec::new();
    fn collect(node: Node, code: &str, symbols: &mut Vec<(Kind, String)>) {
        if let Some(sym) = Go::parse_symbol(node, code) {
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
fn test_parse_function() {
    let code = r#"
package main

func hello(name string) string {
    return "Hello, " + name
}
"#;
    let symbols = extract_all_symbols(code);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Function, "hello".to_string()));
}

#[test]
fn test_parse_method() {
    let code = r#"
package main

type Person struct {
    Name string
}

func (p Person) Greet() string {
    return "Hi, " + p.Name
}

func (p *Person) SetName(name string) {
    p.Name = name
}
"#;
    let symbols = extract_all_symbols(code);
    let methods: Vec<_> = symbols.iter().filter(|(k, _)| *k == Kind::Method).collect();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].1, "Greet");
    assert_eq!(methods[1].1, "SetName");
}

#[test]
fn test_parse_struct() {
    let code = r#"
package main

type User struct {
    Name  string
    Email string
    Age   int
}
"#;
    let symbols = extract_all_symbols(code);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Struct, "User".to_string()));
}

#[test]
fn test_parse_interface() {
    let code = r#"
package main

type Reader interface {
    Read(p []byte) (n int, err error)
}
"#;
    let symbols = extract_all_symbols(code);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0], (Kind::Interface, "Reader".to_string()));
}

#[test]
fn test_parse_type_alias() {
    let code = r#"
package main

type StringSlice []string
type MyInt int
"#;
    let symbols = extract_all_symbols(code);
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0], (Kind::TypeAlias, "StringSlice".to_string()));
    assert_eq!(symbols[1], (Kind::TypeAlias, "MyInt".to_string()));
}

#[test]
fn test_parse_const() {
    let code = r#"
package main

const MaxSize = 100
const (
    A = 1
    B = 2
)
"#;
    let symbols = extract_all_symbols(code);
    let consts: Vec<_> = symbols.iter().filter(|(k, _)| *k == Kind::Const).collect();
    assert_eq!(consts.len(), 3);
    assert_eq!(consts[0].1, "MaxSize");
    assert_eq!(consts[1].1, "A");
    assert_eq!(consts[2].1, "B");
}

#[test]
fn test_parse_var() {
    let code = r#"
package main

var GlobalName = "world"
var (
    X int
    Y = 42
)
"#;
    let symbols = extract_all_symbols(code);
    let vars: Vec<_> = symbols.iter().filter(|(k, _)| *k == Kind::Var).collect();
    assert_eq!(vars.len(), 3);
    assert_eq!(vars[0].1, "GlobalName");
    assert_eq!(vars[1].1, "X");
    assert_eq!(vars[2].1, "Y");
}

#[test]
fn test_parse_mixed_file() {
    let code = r#"
package main

import "fmt"

const Version = "1.0"

type Server struct {
    Host string
    Port int
}

type Handler interface {
    Handle(req Request) Response
}

var DefaultServer = &Server{Host: "localhost", Port: 8080}

func NewServer(host string, port int) *Server {
    return &Server{Host: host, Port: port}
}

func (s *Server) Start() error {
    fmt.Println("Starting server")
    return nil
}
"#;
    let symbols = extract_all_symbols(code);
    let names: Vec<_> = symbols.iter().map(|s| s.1.as_str()).collect();
    assert!(names.contains(&"Version"), "Should find const Version");
    assert!(names.contains(&"Server"), "Should find struct Server");
    assert!(names.contains(&"Handler"), "Should find interface Handler");
    assert!(
        names.contains(&"DefaultServer"),
        "Should find var DefaultServer"
    );
    assert!(names.contains(&"NewServer"), "Should find func NewServer");
    assert!(names.contains(&"Start"), "Should find method Start");
}

// ============================================================================
// Callee extraction tests
// ============================================================================

#[test]
fn test_extract_callee_simple() {
    let code = r#"
package main

func main() {
    fmt.Println("hello")
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Go::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let call_node = parse_and_find(&tree, "call_expression").unwrap();
    let callee = Go::extract_callee(call_node, code);
    assert!(callee.is_some(), "Should extract callee");
    assert_eq!(callee.unwrap(), "fmt.Println");
}

#[test]
fn test_extract_callee_function() {
    let code = r#"
package main

func main() {
    doWork()
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Go::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let call_node = parse_and_find(&tree, "call_expression").unwrap();
    let callee = Go::extract_callee(call_node, code);
    assert!(callee.is_some(), "Should extract callee");
    assert_eq!(callee.unwrap(), "doWork");
}

// ============================================================================
// Signature extraction tests
// ============================================================================

#[test]
fn test_extract_function_signature() {
    let code = r#"
package main

func greet(name string, age int) (string, error) {
    return fmt.Sprintf("Hello %s, age %d", name, age), nil
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Go::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_declaration").unwrap();
    let sig = Go::extract_signature(fn_node, code);
    assert!(sig.is_some(), "Should extract signature");
    let sig = sig.unwrap();
    assert!(
        sig.contains("func greet"),
        "Sig should contain 'func greet', got: {}",
        sig
    );
    assert!(
        sig.contains("name string"),
        "Sig should contain params, got: {}",
        sig
    );
    // Should NOT contain body
    assert!(
        !sig.contains("Sprintf"),
        "Sig should not contain body, got: {}",
        sig
    );
}

#[test]
fn test_extract_method_signature() {
    let code = r#"
package main

type Server struct{}

func (s *Server) Listen(addr string) error {
    return nil
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Go::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let method_node = parse_and_find(&tree, "method_declaration").unwrap();
    let sig = Go::extract_signature(method_node, code);
    assert!(sig.is_some(), "Should extract method signature");
    let sig = sig.unwrap();
    assert!(
        sig.contains("func (s *Server) Listen"),
        "Sig should contain receiver and name, got: {}",
        sig
    );
}

// ============================================================================
// Type reference extraction tests
// ============================================================================

#[test]
fn test_extract_type_references_from_function() {
    let code = r#"
package main

func process(r *Reader, w Writer) error {
    return nil
}
"#;
    let mut parser = Parser::new();
    parser.set_language(&Go::grammar()).unwrap();
    let tree = parser.parse(code, None).unwrap();

    let fn_node = parse_and_find(&tree, "function_declaration").unwrap();
    let refs = Go::extract_type_references(fn_node, code);
    let type_names: Vec<_> = refs.iter().map(|(name, _)| name.as_str()).collect();
    assert!(
        type_names.contains(&"Reader"),
        "Should reference Reader, got: {:?}",
        type_names
    );
    assert!(
        type_names.contains(&"Writer"),
        "Should reference Writer, got: {:?}",
        type_names
    );
}
