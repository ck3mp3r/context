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
