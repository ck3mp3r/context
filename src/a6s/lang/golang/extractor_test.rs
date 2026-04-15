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

#[test]
fn test_extract_param_type_edge_direct() {
    let code = r#"
package main

type MyType struct{}

func Process(item MyType) {}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();

    assert_eq!(
        param_edges.len(),
        1,
        "Expected 1 ParamType edge, got: {:?}",
        param_edges
    );
    let edge = &param_edges[0];

    // from should be resolved to Process
    match &edge.from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Process:"),
            "from should be Process, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }

    // to should be resolved to MyType (same file)
    match &edge.to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":MyType:"),
            "to should be MyType, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_param_type_skips_builtins() {
    let code = r#"
package main

func Process(name string, count int, flag bool) {}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();

    assert_eq!(
        param_edges.len(),
        0,
        "Builtin types should not produce ParamType edges, got: {:?}",
        param_edges
    );
}

#[test]
fn test_param_type_pointer() {
    let code = r#"
package main

type MyType struct{}

func Process(item *MyType) {}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();

    assert_eq!(
        param_edges.len(),
        1,
        "Expected 1 ParamType edge, got: {:?}",
        param_edges
    );
    match &param_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Process:"),
            "from should be Process, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &param_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":MyType:"),
            "to should be MyType, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_param_type_slice() {
    let code = r#"
package main

type MyType struct{}

func Process(items []MyType) {}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();

    assert_eq!(
        param_edges.len(),
        1,
        "Expected 1 ParamType edge, got: {:?}",
        param_edges
    );
    match &param_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Process:"),
            "from should be Process, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &param_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":MyType:"),
            "to should be MyType, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_param_type_multiple_params() {
    let code = r#"
package main

type TypeA struct{}
type TypeB struct{}

func Convert(from TypeA, to TypeB) {}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();

    assert_eq!(
        param_edges.len(),
        2,
        "Expected 2 ParamType edges, got: {:?}",
        param_edges
    );

    // Both should be from Convert
    for edge in &param_edges {
        match &edge.from {
            crate::a6s::types::SymbolRef::Resolved(id) => assert!(
                id.as_str().contains(":Convert:"),
                "from should be Convert, got {:?}",
                id
            ),
            other => panic!("from should be Resolved, got {:?}", other),
        }
    }

    // Should have edges to both TypeA and TypeB
    assert!(
        param_edges.iter().any(|e| matches!(
            &e.to,
            crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":TypeA:")
        )),
        "Expected ParamType edge to TypeA"
    );
    assert!(
        param_edges.iter().any(|e| matches!(
            &e.to,
            crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":TypeB:")
        )),
        "Expected ParamType edge to TypeB"
    );
}

#[test]
fn test_param_type_method() {
    let code = r#"
package main

type Server struct{}
type Request struct{}

func (s *Server) Handle(req Request) {}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();

    // Should have 1 ParamType edge: Handle → Request
    // The receiver (Server) is NOT a param type edge
    assert_eq!(
        param_edges.len(),
        1,
        "Expected 1 ParamType edge (Handle → Request), got: {:?}",
        param_edges
    );
    match &param_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Handle:"),
            "from should be Handle, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &param_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Request:"),
            "to should be Request, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_param_type_mixed_builtin_and_custom() {
    let code = r#"
package main

type MyType struct{}

func Process(name string, item MyType, count int) {}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();

    // Only MyType should produce a ParamType edge (string and int are builtins)
    assert_eq!(
        param_edges.len(),
        1,
        "Expected 1 ParamType edge (only MyType), got: {:?}",
        param_edges
    );
    match &param_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Process:"),
            "from should be Process, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &param_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":MyType:"),
            "to should be MyType, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_param_type_variadic() {
    let code = r#"
package main

type MyType struct{}

func Process(items ...MyType) {}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let param_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();

    assert_eq!(
        param_edges.len(),
        1,
        "Expected 1 ParamType edge, got: {:?}",
        param_edges
    );
    match &param_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Process:"),
            "from should be Process, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &param_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":MyType:"),
            "to should be MyType, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

// ==========================================================================
// ReturnType edge tests
// ==========================================================================

