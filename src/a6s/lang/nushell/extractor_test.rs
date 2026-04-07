use super::extractor::NushellExtractor;
use crate::a6s::extract::LanguageExtractor;
use crate::a6s::types::{EdgeKind, SymbolRef};

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/a6s/lang/nushell/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

/// Extract the name component from a SymbolRef.
fn extract_name_from_ref(sym_ref: &SymbolRef) -> Option<&str> {
    match sym_ref {
        SymbolRef::Resolved(id) => {
            let s = id.as_str().strip_prefix("symbol:")?;
            let last_colon = s.rfind(':')?;
            let before_last = &s[..last_colon];
            let second_last_colon = before_last.rfind(':')?;
            Some(&before_last[second_last_colon + 1..])
        }
        SymbolRef::Unresolved { name, .. } => Some(name),
    }
}

/// Get all edges of a given kind as (from_name, to_name) pairs.
fn edges_of_kind(parsed: &crate::a6s::types::ParsedFile, kind: EdgeKind) -> Vec<(&str, &str)> {
    parsed
        .edges
        .iter()
        .filter(|e| e.kind == kind)
        .filter_map(|e| {
            let from = extract_name_from_ref(&e.from)?;
            let to = extract_name_from_ref(&e.to)?;
            Some((from, to))
        })
        .collect()
}

#[test]
fn test_nushell_extractor_language() {
    let extractor = NushellExtractor;
    assert_eq!(extractor.language(), "nushell");
}

#[test]
fn test_nushell_extractor_extensions() {
    let extractor = NushellExtractor;
    assert_eq!(extractor.extensions(), &["nu"]);
}

#[test]
fn test_nushell_extractor_queries() {
    let extractor = NushellExtractor;
    assert!(!extractor.symbol_queries().is_empty());
    // Nushell has no type_refs query
    assert_eq!(extractor.type_ref_queries(), "");
}

#[test]
fn test_query_compiles() {
    let extractor = NushellExtractor;
    let language = extractor.grammar();
    assert!(
        tree_sitter::Query::new(&language, extractor.symbol_queries()).is_ok(),
        "Nushell .scm queries must compile against the grammar"
    );
}

#[test]
fn test_extracts_command() {
    let extractor = NushellExtractor;
    let code = r#"
def greet [name: string] {
    $"Hello, ($name)!"
}
"#;
    let parsed = extractor.extract(code, "utils.nu");
    assert_eq!(parsed.symbols.len(), 1);
    assert_eq!(parsed.symbols[0].name, "greet");
    assert_eq!(parsed.symbols[0].kind, "function");
    assert_eq!(parsed.symbols[0].language, "nushell");
}

#[test]
fn test_extracts_module() {
    let extractor = NushellExtractor;
    let code = r#"
module network {
    export def ping [host: string] { }
}
"#;
    let parsed = extractor.extract(code, "lib.nu");
    assert!(parsed
        .symbols
        .iter()
        .any(|s| s.name == "network" && s.kind == "module"));
}

#[test]
fn test_extracts_const() {
    let extractor = NushellExtractor;
    let code = "const MAX_RETRIES = 5";
    let parsed = extractor.extract(code, "config.nu");
    assert!(parsed
        .symbols
        .iter()
        .any(|s| s.name == "MAX_RETRIES" && s.kind == "const"));
}

#[test]
fn test_extracts_alias() {
    let extractor = NushellExtractor;
    let code = "alias ll = ls -l";
    let parsed = extractor.extract(code, "aliases.nu");
    assert!(parsed
        .symbols
        .iter()
        .any(|s| s.name == "ll" && s.kind == "alias"));
}

#[test]
fn test_extracts_command_calls() {
    let extractor = NushellExtractor;
    let code = r#"
def main [] {
    let files = ls
    $files | length
}
"#;
    let parsed = extractor.extract(code, "main.nu");
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);
    assert!(
        calls.iter().any(|(_, to)| *to == "ls"),
        "should capture ls call, got: {:?}",
        calls
    );
}

