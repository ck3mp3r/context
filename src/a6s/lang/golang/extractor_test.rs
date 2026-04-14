use super::extractor::GolangExtractor;
use crate::a6s::extract::LanguageExtractor;

#[test]
fn test_golang_extractor_language() {
    let extractor = GolangExtractor;
    assert_eq!(extractor.language(), "go");
}

#[test]
fn test_golang_extractor_extensions() {
    let extractor = GolangExtractor;
    assert_eq!(extractor.extensions(), &["go"]);
}

#[test]
fn test_extract_compiles_query() {
    let extractor = GolangExtractor;
    let code = "package main\nfunc main() {}";
    let result = extractor.extract(code, "main.go");

    // Verify basic extraction works
    assert_eq!(result.language, "go");
    assert_eq!(result.file_path, "main.go");
    // Symbols will be empty until Phase 2 is implemented
}

#[test]
fn test_golang_extractor_queries_nonempty() {
    let extractor = GolangExtractor;
    assert!(!extractor.symbol_queries().is_empty());
    assert!(!extractor.type_ref_queries().is_empty());
}

#[test]
fn test_extract_package() {
    let extractor = GolangExtractor;
    let code = "package main";
    let result = extractor.extract(code, "main.go");

    assert_eq!(result.symbols.len(), 1);
    let pkg = &result.symbols[0];
    assert_eq!(pkg.name, "main");
    assert_eq!(pkg.kind, "package");
    assert_eq!(pkg.visibility, Some("pub".to_string()));
}

#[test]
fn test_is_exported() {
    // Test exported names (uppercase first letter)
    assert!(GolangExtractor::is_exported("Hello"));
    assert!(GolangExtractor::is_exported("Server"));
    assert!(GolangExtractor::is_exported("API"));

    // Test unexported names (lowercase first letter)
    assert!(!GolangExtractor::is_exported("hello"));
    assert!(!GolangExtractor::is_exported("server"));
    assert!(!GolangExtractor::is_exported("port"));
}

#[test]
fn test_extract_exported_function() {
    let extractor = GolangExtractor;
    let code = "package main\nfunc Hello() {}";
    let result = extractor.extract(code, "main.go");

    // Should extract package + function
    assert_eq!(result.symbols.len(), 2);

    let func = result
        .symbols
        .iter()
        .find(|s| s.kind == "function")
        .unwrap();
    assert_eq!(func.name, "Hello");
    assert_eq!(func.kind, "function");
    assert_eq!(func.visibility, Some("pub".to_string()));
    assert_eq!(func.entry_type, None);
}

#[test]
fn test_extract_unexported_function() {
    let extractor = GolangExtractor;
    let code = "package main\nfunc hello() {}";
    let result = extractor.extract(code, "main.go");

    let func = result
        .symbols
        .iter()
        .find(|s| s.kind == "function")
        .unwrap();
    assert_eq!(func.name, "hello");
    assert_eq!(func.visibility, Some("private".to_string()));
}

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/a6s/lang/golang/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