#[test]
fn test_return_type_direct() {
    let code = r#"
package main

type MyType struct{}

func Create() MyType { return MyType{} }
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let ret_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ReturnType)
        .collect();

    assert_eq!(
        ret_edges.len(),
        1,
        "Expected 1 ReturnType edge, got: {:?}",
        ret_edges
    );
    match &ret_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Create:"),
            "from should be Create, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &ret_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":MyType:"),
            "to should be MyType, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_return_type_pointer() {
    let code = r#"
package main

type Server struct{}

func NewServer() *Server { return &Server{} }
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let ret_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ReturnType)
        .collect();

    assert_eq!(
        ret_edges.len(),
        1,
        "Expected 1 ReturnType edge, got: {:?}",
        ret_edges
    );
    match &ret_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":NewServer:"),
            "from should be NewServer, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &ret_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Server:"),
            "to should be Server, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_return_type_tuple_filters_builtins() {
    let code = r#"
package main

type Item struct{}

func GetItems() ([]Item, error) { return nil, nil }
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let ret_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ReturnType)
        .collect();

    // Only Item should produce a ReturnType edge (error is builtin)
    assert_eq!(
        ret_edges.len(),
        1,
        "Expected 1 ReturnType edge (Item only, error is builtin), got: {:?}",
        ret_edges
    );
    match &ret_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":GetItems:"),
            "from should be GetItems, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &ret_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Item:"),
            "to should be Item, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_return_type_method() {
    let code = r#"
package main

type Server struct{}
type ServerStatus int

func (s *Server) Status() ServerStatus { return 0 }
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let ret_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ReturnType)
        .collect();

    assert_eq!(
        ret_edges.len(),
        1,
        "Expected 1 ReturnType edge (Status → ServerStatus), got: {:?}",
        ret_edges
    );
    match &ret_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Status:"),
            "from should be Status, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &ret_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":ServerStatus:"),
            "to should be ServerStatus, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_return_type_no_edges_for_builtins() {
    let code = r#"
package main

func GetName() string { return "" }
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let ret_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ReturnType)
        .collect();

    assert_eq!(
        ret_edges.len(),
        0,
        "Builtin return types should not produce ReturnType edges, got: {:?}",
        ret_edges
    );
}

// ==========================================================================
// FieldType edge tests
// ==========================================================================

#[test]
fn test_field_type_direct() {
    let code = r#"
package main

type MyType struct{}

type Container struct {
    Item MyType
}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::FieldType)
        .collect();

    assert_eq!(
        field_edges.len(),
        1,
        "Expected 1 FieldType edge, got: {:?}",
        field_edges
    );
    match &field_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Item:"),
            "from should be Item field, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    match &field_edges[0].to {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":MyType:"),
            "to should be MyType, got {:?}",
            id
        ),
        other => panic!("to should be Resolved, got {:?}", other),
    }
}

#[test]
fn test_field_type_pointer_and_slice() {
    let code = r#"
package main

type Handler struct{}
type Item struct{}

type Config struct {
    Handler *Handler
    Items   []Item
}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::FieldType)
        .collect();

    assert_eq!(
        field_edges.len(),
        2,
        "Expected 2 FieldType edges, got: {:?}",
        field_edges
    );

    // Check Handler field → Handler type
    assert!(
        field_edges.iter().any(|e| {
            matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Handler:"))
                && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Handler:"))
        }),
        "Expected FieldType edge Handler field → Handler type"
    );

    // Check Items field → Item type
    assert!(
        field_edges.iter().any(|e| {
            matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Items:"))
                && matches!(&e.to, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(":Item:"))
        }),
        "Expected FieldType edge Items field → Item type"
    );
}

#[test]
fn test_field_type_qualified_unresolved() {
    let code = r#"
package main

type App struct {
    Srv http.Server
}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::FieldType)
        .collect();

    // qualified_type captures just the type_identifier "Server" — which is unresolved
    // because http.Server is external and no Server symbol exists in this file
    assert_eq!(
        field_edges.len(),
        1,
        "Expected 1 FieldType edge, got: {:?}",
        field_edges
    );
    match &field_edges[0].from {
        crate::a6s::types::SymbolRef::Resolved(id) => assert!(
            id.as_str().contains(":Srv:"),
            "from should be Srv field, got {:?}",
            id
        ),
        other => panic!("from should be Resolved, got {:?}", other),
    }
    // to should be Unresolved because http.Server is external
    match &field_edges[0].to {
        crate::a6s::types::SymbolRef::Unresolved { name, .. } => assert_eq!(
            name, "Server",
            "to should be unresolved Server, got {:?}",
            name
        ),
        other => panic!("to should be Unresolved, got {:?}", other),
    }
}