#[test]
fn test_extracts_use_import() {
    let extractor = NushellExtractor;
    let code = "use std";
    let parsed = extractor.extract(code, "main.nu");
    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].entry.module_path, "std");
}

#[test]
fn test_combined_nushell_extraction() {
    let extractor = NushellExtractor;
    let code = r#"
use std

const VERSION = "1.0.0"

module utils {
    export def process [input: string] {
        $input | str trim
    }
}

def main [] {
    let result = utils process "hello  "
    print $result
}

alias p = print
"#;
    let parsed = extractor.extract(code, "app.nu");

    // Symbols
    assert!(parsed
        .symbols
        .iter()
        .any(|s| s.name == "VERSION" && s.kind == "const"));
    assert!(parsed
        .symbols
        .iter()
        .any(|s| s.name == "utils" && s.kind == "module"));
    assert!(parsed
        .symbols
        .iter()
        .any(|s| s.name == "main" && s.kind == "command"));
    assert!(parsed
        .symbols
        .iter()
        .any(|s| s.name == "p" && s.kind == "alias"));

    // Imports
    assert!(parsed.imports.iter().any(|i| i.entry.module_path == "std"));

    // Calls - should have edges for command calls
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);
    assert!(
        !calls.is_empty(),
        "should have call edges, got: {:?}",
        calls
    );
}

#[test]
fn test_entry_type_main() {
    let extractor = NushellExtractor;
    let code = load_testdata("app.nu");
    let parsed = extractor.extract(&code, "app.nu");

    let main_sym = parsed.symbols.iter().find(|s| s.name == "main");
    assert!(main_sym.is_some(), "main should be in symbols");
    let main_sym = main_sym.unwrap();
    assert_eq!(
        main_sym.entry_type,
        Some("main".to_string()),
        "def main should have entry_type 'main'"
    );

    // Check line range
    eprintln!(
        "def main: start_line={}, end_line={}",
        main_sym.start_line, main_sym.end_line
    );
    assert!(
        main_sym.end_line > main_sym.start_line,
        "def main end_line should be greater than start_line, got start={} end={}",
        main_sym.start_line,
        main_sym.end_line
    );

    let helper_sym = parsed.symbols.iter().find(|s| s.name == "process-items");
    assert!(helper_sym.is_some(), "process-items should be in symbols");
    assert_eq!(
        helper_sym.unwrap().entry_type,
        None,
        "regular command should not have entry_type"
    );

    // Debug: Print all edges
    eprintln!("All Calls edges:");
    for (from, to) in edges_of_kind(&parsed, EdgeKind::Calls) {
        eprintln!("  {} -> {}", from, to);
    }

    // CRITICAL: Verify that calls INSIDE def main are extracted
    let main_name = &main_sym.name;
    let all_calls = edges_of_kind(&parsed, EdgeKind::Calls);
    let calls_from_main: Vec<_> = all_calls
        .iter()
        .filter(|(from, _)| *from == main_name)
        .collect();

    assert!(
        !calls_from_main.is_empty(),
        "def main should have outgoing call edges, found {}. All calls: {:?}",
        calls_from_main.len(),
        all_calls
    );
}

#[test]
fn test_nushell_visibility() {
    let extractor = NushellExtractor;
    let code = load_testdata("app.nu");
    let parsed = extractor.extract(&code, "app.nu");

    let vis = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.visibility.clone())
    };

    assert_eq!(
        vis("ping"),
        Some("public".to_string()),
        "export def should be public"
    );
    assert_eq!(
        vis("process-items"),
        Some("private".to_string()),
        "def without export should be private"
    );
    assert_eq!(
        vis("main"),
        Some("private".to_string()),
        "def main without export should be private"
    );
}