#[test]
fn test_extract_all_symbol_types() {
    let extractor = GolangExtractor;
    let code = load_testdata("symbols.go");
    let result = extractor.extract(&code, "symbols.go");

    // Should extract: package, 2 consts, 2 vars, 1 type_alias, 1 struct, 1 interface, 2 functions, 2 methods, 2 fields, 1 interface_method
    assert!(
        result.symbols.len() >= 10,
        "Expected at least 10 symbols, got {}",
        result.symbols.len()
    );

    // Check package
    let pkg = result.symbols.iter().find(|s| s.kind == "package").unwrap();
    assert_eq!(pkg.name, "testpkg");

    // Check exported const
    let const_sym = result.symbols.iter().find(|s| s.name == "MaxSize").unwrap();
    assert_eq!(const_sym.kind, "const");
    assert_eq!(const_sym.visibility, Some("pub".to_string()));

    // Check unexported const
    let const_priv = result.symbols.iter().find(|s| s.name == "minSize").unwrap();
    assert_eq!(const_priv.kind, "const");
    assert_eq!(const_priv.visibility, Some("private".to_string()));

    // Check exported var
    let var_sym = result
        .symbols
        .iter()
        .find(|s| s.name == "GlobalCounter")
        .unwrap();
    assert_eq!(var_sym.kind, "var");
    assert_eq!(var_sym.visibility, Some("pub".to_string()));

    // Check type alias
    let type_alias = result.symbols.iter().find(|s| s.name == "UserID").unwrap();
    assert_eq!(type_alias.kind, "type_alias");

    // Check struct
    let struct_sym = result.symbols.iter().find(|s| s.name == "Server").unwrap();
    assert_eq!(struct_sym.kind, "struct");
    assert_eq!(struct_sym.visibility, Some("pub".to_string()));

    // Check interface
    let iface = result.symbols.iter().find(|s| s.name == "Reader").unwrap();
    assert_eq!(iface.kind, "interface");
    assert_eq!(iface.visibility, Some("pub".to_string()));

    // Check exported function
    let func = result
        .symbols
        .iter()
        .find(|s| s.name == "NewServer")
        .unwrap();
    assert_eq!(func.kind, "function");
    assert_eq!(func.visibility, Some("pub".to_string()));

    // Check unexported function
    let func_priv = result.symbols.iter().find(|s| s.name == "helper").unwrap();
    assert_eq!(func_priv.kind, "function");
    assert_eq!(func_priv.visibility, Some("private".to_string()));

    // Check exported method
    let method = result
        .symbols
        .iter()
        .find(|s| s.name == "Start" && s.kind == "method")
        .unwrap();
    assert_eq!(method.visibility, Some("pub".to_string()));

    // Check unexported method
    let method_priv = result
        .symbols
        .iter()
        .find(|s| s.name == "validateConfig")
        .unwrap();
    assert_eq!(method_priv.kind, "method");
    assert_eq!(method_priv.visibility, Some("private".to_string()));

    // Check struct fields
    let port_field = result
        .symbols
        .iter()
        .find(|s| s.name == "Port" && s.kind == "field")
        .unwrap();
    assert_eq!(port_field.visibility, Some("pub".to_string()));
    // Field signature should NOT contain "parent:" prefix (line-range containment is used instead)
    assert!(
        !port_field.signature.as_ref().unwrap().contains("parent:"),
        "Field signature should not contain parent: prefix, got: {}",
        port_field.signature.as_ref().unwrap()
    );

    let host_field = result.symbols.iter().find(|s| s.name == "host").unwrap();
    assert_eq!(host_field.kind, "field");
    assert_eq!(host_field.visibility, Some("private".to_string()));

    // Check interface method
    let iface_method = result
        .symbols
        .iter()
        .find(|s| s.name == "Read" && s.kind == "interface_method")
        .unwrap();
    assert_eq!(iface_method.visibility, Some("pub".to_string()));
    // Interface method signature should NOT contain "parent:" prefix (line-range containment is used instead)
    assert!(
        !iface_method.signature.as_ref().unwrap().contains("parent:"),
        "Interface method signature should not contain parent: prefix, got: {}",
        iface_method.signature.as_ref().unwrap()
    );

    // Check HasField edges (struct -> field, using resolved refs)
    let hasfield_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasField))
        .collect();
    assert!(
        hasfield_edges.len() >= 2,
        "Expected at least 2 HasField edges (Port + host), got {}",
        hasfield_edges.len()
    );

    // Should have HasField edge Server -> Port with resolved refs
    let port_edge = hasfield_edges.iter().find(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Server:"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Port:"))
    });
    assert!(port_edge.is_some(), "Expected HasField edge Server -> Port");

    // Check HasMethod edges (interface -> interface_method AND receiver type -> method)
    let hasmethod_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();
    assert!(
        hasmethod_edges.len() >= 3,
        "Expected at least 3 HasMethod edges (Reader->Read, Reader->Close, Server->Start, Server->validateConfig), got {}",
        hasmethod_edges.len()
    );

    // Should have HasMethod edge Reader -> Read with resolved refs
    let read_edge = hasmethod_edges.iter().find(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Reader:"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Read:"))
    });
    assert!(
        read_edge.is_some(),
        "Expected HasMethod edge Reader -> Read"
    );

    // Should have HasMethod edge Server -> Start with resolved refs (receiver method)
    let start_edge = hasmethod_edges.iter().find(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Server:"))
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Start:"))
    });
    assert!(
        start_edge.is_some(),
        "Expected HasMethod edge Server -> Start (receiver method)"
    );
}