#[test]
fn test_field_type_skips_builtins() {
    let code = r#"
package main

type Config struct {
    Name   string
    Count  int
    Active bool
}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::FieldType)
        .collect();

    assert_eq!(
        field_edges.len(),
        0,
        "Builtin field types should not produce FieldType edges, got: {:?}",
        field_edges
    );
}

#[test]
fn test_field_type_multiple_custom_types() {
    let code = r#"
package main

type Config struct{}
type Handler struct{}
type Logger struct{}

type Service struct {
    Config  Config
    Handler Handler
    Logger  Logger
}
"#;
    let extractor = GolangExtractor;
    let parsed = extractor.extract(code, "main.go");

    let field_edges: Vec<_> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::FieldType)
        .collect();

    assert_eq!(
        field_edges.len(),
        3,
        "Expected 3 FieldType edges, got: {:?}",
        field_edges
    );

    // All should have from = field (Resolved) and to = type (Resolved)
    for edge in &field_edges {
        assert!(
            edge.from.is_resolved(),
            "from should be Resolved, got {:?}",
            edge.from
        );
        assert!(
            edge.to.is_resolved(),
            "to should be Resolved, got {:?}",
            edge.to
        );
    }

    // Verify each specific field → type edge
    for name in &["Config", "Handler", "Logger"] {
        assert!(
            field_edges.iter().any(|e| {
                matches!(&e.from, crate::a6s::types::SymbolRef::Resolved(id) if id.as_str().contains(&format!(":{}:", name)))
            }),
            "Expected FieldType edge from {} field",
            name
        );
    }
}

// ============================================================================
// Phase 6: Cross-file resolution tests
// ============================================================================

/// Two files in the same package (same directory). A Calls edge from file1
/// to a function defined in file2 should resolve via same-package lookup.
#[test]
fn test_resolve_cross_file_calls_same_package() {
    let extractor = GolangExtractor;

    // File 1: main.go calls helper()
    let code1 = r#"
package main

func main() {
    helper()
}
"#;

    // File 2: utils.go defines helper()
    let code2 = r#"
package main

func helper() {
    // do stuff
}
"#;

    let file1 = extractor.extract(code1, "cmd/server/main.go");
    let file2 = extractor.extract(code2, "cmd/server/utils.go");

    // Verify file1 has an unresolved Calls edge to "helper"
    let calls_edges: Vec<_> = file1
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Calls)
        .collect();
    assert!(
        !calls_edges.is_empty(),
        "Expected at least one Calls edge in file1"
    );

    // The call to helper() should be unresolved (helper is in a different file)
    let helper_call = calls_edges
        .iter()
        .find(|e| {
            matches!(&e.to, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "helper")
        });
    assert!(
        helper_call.is_some(),
        "Expected unresolved Calls edge to 'helper', edges: {:?}",
        calls_edges
    );

    // Now resolve cross-file
    let mut files = vec![file1, file2];
    let (resolved, imports) = extractor.resolve_cross_file(&mut files);

    // The helper() call should now be resolved
    let resolved_calls: Vec<_> = resolved
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::Calls)
        .collect();

    let helper_resolved = resolved_calls
        .iter()
        .find(|e| e.to.as_str().contains(":helper:"));
    assert!(
        helper_resolved.is_some(),
        "Expected resolved Calls edge to 'helper', resolved_calls: {:?}",
        resolved_calls
    );

    // Imports should be empty
    assert!(imports.is_empty(), "Go imports should be empty");
}