#[test]
fn test_module_children_containment() {
    let extractor = NushellExtractor;
    let code = load_testdata("app.nu");
    let parsed = extractor.extract(&code, "app.nu");

    let network_children: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == EdgeKind::HasMember
                && matches!(&e.from, SymbolRef::Resolved(id) if id.as_str().contains("network"))
        })
        .filter_map(|e| extract_name_from_ref(&e.to))
        .collect();

    assert!(
        network_children.contains(&"ping"),
        "network module should contain command 'ping', got: {:?}",
        network_children
    );
    assert!(
        network_children.contains(&"fetch"),
        "network module should contain command 'fetch', got: {:?}",
        network_children
    );
    assert_eq!(
        network_children.len(),
        2,
        "network module should contain exactly 2 children, got: {:?}",
        network_children
    );
}

#[test]
fn test_mod_nu_extraction() {
    let extractor = NushellExtractor;
    let code = load_testdata("mymod/mod.nu");
    let parsed = extractor.extract(&code, "mymod/mod.nu");

    let sym_names: Vec<(&str, &str)> = parsed
        .symbols
        .iter()
        .map(|s| (s.name.as_str(), s.kind.as_str()))
        .collect();

    assert!(
        sym_names.contains(&("greet", "function")),
        "mod.nu should extract 'greet' function, got: {:?}",
        sym_names
    );
    assert!(
        sym_names.contains(&("VERSION", "const")),
        "mod.nu should extract 'VERSION' const, got: {:?}",
        sym_names
    );
    assert!(
        sym_names.contains(&("inner", "module")),
        "mod.nu should extract 'inner' inline module, got: {:?}",
        sym_names
    );
    assert!(
        sym_names.contains(&("helper", "function")),
        "mod.nu should extract 'helper' function inside inner module, got: {:?}",
        sym_names
    );
    assert!(
        sym_names.contains(&("INNER_CONST", "const")),
        "mod.nu should extract 'INNER_CONST' inside inner module, got: {:?}",
        sym_names
    );
    assert!(
        sym_names.contains(&("hi", "alias")),
        "mod.nu should extract 'hi' alias, got: {:?}",
        sym_names
    );

    // inner module should contain helper and INNER_CONST via line-range containment
    let inner_children: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == EdgeKind::HasMember
                && matches!(&e.from, SymbolRef::Resolved(id) if id.as_str().contains("inner"))
        })
        .filter_map(|e| extract_name_from_ref(&e.to))
        .collect();

    assert!(
        inner_children.contains(&"helper"),
        "inner module should contain 'helper', got: {:?}",
        inner_children
    );
    assert!(
        inner_children.contains(&"INNER_CONST"),
        "inner module should contain 'INNER_CONST', got: {:?}",
        inner_children
    );
}

#[test]
fn test_sibling_file_extraction() {
    let extractor = NushellExtractor;
    let code = load_testdata("mymod/utils.nu");
    let parsed = extractor.extract(&code, "mymod/utils.nu");

    let sym_names: Vec<(&str, &str)> = parsed
        .symbols
        .iter()
        .map(|s| (s.name.as_str(), s.kind.as_str()))
        .collect();

    assert!(
        sym_names.contains(&("process", "function")),
        "utils.nu should extract 'process' function, got: {:?}",
        sym_names
    );
    assert!(
        sym_names.contains(&("nested", "module")),
        "utils.nu should extract 'nested' inline module, got: {:?}",
        sym_names
    );
    assert!(
        sym_names.contains(&("deep-func", "function")),
        "utils.nu should extract 'deep-func' inside nested module, got: {:?}",
        sym_names
    );

    // nested module should contain deep-func
    let nested_children: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| {
            e.kind == EdgeKind::HasMember
                && matches!(&e.from, SymbolRef::Resolved(id) if id.as_str().contains("nested"))
        })
        .filter_map(|e| extract_name_from_ref(&e.to))
        .collect();

    assert!(
        nested_children.contains(&"deep-func"),
        "nested module should contain 'deep-func', got: {:?}",
        nested_children
    );
}

