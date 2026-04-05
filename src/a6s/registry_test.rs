//! Tests for SymbolRegistry stub

use crate::a6s::registry::{ImportTable, SymbolRegistry};
use crate::a6s::types::{SymbolId, SymbolRef};

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

    // STUB: Even resolved refs return None for now
    let result = registry.resolve(&sym_ref, "src/main.rs");
    assert!(result.is_none());
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
