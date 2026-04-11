//! Tests for SymbolRegistry stub

use crate::a6s::registry::{ImportTable, SymbolRegistry};
use crate::a6s::types::{ParsedFile, RawSymbol, SymbolId, SymbolRef};

#[test]
fn test_build_empty() {
    let registry = SymbolRegistry::build(&[]);
    let stats = registry.stats();

    assert_eq!(stats.symbols_registered, 0);
    assert_eq!(stats.edges_resolved, 0);
    assert_eq!(stats.edges_dropped, 0);
    assert_eq!(stats.imports_resolved, 0);
}

#[test]
fn test_resolve_always_none() {
    let registry = SymbolRegistry::build(&[]);
    let sym_ref = SymbolRef::unresolved("Foo", "src/main.rs");

    let result = registry.resolve(&sym_ref, "src/main.rs");
    assert!(result.is_none());
}

#[test]
fn test_resolve_resolved_passthrough() {
    let registry = SymbolRegistry::build(&[]);
    let id = SymbolId::new("src/main.rs", "main", 1);
    let sym_ref = SymbolRef::Resolved(id.clone());

    // Resolved refs are passed through
    let result = registry.resolve(&sym_ref, "src/main.rs");
    assert_eq!(result, Some(id));
}

#[test]
fn test_import_table_new() {
    let table = ImportTable::new();

    assert_eq!(table.name_to_module.len(), 0);
    assert_eq!(table.glob_modules.len(), 0);
}

#[test]
fn test_import_table_default() {
    let table = ImportTable::default();

    assert_eq!(table.name_to_module.len(), 0);
    assert_eq!(table.glob_modules.len(), 0);
}

#[test]
fn test_stats_empty() {
    let registry = SymbolRegistry::build(&[]);
    let stats = registry.stats();

    assert_eq!(stats.symbols_registered, 0);
}

#[test]
fn test_resolve_cross_file_call() {
    // File A defines function "helper"
    let file_a = ParsedFile {
        file_path: "src/a.rs".to_string(),
        language: "rust".to_string(),
        symbols: vec![RawSymbol {
            name: "helper".to_string(),
            kind: "function".to_string(),
            file_path: "src/a.rs".to_string(),
            start_line: 1,
            end_line: 5,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        }],
        edges: vec![],
        imports: vec![],
        file_category: None,
    };

    // File B calls "helper" (cross-file call)
    let file_b = ParsedFile {
        file_path: "src/b.rs".to_string(),
        language: "rust".to_string(),
        symbols: vec![],
        edges: vec![],
        imports: vec![],
        file_category: None,
    };

    let registry = SymbolRegistry::build(&[file_a, file_b]);

    // Cross-file call: caller in b.rs, callee "helper" defined in a.rs
    let sym_ref = SymbolRef::unresolved("helper", "src/b.rs");
    let result = registry.resolve(&sym_ref, "src/b.rs");

    // Should resolve via Tier 1b global bare name search
    assert!(result.is_some(), "Cross-file call should resolve");
    let resolved = result.unwrap();
    assert_eq!(resolved.file_path().unwrap(), "src/a.rs");
    assert_eq!(resolved.as_str(), "symbol:src/a.rs:helper:1");
}

#[test]
fn test_resolve_prefers_same_file_over_global() {
    // File A defines "helper"
    let file_a = ParsedFile {
        file_path: "src/a.rs".to_string(),
        language: "rust".to_string(),
        symbols: vec![RawSymbol {
            name: "helper".to_string(),
            kind: "function".to_string(),
            file_path: "src/a.rs".to_string(),
            start_line: 1,
            end_line: 5,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        }],
        edges: vec![],
        imports: vec![],
        file_category: None,
    };

    // File B also defines "helper" (duplicate name)
    let file_b = ParsedFile {
        file_path: "src/b.rs".to_string(),
        language: "rust".to_string(),
        symbols: vec![RawSymbol {
            name: "helper".to_string(),
            kind: "function".to_string(),
            file_path: "src/b.rs".to_string(),
            start_line: 10,
            end_line: 15,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        }],
        edges: vec![],
        imports: vec![],
        file_category: None,
    };

    let registry = SymbolRegistry::build(&[file_a, file_b]);

    // Call from b.rs should resolve to b.rs (same-file priority)
    let sym_ref = SymbolRef::unresolved("helper", "src/b.rs");
    let result = registry.resolve(&sym_ref, "src/b.rs");

    assert!(result.is_some(), "Should resolve to same-file symbol");
    let resolved = result.unwrap();
    assert_eq!(
        resolved.file_path().unwrap(),
        "src/b.rs",
        "Should prefer same-file"
    );
    assert_eq!(resolved.as_str(), "symbol:src/b.rs:helper:10");
}

#[test]
fn test_resolve_ambiguous_name_returns_first_match() {
    // File A defines "util"
    let file_a = ParsedFile {
        file_path: "src/a.rs".to_string(),
        language: "rust".to_string(),
        symbols: vec![RawSymbol {
            name: "util".to_string(),
            kind: "function".to_string(),
            file_path: "src/a.rs".to_string(),
            start_line: 1,
            end_line: 5,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        }],
        edges: vec![],
        imports: vec![],
        file_category: None,
    };

    // File B also defines "util" (ambiguous)
    let file_b = ParsedFile {
        file_path: "src/b.rs".to_string(),
        language: "rust".to_string(),
        symbols: vec![RawSymbol {
            name: "util".to_string(),
            kind: "function".to_string(),
            file_path: "src/b.rs".to_string(),
            start_line: 1,
            end_line: 5,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        }],
        edges: vec![],
        imports: vec![],
        file_category: None,
    };

    // File C calls "util" (no local definition, ambiguous global)
    let file_c = ParsedFile {
        file_path: "src/c.rs".to_string(),
        language: "rust".to_string(),
        symbols: vec![],
        edges: vec![],
        imports: vec![],
        file_category: None,
    };

    let registry = SymbolRegistry::build(&[file_a, file_b, file_c]);

    // Call from c.rs should resolve to first match (a.rs or b.rs)
    let sym_ref = SymbolRef::unresolved("util", "src/c.rs");
    let result = registry.resolve(&sym_ref, "src/c.rs");

    // Should return SOME result (first match), not None
    assert!(
        result.is_some(),
        "Ambiguous name should return first match, not drop edge"
    );
    let resolved = result.unwrap();
    let resolved_file = resolved.file_path().unwrap();
    // Could be either a.rs or b.rs - just verify it resolved
    assert!(
        resolved_file == "src/a.rs" || resolved_file == "src/b.rs",
        "Should resolve to one of the candidates"
    );
}