// ============================================================================
// Import Resolution Tests (TDD)
// ============================================================================

#[test]
fn test_resolve_imports_glob() {
    use crate::a6s::registry::SymbolRegistry;
    use crate::a6s::types::{ImportEntry, ParsedFile, RawImport, RawSymbol};

    let extractor = NushellExtractor;

    // Create parsed files with symbols in "std" module
    let mut std_file = ParsedFile::new("std/mod.nu", "nushell");
    std_file.symbols.push(RawSymbol {
        name: "print".to_string(),
        kind: "function".to_string(),
        file_path: "std/mod.nu".to_string(),
        start_line: 1,
        end_line: 3,
        signature: None,
        language: "nushell".to_string(),
        visibility: Some("public".to_string()),
        entry_type: None,
    });
    std_file.symbols.push(RawSymbol {
        name: "length".to_string(),
        kind: "function".to_string(),
        file_path: "std/mod.nu".to_string(),
        start_line: 5,
        end_line: 7,
        signature: None,
        language: "nushell".to_string(),
        visibility: Some("public".to_string()),
        entry_type: None,
    });

    // Build registry
    let registry = SymbolRegistry::build(&[std_file]);

    // Create import: use std *
    let imports = vec![RawImport {
        file_path: "main.nu".to_string(),
        entry: ImportEntry::glob_import("std"),
    }];

    // Resolve
    let resolved = extractor.resolve_imports(&imports, &registry);

    // Should resolve to 2 symbols (print + length)
    assert_eq!(resolved.len(), 2, "glob import should resolve to 2 symbols");
    assert!(
        resolved
            .iter()
            .any(|r| r.file_id.as_str() == "file:main.nu"),
        "all resolved imports should reference the importing file"
    );
}

#[test]
fn test_resolve_imports_named() {
    use crate::a6s::registry::SymbolRegistry;
    use crate::a6s::types::{ImportEntry, ParsedFile, RawImport, RawSymbol};

    let extractor = NushellExtractor;

    // Create parsed file with symbols
    let mut std_file = ParsedFile::new("std/mod.nu", "nushell");
    std_file.symbols.push(RawSymbol {
        name: "print".to_string(),
        kind: "function".to_string(),
        file_path: "std/mod.nu".to_string(),
        start_line: 1,
        end_line: 3,
        signature: None,
        language: "nushell".to_string(),
        visibility: Some("public".to_string()),
        entry_type: None,
    });
    std_file.symbols.push(RawSymbol {
        name: "length".to_string(),
        kind: "function".to_string(),
        file_path: "std/mod.nu".to_string(),
        start_line: 5,
        end_line: 7,
        signature: None,
        language: "nushell".to_string(),
        visibility: Some("public".to_string()),
        entry_type: None,
    });

    let registry = SymbolRegistry::build(&[std_file]);

    // Create import: use std [print]
    let imports = vec![RawImport {
        file_path: "main.nu".to_string(),
        entry: ImportEntry::named_import("std", vec!["print".to_string()]),
    }];

    let resolved = extractor.resolve_imports(&imports, &registry);

    // Should resolve to 1 symbol (print only)
    assert_eq!(resolved.len(), 1, "named import should resolve to 1 symbol");
    assert_eq!(
        resolved[0].file_id.as_str(),
        "file:main.nu",
        "resolved import should reference the importing file"
    );
}