#[test]
fn test_extract_hasfield_edges() {
    let extractor = GolangExtractor;
    let code = r#"
package test
type User struct {
    Name string
    age  int
}
"#;
    let result = extractor.extract(code, "test.go");

    // Should have 2 HasField edges: User -> Name, User -> age
    let hasfield_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasField))
        .collect();
    assert_eq!(
        hasfield_edges.len(),
        2,
        "Expected 2 HasField edges, got {}",
        hasfield_edges.len()
    );

    // Both from and to must be Resolved SymbolRefs
    let name_edge = hasfield_edges.iter().find(|e| {
        e.kind == crate::a6s::types::EdgeKind::HasField
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Name:"))
    });
    assert!(name_edge.is_some(), "Expected HasField edge to Name");
    let name_edge = name_edge.unwrap();
    assert!(
        matches!(&name_edge.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":User:")),
        "Expected HasField edge from User"
    );

    let age_edge = hasfield_edges.iter().find(|e| {
        e.kind == crate::a6s::types::EdgeKind::HasField
            && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":age:"))
    });
    assert!(age_edge.is_some(), "Expected HasField edge to age");
    let age_edge = age_edge.unwrap();
    assert!(
        matches!(&age_edge.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":User:")),
        "Expected HasField edge from User"
    );
}

#[test]
fn test_file_categorization_test_file() {
    let extractor = GolangExtractor;
    let code = "package main\nfunc Hello() {}";
    let result = extractor.extract(code, "main_test.go");

    assert_eq!(result.file_category, Some("test_file".to_string()));
}

#[test]
fn test_file_categorization_contains_tests() {
    let extractor = GolangExtractor;
    let code = "package main\nfunc TestHello() {}\nfunc Hello() {}";
    let result = extractor.extract(code, "main.go");

    // Has test function but not a _test.go file
    assert_eq!(result.file_category, Some("contains_tests".to_string()));
}

#[test]
fn test_test_function_detection() {
    let extractor = GolangExtractor;
    let code = r#"
package main
func TestFoo() {}
func BenchmarkBar() {}
func ExampleBaz() {}
"#;
    let result = extractor.extract(code, "main_test.go");

    let test_func = result.symbols.iter().find(|s| s.name == "TestFoo").unwrap();
    assert_eq!(test_func.entry_type, Some("test".to_string()));

    let bench_func = result
        .symbols
        .iter()
        .find(|s| s.name == "BenchmarkBar")
        .unwrap();
    assert_eq!(bench_func.entry_type, Some("test".to_string()));

    let example_func = result
        .symbols
        .iter()
        .find(|s| s.name == "ExampleBaz")
        .unwrap();
    assert_eq!(example_func.entry_type, Some("test".to_string()));
}

#[test]
fn test_calls_edge_function_call() {
    let code = r#"
package main

func caller() {
    callee()
}

func callee() {}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have 3 symbols: package, caller, callee
    assert_eq!(result.symbols.len(), 3);

    // Should have 1 Calls edge: caller → callee
    let calls_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::Calls))
        .collect();
    assert_eq!(
        calls_edges.len(),
        1,
        "Expected 1 Calls edge, got {}",
        calls_edges.len()
    );

    // Verify edge goes from caller to callee
    let edge = calls_edges[0];
    assert!(edge.from.is_resolved(), "from should be resolved");
    assert!(
        matches!(&edge.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "callee"),
        "to should be unresolved callee"
    );
}

#[test]
fn test_calls_edge_method_call() {
    let code = r#"
package main

type Server struct {}

func (s *Server) Start() {
    s.init()
}

func (s *Server) init() {}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have Calls edge: Start → init
    let calls_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::Calls))
        .collect();
    assert!(calls_edges.len() >= 1, "Expected at least 1 Calls edge");

    // Verify one edge has init as target
    assert!(calls_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "init")
    }));
}

#[test]
fn test_uses_edge_return_type() {
    let code = r#"
package main

type Server struct {
    Name string
}

func NewServer() *Server {
    return &Server{}
}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have Usage edge: NewServer → Server
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::Usage))
        .collect();
    assert!(
        uses_edges.len() >= 1,
        "Expected at least 1 Usage edge, got {}",
        uses_edges.len()
    );

    // Verify edge targets Server
    assert!(
        uses_edges.iter().any(|e| {
            matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Server")
        }),
        "Expected Usage edge to Server, got: {:?}",
        uses_edges.iter().map(|e| match &e.to {
            crate::a6s::types::SymbolRef::Unresolved { name, .. } => name.as_str(),
            _ => "resolved"
        }).collect::<Vec<_>>()
    );
}

