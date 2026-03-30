use super::parser::Nushell;

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/analysis/lang/nushell/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

#[test]
fn test_query_compiles() {
    let language = Nushell::grammar();
    assert!(
        tree_sitter::Query::new(&language, Nushell::queries()).is_ok(),
        "Nushell .scm queries must compile against the grammar"
    );
}

#[test]
fn test_extracts_command() {
    let code = r#"
def greet [name: string] {
    $"Hello, ($name)!"
}
"#;
    let parsed = Nushell::extract(code, "utils.nu");
    assert_eq!(parsed.symbols.len(), 1);
    assert_eq!(parsed.symbols[0].name, "greet");
    assert_eq!(parsed.symbols[0].kind, "command");
    assert_eq!(parsed.symbols[0].language, "nushell");
}

#[test]
fn test_extracts_module() {
    let code = r#"
module network {
    export def ping [host: string] { }
}
"#;
    let parsed = Nushell::extract(code, "lib.nu");
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "network" && s.kind == "module")
    );
}

#[test]
fn test_extracts_const() {
    let code = "const MAX_RETRIES = 5";
    let parsed = Nushell::extract(code, "config.nu");
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "MAX_RETRIES" && s.kind == "const")
    );
}

#[test]
fn test_extracts_alias() {
    let code = "alias ll = ls -l";
    let parsed = Nushell::extract(code, "aliases.nu");
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "ll" && s.kind == "alias")
    );
}

#[test]
fn test_extracts_command_calls() {
    let code = r#"
def main [] {
    let files = ls
    $files | length
}
"#;
    let parsed = Nushell::extract(code, "main.nu");
    assert!(parsed.calls.iter().any(|c| c.callee_name == "ls"));
}

#[test]
fn test_extracts_use_import() {
    let code = "use std";
    let parsed = Nushell::extract(code, "main.nu");
    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].entry.module_path, "std");
}

#[test]
fn test_combined_nushell_extraction() {
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
    let parsed = Nushell::extract(code, "app.nu");

    // Symbols
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "VERSION" && s.kind == "const")
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "utils" && s.kind == "module")
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "main" && s.kind == "command")
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|s| s.name == "p" && s.kind == "alias")
    );

    // Imports
    assert!(parsed.imports.iter().any(|i| i.entry.module_path == "std"));

    // Calls
    assert!(!parsed.calls.is_empty());
}

#[test]
fn test_entry_type_main() {
    let code = load_testdata("app.nu");
    let parsed = Nushell::extract(&code, "app.nu");

    let main_sym = parsed.symbols.iter().find(|s| s.name == "main");
    assert!(main_sym.is_some(), "main should be in symbols");
    assert_eq!(
        main_sym.unwrap().entry_type,
        Some("main".to_string()),
        "def main should have entry_type 'main'"
    );

    let helper_sym = parsed.symbols.iter().find(|s| s.name == "process-items");
    assert!(helper_sym.is_some(), "process-items should be in symbols");
    assert_eq!(
        helper_sym.unwrap().entry_type,
        None,
        "regular command should not have entry_type"
    );
}