#[test]
fn test_resolve_imports_module_import() {
    use crate::a6s::registry::SymbolRegistry;
    use crate::a6s::types::{ImportEntry, ParsedFile, RawImport, RawSymbol};

    let extractor = NushellExtractor;

    // Create module symbol
    let mut std_file = ParsedFile::new("std/mod.nu", "nushell");
    std_file.symbols.push(RawSymbol {
        name: "std".to_string(),
        kind: "module".to_string(),
        file_path: "std/mod.nu".to_string(),
        start_line: 1,
        end_line: 100,
        signature: None,
        language: "nushell".to_string(),
        visibility: Some("public".to_string()),
        entry_type: None,
    });

    let registry = SymbolRegistry::build(&[std_file]);

    // Create import: use std
    let imports = vec![RawImport {
        file_path: "main.nu".to_string(),
        entry: ImportEntry::module_import("std"),
    }];

    let resolved = extractor.resolve_imports(&imports, &registry);

    // Should resolve to the module symbol itself
    assert_eq!(
        resolved.len(),
        1,
        "module import should resolve to 1 symbol"
    );
}

#[test]
fn test_resolve_imports_nonexistent() {
    use crate::a6s::registry::SymbolRegistry;
    use crate::a6s::types::{ImportEntry, RawImport};

    let extractor = NushellExtractor;
    let registry = SymbolRegistry::build(&[]); // Empty registry

    let imports = vec![RawImport {
        file_path: "main.nu".to_string(),
        entry: ImportEntry::named_import("nonexistent", vec!["foo".to_string()]),
    }];

    let resolved = extractor.resolve_imports(&imports, &registry);

    // Should resolve to nothing
    assert_eq!(
        resolved.len(),
        0,
        "import from empty registry should resolve nothing"
    );
}

#[test]
fn test_extracts_glob_import() {
    let extractor = NushellExtractor;
    let code = "use std *";
    let parsed = extractor.extract(code, "main.nu");
    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].entry.module_path, "std");
    assert!(parsed.imports[0].entry.is_glob);
    assert!(parsed.imports[0].entry.imported_names.is_empty());
}

#[test]
fn test_help_main_call() {
    let extractor = NushellExtractor;
    let code = r#"def main [] {
  help main
}"#;
    let parsed = extractor.extract(code, "test.nu");

    // Should have one symbol: main
    assert_eq!(parsed.symbols.len(), 1, "Should have 1 symbol");
    assert_eq!(parsed.symbols[0].name, "main");
    assert_eq!(parsed.symbols[0].entry_type, Some("main".to_string()));

    // Should have one call edge: main -> help
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);
    println!("Calls edges: {:#?}", calls);
    assert_eq!(calls.len(), 1, "Should have 1 call edge");
    assert_eq!(calls[0].0, "main", "Caller should be 'main'");
    assert_eq!(calls[0].1, "help", "Callee should be 'help'");
}

#[test]
fn test_symbol_classification_private_function() {
    let extractor = NushellExtractor;
    let code = r#"def foo [] {
    print "hello"
}"#;
    let parsed = extractor.extract(code, "test.nu");

    assert_eq!(parsed.symbols.len(), 1, "Should have 1 symbol");
    let sym = &parsed.symbols[0];
    assert_eq!(sym.name, "foo");
    assert_eq!(
        sym.kind, "function",
        "def foo should be classified as function"
    );
    assert_eq!(
        sym.visibility,
        Some("private".to_string()),
        "def foo should be private"
    );
    assert_eq!(
        sym.entry_type, None,
        "regular function should not have entry_type"
    );
}

#[test]
fn test_symbol_classification_public_function() {
    let extractor = NushellExtractor;
    let code = r#"export def bar [] {
    print "world"
}"#;
    let parsed = extractor.extract(code, "test.nu");

    assert_eq!(parsed.symbols.len(), 1, "Should have 1 symbol");
    let sym = &parsed.symbols[0];
    assert_eq!(sym.name, "bar");
    assert_eq!(
        sym.kind, "function",
        "export def bar should be classified as function"
    );
    assert_eq!(
        sym.visibility,
        Some("public".to_string()),
        "export def should be public"
    );
    assert_eq!(
        sym.entry_type, None,
        "regular function should not have entry_type"
    );
}