/// Two files in DIFFERENT packages. With unique bare names, the bare-name
/// fallback should resolve the cross-package call.
#[test]
fn test_resolve_cross_file_different_packages_unique_name() {
    let extractor = GolangExtractor;

    // File 1 in cmd/server — calls UniqueHelper()
    let code1 = r#"
package main

func main() {
    UniqueHelper()
}
"#;

    // File 2 in pkg/utils — defines UniqueHelper()
    let code2 = r#"
package utils

func UniqueHelper() {}
"#;

    let file1 = extractor.extract(code1, "cmd/server/main.go");
    let file2 = extractor.extract(code2, "pkg/utils/helpers.go");

    let mut files = vec![file1, file2];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // Should resolve via bare-name fallback (only one "UniqueHelper" exists)
    let helper_resolved = resolved.iter().find(|e| {
        e.kind == crate::a6s::types::EdgeKind::Calls && e.to.as_str().contains(":UniqueHelper:")
    });
    assert!(
        helper_resolved.is_some(),
        "Expected resolved Calls edge to 'UniqueHelper' via bare-name fallback, resolved: {:?}",
        resolved
    );
}

/// When a bare name appears in multiple packages, it should NOT resolve
/// (ambiguous).
#[test]
fn test_resolve_cross_file_ambiguous_name_no_resolve() {
    let extractor = GolangExtractor;

    // File 1 calls Helper()
    let code1 = r#"
package main

func main() {
    Helper()
}
"#;

    // File 2 defines Helper() in pkg/a
    let code2 = r#"
package a

func Helper() {}
"#;

    // File 3 defines Helper() in pkg/b
    let code3 = r#"
package b

func Helper() {}
"#;

    let file1 = extractor.extract(code1, "cmd/main.go");
    let file2 = extractor.extract(code2, "pkg/a/helpers.go");
    let file3 = extractor.extract(code3, "pkg/b/helpers.go");

    let mut files = vec![file1, file2, file3];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // The call to Helper() should NOT resolve (ambiguous: 2 candidates)
    let helper_resolved = resolved.iter().find(|e| {
        e.kind == crate::a6s::types::EdgeKind::Calls && e.to.as_str().contains(":Helper:")
    });
    assert!(
        helper_resolved.is_none(),
        "Expected NO resolved Calls edge to 'Helper' (ambiguous), resolved: {:?}",
        resolved
    );
}

/// Cross-file Usage edge resolution: a struct literal in file1 referring
/// to a type defined in file2 (same package).
#[test]
fn test_resolve_cross_file_usage_same_package() {
    let extractor = GolangExtractor;

    // File 1 uses Config{}
    let code1 = r#"
package server

func NewServer() {
    cfg := Config{}
    _ = cfg
}
"#;

    // File 2 defines Config struct
    let code2 = r#"
package server

type Config struct {
    Port int
}
"#;

    let file1 = extractor.extract(code1, "internal/server/server.go");
    let file2 = extractor.extract(code2, "internal/server/config.go");

    let mut files = vec![file1, file2];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // Usage edge from NewServer to Config should resolve
    let config_usage = resolved.iter().find(|e| {
        e.kind == crate::a6s::types::EdgeKind::Usage && e.to.as_str().contains(":Config:")
    });
    assert!(
        config_usage.is_some(),
        "Expected resolved Usage edge to 'Config', resolved: {:?}",
        resolved
    );
}

/// Cross-file HasMethod resolution: a method with a receiver type defined
/// in another file (same package).
#[test]
fn test_resolve_cross_file_hasmethod_receiver() {
    let extractor = GolangExtractor;

    // File 1 defines the struct
    let code1 = r#"
package server

type Server struct {
    Port int
}
"#;

    // File 2 defines a method on Server
    let code2 = r#"
package server

func (s *Server) Start() {
    // start the server
}
"#;

    let file1 = extractor.extract(code1, "internal/server/types.go");
    let file2 = extractor.extract(code2, "internal/server/server.go");

    // In file2, the HasMethod edge should have unresolved `from` (Server is in file1)
    let hasmethod_edges: Vec<_> = file2
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasMethod)
        .collect();
    assert!(
        !hasmethod_edges.is_empty(),
        "Expected HasMethod edges in file2"
    );

    // The from should be unresolved (Server is not in file2)
    let unresolved_from = hasmethod_edges.iter().find(|e| {
        matches!(&e.from, crate::a6s::types::SymbolRef::Unresolved { name, .. } if name == "Server")
    });
    assert!(
        unresolved_from.is_some(),
        "Expected unresolved HasMethod from 'Server', edges: {:?}",
        hasmethod_edges
    );

    let mut files = vec![file1, file2];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // After resolution, the HasMethod edge should resolve
    let server_method = resolved.iter().find(|e| {
        e.kind == crate::a6s::types::EdgeKind::HasMethod
            && e.from.as_str().contains(":Server:")
            && e.to.as_str().contains(":Start:")
    });
    assert!(
        server_method.is_some(),
        "Expected resolved HasMethod edge Server→Start, resolved: {:?}",
        resolved
    );
}