#[test]
fn test_uses_edge_composite_literal() {
    let code = r#"
package main

type Config struct {
    Port int
}

func getConfig() Config {
    return Config{Port: 8080}
}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have Usage edge: getConfig → Config
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::Usage))
        .collect();
    assert!(uses_edges.len() >= 1, "Expected at least 1 Usage edge");

    // Verify edge targets Config
    assert!(uses_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Config")
    }));
}

#[test]
fn test_uses_edge_short_var_decl() {
    let code = r#"
package main

type Logger struct {}

func setup() {
    log := Logger{}
}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have Usage edge: setup → Logger
    let uses_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::Usage))
        .collect();
    assert!(uses_edges.len() >= 1, "Expected at least 1 Usage edge");

    // Verify edge targets Logger
    assert!(uses_edges.iter().any(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Logger")
    }));
}

#[test]
fn test_import_simple() {
    let code = r#"
package main

import "fmt"

func main() {}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have 1 import
    assert_eq!(
        result.imports.len(),
        1,
        "Expected 1 import, got {}",
        result.imports.len()
    );

    // Verify import details
    let import = &result.imports[0];
    assert_eq!(import.entry.module_path, "fmt");
    assert_eq!(
        import.entry.alias, None,
        "Simple import should have no alias"
    );
}

#[test]
fn test_import_aliased() {
    let code = r#"
package main

import f "fmt"

func main() {}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have 1 import
    assert_eq!(result.imports.len(), 1);

    // Verify import has alias
    let import = &result.imports[0];
    assert_eq!(import.entry.module_path, "fmt");
    assert_eq!(
        import.entry.alias,
        Some("f".to_string()),
        "Aliased import should have alias 'f'"
    );
}

#[test]
fn test_import_grouped() {
    let code = r#"
package main

import (
    "fmt"
    "os"
    "io"
)

func main() {}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have 3 imports
    assert_eq!(
        result.imports.len(),
        3,
        "Expected 3 imports, got {}",
        result.imports.len()
    );

    // Verify all three imported
    let paths: Vec<_> = result
        .imports
        .iter()
        .map(|i| i.entry.module_path.as_str())
        .collect();
    assert!(paths.contains(&"fmt"));
    assert!(paths.contains(&"os"));
    assert!(paths.contains(&"io"));
}

#[test]
fn test_import_grouped_with_alias() {
    let code = r#"
package main

import (
    "fmt"
    f "fmt"
    "os"
)

func main() {}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have 3 imports (fmt twice - once simple, once aliased)
    assert_eq!(result.imports.len(), 3);

    // Find the aliased fmt import
    let aliased = result
        .imports
        .iter()
        .find(|i| i.entry.module_path == "fmt" && i.entry.alias.is_some());
    assert!(aliased.is_some(), "Should have aliased fmt import");
    assert_eq!(aliased.unwrap().entry.alias, Some("f".to_string()));

    // Find the simple fmt import
    let simple = result
        .imports
        .iter()
        .find(|i| i.entry.module_path == "fmt" && i.entry.alias.is_none());
    assert!(simple.is_some(), "Should have simple fmt import");
}

#[test]
fn test_import_external_package() {
    let code = r#"
package main

import "github.com/user/repo/pkg"

func main() {}
"#;
    let extractor = GolangExtractor;
    let result = extractor.extract(code, "test.go");

    // Should have 1 import
    assert_eq!(result.imports.len(), 1);

    // Verify full path preserved
    let import = &result.imports[0];
    assert_eq!(import.entry.module_path, "github.com/user/repo/pkg");
}