#[test]
fn test_symbol_classification_main_command() {
    let extractor = NushellExtractor;
    let code = r#"def main [] {
    print "app"
}"#;
    let parsed = extractor.extract(code, "test.nu");

    assert_eq!(parsed.symbols.len(), 1, "Should have 1 symbol");
    let sym = &parsed.symbols[0];
    assert_eq!(sym.name, "main");
    assert_eq!(
        sym.kind, "command",
        "def main should be classified as command"
    );
    assert_eq!(
        sym.entry_type,
        Some("main".to_string()),
        "def main should have entry_type 'main'"
    );
}

#[test]
fn test_symbol_classification_subcommand() {
    let extractor = NushellExtractor;
    let code = r#"def "main list" [] {
    print "listing"
}"#;
    let parsed = extractor.extract(code, "test.nu");

    assert_eq!(
        parsed.symbols.len(),
        1,
        "Should have 1 symbol, got: {:?}",
        parsed.symbols
    );
    let sym = &parsed.symbols[0];
    assert_eq!(sym.name, "main list");
    assert_eq!(
        sym.kind, "command",
        "def 'main list' should be classified as command"
    );
    assert_eq!(
        sym.visibility,
        Some("private".to_string()),
        "non-exported subcommand should be private"
    );
}

#[test]
fn test_symbol_classification_exported_subcommand() {
    let extractor = NushellExtractor;
    let code = r#"export def "git status" [] {
    ^git status
}"#;
    let parsed = extractor.extract(code, "test.nu");

    assert_eq!(parsed.symbols.len(), 1, "Should have 1 symbol");
    let sym = &parsed.symbols[0];
    assert_eq!(sym.name, "git status");
    assert_eq!(
        sym.kind, "command",
        "export def 'git status' should be classified as command"
    );
    assert_eq!(
        sym.visibility,
        Some("public".to_string()),
        "export def should be public"
    );
}

#[test]
fn test_extracts_quoted_command_calls() {
    let extractor = NushellExtractor;
    let code = r#"
def "ci log error" [msg: string] {
    print $"ERROR: ($msg)"
}

def "ci scm config" [key: string, value: string] {
    if ($value | is-empty) {
        "Value is required" | ci log error
    }
    print $"Setting ($key) = ($value)"
}
"#;
    let parsed = extractor.extract(code, "ci.nu");

    // Should have 2 symbols (both commands with space-separated names)
    assert_eq!(parsed.symbols.len(), 2, "Should have 2 symbols");
    let log_error = parsed.symbols.iter().find(|s| s.name == "ci log error");
    let scm_config = parsed.symbols.iter().find(|s| s.name == "ci scm config");
    assert!(log_error.is_some(), "Should find 'ci log error' symbol");
    assert!(scm_config.is_some(), "Should find 'ci scm config' symbol");

    // The critical test: should extract the call from "ci scm config" to "ci log error"
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);
    eprintln!("All call edges: {:?}", calls);

    assert!(
        calls.iter().any(|(from, to)| *from == "ci scm config" && *to == "ci log error"),
        "Should extract call from 'ci scm config' to 'ci log error' (quoted command call in pipeline), got: {:?}",
        calls
    );
}

#[test]
fn test_extracts_function_call_with_variable_arg() {
    let extractor = NushellExtractor;
    let code = r#"
def make-prompt [text: string] {
    print $"Prompt: ($text)"
}

def process [] {
    let input = "hello"
    let result = make-prompt $input
}
"#;
    let parsed = extractor.extract(code, "test.nu");

    // Should have 2 symbols
    assert_eq!(parsed.symbols.len(), 2, "Should have 2 symbols");

    // Should extract call from process to make-prompt
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);
    eprintln!("All call edges: {:?}", calls);

    assert!(
        calls
            .iter()
            .any(|(from, to)| *from == "process" && *to == "make-prompt"),
        "Should extract call from 'process' to 'make-prompt', got: {:?}",
        calls
    );
}