#[test]
fn test_nushell_visibility() {
    let code = load_testdata("app.nu");
    let parsed = Nushell::extract(&code, "app.nu");

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
    let code = load_testdata("app.nu");
    let parsed = Nushell::extract(&code, "app.nu");

    let network_children: Vec<&str> = parsed
        .containments
        .iter()
        .filter(|c| c.parent_name == "network")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
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
    let code = load_testdata("mymod/mod.nu");
    let parsed = Nushell::extract(&code, "mymod/mod.nu");

    let sym_names: Vec<(&str, &str)> = parsed
        .symbols
        .iter()
        .map(|s| (s.name.as_str(), s.kind.as_str()))
        .collect();

    assert!(
        sym_names.contains(&("greet", "command")),
        "mod.nu should extract 'greet' command, got: {:?}",
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
        sym_names.contains(&("helper", "command")),
        "mod.nu should extract 'helper' command inside inner module, got: {:?}",
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
        .containments
        .iter()
        .filter(|c| c.parent_name == "inner")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
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
    let code = load_testdata("mymod/utils.nu");
    let parsed = Nushell::extract(&code, "mymod/utils.nu");

    let sym_names: Vec<(&str, &str)> = parsed
        .symbols
        .iter()
        .map(|s| (s.name.as_str(), s.kind.as_str()))
        .collect();

    assert!(
        sym_names.contains(&("process", "command")),
        "utils.nu should extract 'process' command, got: {:?}",
        sym_names
    );
    assert!(
        sym_names.contains(&("nested", "module")),
        "utils.nu should extract 'nested' inline module, got: {:?}",
        sym_names
    );
    assert!(
        sym_names.contains(&("deep-func", "command")),
        "utils.nu should extract 'deep-func' inside nested module, got: {:?}",
        sym_names
    );

    // nested module should contain deep-func
    let nested_children: Vec<&str> = parsed
        .containments
        .iter()
        .filter(|c| c.parent_name == "nested")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
        .collect();

    assert!(
        nested_children.contains(&"deep-func"),
        "nested module should contain 'deep-func', got: {:?}",
        nested_children
    );
}

// --- File-based module resolution tests ---
// These simulate the pipeline's Phase 2 (register) + Phase 3 (resolve containments)
// to verify cross-file containment edges are resolvable.

use crate::analysis::pipeline::SymbolRegistry;
use crate::analysis::types::*;

fn resolve_containments(parsed_files: &[ParsedFile]) -> Vec<(String, String)> {
    let mut registry = SymbolRegistry::new();

    for pf in parsed_files {
        let module_path = derive_module_path(&pf.file_path, &pf.language);
        for sym in &pf.symbols {
            let sid = sym.symbol_id();
            let qn = QualifiedName::new(&module_path, &sym.name);
            registry.register(qn, sid, &sym.kind, &sym.language);
        }
    }

    let mut edges = Vec::new();
    for pf in parsed_files {
        let module_path = derive_module_path(&pf.file_path, &pf.language);
        for cont in &pf.containments {
            let parent_qn = QualifiedName::new(&module_path, &cont.parent_name);
            if registry.qualified_map.get(&parent_qn).is_some() {
                let child = &pf.symbols[cont.child_symbol_idx];
                edges.push((cont.parent_name.clone(), child.name.clone()));
            } else {
                panic!(
                    "UNRESOLVABLE containment: parent '{}' (qualified '{}') \
                     not found in registry for child '{}' in file '{}'. \
                     Available qualified names: {:?}",
                    cont.parent_name,
                    parent_qn,
                    pf.symbols[cont.child_symbol_idx].name,
                    pf.file_path,
                    registry
                        .qualified_map
                        .keys()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    edges
}

#[test]
fn test_inline_module_containment_resolves() {
    let code = load_testdata("mymod/mod.nu");
    let mut parsed_files = vec![Nushell::extract(&code, "mymod/mod.nu")];
    Nushell::resolve_file_modules(&mut parsed_files);

    let edges = resolve_containments(&parsed_files);

    assert!(
        edges.contains(&("inner".to_string(), "helper".to_string())),
        "inner module should contain 'helper', got: {:?}",
        edges
    );
    assert!(
        edges.contains(&("inner".to_string(), "INNER_CONST".to_string())),
        "inner module should contain 'INNER_CONST', got: {:?}",
        edges
    );
}

#[test]
fn test_file_module_containment_mod_nu_resolves() {
    let mod_code = load_testdata("mymod/mod.nu");
    let mut parsed_files = vec![Nushell::extract(&mod_code, "mymod/mod.nu")];
    Nushell::resolve_file_modules(&mut parsed_files);

    let edges = resolve_containments(&parsed_files);

    assert!(
        edges.contains(&("mymod".to_string(), "greet".to_string())),
        "mymod should contain 'greet' from mod.nu, got: {:?}",
        edges
    );
    assert!(
        edges.contains(&("mymod".to_string(), "VERSION".to_string())),
        "mymod should contain 'VERSION' from mod.nu, got: {:?}",
        edges
    );
    assert!(
        edges.contains(&("mymod".to_string(), "inner".to_string())),
        "mymod should contain inline module 'inner', got: {:?}",
        edges
    );
    assert!(
        edges.contains(&("mymod".to_string(), "hi".to_string())),
        "mymod should contain alias 'hi', got: {:?}",
        edges
    );
    assert!(
        !edges.contains(&("mymod".to_string(), "helper".to_string())),
        "helper should NOT be direct child of mymod (it's inside inner)"
    );
}

#[test]
fn test_file_module_containment_sibling_resolves() {
    let mod_code = load_testdata("mymod/mod.nu");
    let utils_code = load_testdata("mymod/utils.nu");

    let mut parsed_files = vec![
        Nushell::extract(&mod_code, "mymod/mod.nu"),
        Nushell::extract(&utils_code, "mymod/utils.nu"),
    ];
    Nushell::resolve_file_modules(&mut parsed_files);

    let edges = resolve_containments(&parsed_files);

    assert!(
        edges.contains(&("mymod".to_string(), "process".to_string())),
        "mymod should contain 'process' from utils.nu, got: {:?}",
        edges
    );
    assert!(
        edges.contains(&("mymod".to_string(), "nested".to_string())),
        "mymod should contain inline module 'nested' from utils.nu, got: {:?}",
        edges
    );
    assert!(
        !edges.contains(&("mymod".to_string(), "deep-func".to_string())),
        "deep-func should NOT be direct child of mymod (it's inside nested)"
    );

    assert!(
        edges.contains(&("nested".to_string(), "deep-func".to_string())),
        "nested module should contain 'deep-func', got: {:?}",
        edges
    );
}