/// Already-resolved edges should pass through unchanged.
#[test]
fn test_resolve_cross_file_already_resolved_edges() {
    let extractor = GolangExtractor;

    // A file with struct+field → HasField edges are already resolved
    let code = r#"
package main

type Config struct {
    Port int
    Host string
}
"#;

    let file = extractor.extract(code, "config.go");

    // HasField edges should already be resolved
    let hasfield_count = file
        .edges
        .iter()
        .filter(|e| {
            e.kind == crate::a6s::types::EdgeKind::HasField
                && e.from.is_resolved()
                && e.to.is_resolved()
        })
        .count();
    assert!(hasfield_count > 0, "Expected resolved HasField edges");

    let mut files = vec![file];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // Already-resolved HasField edges should appear in output
    let resolved_hasfield = resolved
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::HasField)
        .count();
    assert_eq!(
        resolved_hasfield, hasfield_count,
        "All already-resolved HasField edges should pass through"
    );
}

/// resolve_cross_file should return empty imports (Go import resolution
/// is future work).
#[test]
fn test_resolve_cross_file_no_imports_returned() {
    let extractor = GolangExtractor;

    let code = r#"
package main

import "fmt"

func main() {
    fmt.Println("hello")
}
"#;

    let file = extractor.extract(code, "main.go");
    let mut files = vec![file];
    let (_, imports) = extractor.resolve_cross_file(&mut files);

    assert!(
        imports.is_empty(),
        "Go imports should be empty, got: {:?}",
        imports
    );
}

/// derive_module_path should return the directory path as the Go package.
#[test]
fn test_derive_module_path_go() {
    let extractor = GolangExtractor;

    // Nested path
    assert_eq!(
        extractor.derive_module_path("cmd/server/main.go"),
        Some("cmd/server".to_string())
    );

    // Root-level file
    assert_eq!(
        extractor.derive_module_path("main.go"),
        Some("".to_string())
    );

    // Deeply nested
    assert_eq!(
        extractor.derive_module_path("internal/pkg/handler/routes.go"),
        Some("internal/pkg/handler".to_string())
    );
}

/// ParamType edge cross-file resolution: a function parameter type
/// defined in another file of the same package should resolve.
#[test]
fn test_resolve_cross_file_param_type() {
    let extractor = GolangExtractor;

    // File 1: function with a Config parameter
    let code1 = r#"
package server

func StartServer(cfg Config) {
    _ = cfg
}
"#;

    // File 2: defines Config struct
    let code2 = r#"
package server

type Config struct {
    Port int
}
"#;

    let file1 = extractor.extract(code1, "internal/server/server.go");
    let file2 = extractor.extract(code2, "internal/server/config.go");

    // file1 should have a ParamType edge with unresolved 'to' (Config is in file2)
    let param_edges: Vec<_> = file1
        .edges
        .iter()
        .filter(|e| e.kind == crate::a6s::types::EdgeKind::ParamType)
        .collect();
    assert!(
        !param_edges.is_empty(),
        "Expected ParamType edges in file1, edges: {:?}",
        file1.edges
    );

    let mut files = vec![file1, file2];
    let (resolved, _) = extractor.resolve_cross_file(&mut files);

    // ParamType edge should resolve
    let config_param = resolved.iter().find(|e| {
        e.kind == crate::a6s::types::EdgeKind::ParamType && e.to.as_str().contains(":Config:")
    });
    assert!(
        config_param.is_some(),
        "Expected resolved ParamType edge to 'Config', resolved: {:?}",
        resolved
    );
}