#[test]
fn test_extract_hasmethod_edges() {
    let extractor = GolangExtractor;
    let code = r#"
package test

type Writer interface {
    Write(p []byte) (int, error)
    Close() error
}
"#;
    let result = extractor.extract(code, "test.go");

    // Should have interface_method symbols (not "method" — those are receiver methods)
    let iface_methods: Vec<_> = result
        .symbols
        .iter()
        .filter(|s| s.kind == "interface_method")
        .collect();
    assert_eq!(
        iface_methods.len(),
        2,
        "Expected 2 interface_method symbols (Write, Close), got {}",
        iface_methods.len()
    );

    // Should have 2 HasMethod edges: Writer -> Write, Writer -> Close
    let hasmethod_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();
    assert_eq!(
        hasmethod_edges.len(),
        2,
        "Expected 2 HasMethod edges, got {}",
        hasmethod_edges.len()
    );

    // Both from and to must be Resolved SymbolRefs
    let write_edge = hasmethod_edges.iter().find(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Write:"))
    });
    assert!(write_edge.is_some(), "Expected HasMethod edge to Write");
    let write_edge = write_edge.unwrap();
    assert!(
        matches!(&write_edge.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Writer:")),
        "Expected HasMethod edge from Writer"
    );

    let close_edge = hasmethod_edges.iter().find(|e| {
        matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Close:"))
    });
    assert!(close_edge.is_some(), "Expected HasMethod edge to Close");
    let close_edge = close_edge.unwrap();
    assert!(
        matches!(&close_edge.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Writer:")),
        "Expected HasMethod edge from Writer"
    );
}

#[test]
fn test_hasmethod_pointer_receiver() {
    let extractor = GolangExtractor;
    let code = r#"
package test

type Server struct{}

func (s *Server) Start() error { return nil }
"#;
    let result = extractor.extract(code, "test.go");

    let hasmethod_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();
    assert_eq!(
        hasmethod_edges.len(),
        1,
        "Expected 1 HasMethod edge (Server -> Start), got {}",
        hasmethod_edges.len()
    );

    let edge = &hasmethod_edges[0];
    // from: Resolved (Server is in the same file)
    assert!(
        matches!(&edge.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Server:")),
        "Expected HasMethod from resolved Server, got: {:?}",
        edge.from
    );
    // to: Resolved (Start is in the same file)
    assert!(
        matches!(&edge.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Start:")),
        "Expected HasMethod to resolved Start, got: {:?}",
        edge.to
    );
}

#[test]
fn test_hasmethod_value_receiver() {
    let extractor = GolangExtractor;
    let code = r#"
package test

type Server struct{}

func (s Server) Stop() {}
"#;
    let result = extractor.extract(code, "test.go");

    let hasmethod_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();
    assert_eq!(
        hasmethod_edges.len(),
        1,
        "Expected 1 HasMethod edge (Server -> Stop), got {}",
        hasmethod_edges.len()
    );

    let edge = &hasmethod_edges[0];
    assert!(
        matches!(&edge.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Server:")),
        "Expected HasMethod from resolved Server, got: {:?}",
        edge.from
    );
    assert!(
        matches!(&edge.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Stop:")),
        "Expected HasMethod to resolved Stop, got: {:?}",
        edge.to
    );
}

#[test]
fn test_hasmethod_unresolved_receiver() {
    let extractor = GolangExtractor;
    // Server type is NOT defined in this file
    let code = r#"
package test

func (s *Server) Start() error { return nil }
"#;
    let result = extractor.extract(code, "test.go");

    let hasmethod_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();
    assert_eq!(
        hasmethod_edges.len(),
        1,
        "Expected 1 HasMethod edge, got {}",
        hasmethod_edges.len()
    );

    let edge = &hasmethod_edges[0];
    // from: Unresolved (Server is NOT in this file)
    assert!(
        matches!(&edge.from, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Server"),
        "Expected HasMethod from unresolved Server, got: {:?}",
        edge.from
    );
    // to: Resolved (Start method IS in this file)
    assert!(
        matches!(&edge.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Start:")),
        "Expected HasMethod to resolved Start, got: {:?}",
        edge.to
    );
}

#[test]
fn test_hasmethod_mixed_interface_and_receiver() {
    let extractor = GolangExtractor;
    let code = r#"
package test

type Reader interface {
    Read() error
}

type Server struct{}

func (s *Server) Start() error { return nil }
func (s Server) Stop() {}
"#;
    let result = extractor.extract(code, "test.go");

    // 3 HasMethod edges: Reader->Read, Server->Start, Server->Stop
    let hasmethod_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMethod))
        .collect();
    assert_eq!(
        hasmethod_edges.len(),
        3,
        "Expected 3 HasMethod edges (1 interface + 2 receiver), got {}",
        hasmethod_edges.len()
    );

    // Interface: Reader -> Read
    assert!(
        hasmethod_edges.iter().any(|e| {
            matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Reader:"))
                && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Read:"))
        }),
        "Expected HasMethod edge Reader -> Read"
    );

    // Receiver: Server -> Start
    assert!(
        hasmethod_edges.iter().any(|e| {
            matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Server:"))
                && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Start:"))
        }),
        "Expected HasMethod edge Server -> Start"
    );

    // Receiver: Server -> Stop
    assert!(
        hasmethod_edges.iter().any(|e| {
            matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Server:"))
                && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Stop:"))
        }),
        "Expected HasMethod edge Server -> Stop"
    );
}

#[test]
fn test_extract_package_members() {
    let extractor = GolangExtractor;
    let code = r#"
package main

func Hello() {}
type Server struct{}
const MaxSize = 100
var GlobalVar int
type UserID string
"#;
    let result = extractor.extract(code, "test.go");

    // Should have 6 symbols: package, function, struct, const, var, type_alias
    assert_eq!(
        result.symbols.len(),
        6,
        "Expected 6 symbols, got {}",
        result.symbols.len()
    );

    // Should have 5 HasMember edges: main -> Hello, Server, MaxSize, GlobalVar, UserID
    let hasmember_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMember))
        .collect();
    assert_eq!(
        hasmember_edges.len(),
        5,
        "Expected 5 HasMember edges, got {}",
        hasmember_edges.len()
    );

    // All edges should be from the package symbol (resolved)
    for edge in &hasmember_edges {
        assert!(
            matches!(&edge.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":main:")),
            "Expected HasMember edge from resolved 'main', got: {:?}",
            edge.from
        );
        assert!(
            edge.to.is_resolved(),
            "Expected HasMember 'to' to be resolved, got: {:?}",
            edge.to
        );
    }

    // Verify each specific member
    let member_names = ["Hello", "Server", "MaxSize", "GlobalVar", "UserID"];
    for name in &member_names {
        assert!(
            hasmember_edges.iter().any(|e| {
                matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(&format!(":{}:", name)))
            }),
            "Expected HasMember edge to {}, edges: {:?}",
            name,
            hasmember_edges
        );
    }
}

#[test]
fn test_package_members_excludes_fields_and_methods() {
    let extractor = GolangExtractor;
    let code = r#"
package test

type Server struct {
    Port int
    host string
}

type Reader interface {
    Read() error
}

func (s *Server) Start() error { return nil }

func TopLevel() {}
"#;
    let result = extractor.extract(code, "test.go");

    let hasmember_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMember))
        .collect();

    // Should have HasMember edges for: Server, Reader, TopLevel (3 top-level declarations)
    // Should NOT have HasMember edges for: Port, host (fields), Read (interface_method), Start (method)
    assert_eq!(
        hasmember_edges.len(),
        3,
        "Expected 3 HasMember edges (Server, Reader, TopLevel), got {}",
        hasmember_edges.len()
    );

    // Verify no field edges
    assert!(
        !hasmember_edges.iter().any(|e| {
            matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Port:"))
        }),
        "Fields should NOT get HasMember edges to the package"
    );

    // Verify no interface_method edges
    assert!(
        !hasmember_edges.iter().any(|e| {
            matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Read:"))
        }),
        "Interface methods should NOT get HasMember edges to the package"
    );

    // Verify no method edges
    assert!(
        !hasmember_edges.iter().any(|e| {
            matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Start:"))
        }),
        "Methods should NOT get HasMember edges to the package"
    );

    // Verify the 3 expected top-level members are present
    for name in &["Server", "Reader", "TopLevel"] {
        assert!(
            hasmember_edges.iter().any(|e| {
                matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(&format!(":{}:", name)))
            }),
            "Expected HasMember edge to {}",
            name
        );
    }
}

#[test]
fn test_package_members_no_self_edge() {
    let extractor = GolangExtractor;
    let code = r#"
package main

func Hello() {}
"#;
    let result = extractor.extract(code, "test.go");

    let hasmember_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| matches!(e.kind, crate::a6s::types::EdgeKind::HasMember))
        .collect();

    // Should have exactly 1 HasMember edge: main -> Hello
    assert_eq!(
        hasmember_edges.len(),
        1,
        "Expected 1 HasMember edge, got {}",
        hasmember_edges.len()
    );

    // The package should NOT have a self-edge
    assert!(
        !hasmember_edges.iter().any(|e| {
            matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":main:"))
        }),
        "Package should NOT have a self-edge"
    );
}
