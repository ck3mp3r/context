//! Symbol registry for cross-file resolution
//!
//! STUB: This is a placeholder implementation.
//! Real resolution logic will be added later.

use crate::a6s::types::{ParsedFile, QualifiedName, ResolveStats, SymbolId, SymbolName, SymbolRef};
use std::collections::HashMap;

pub struct ImportTable {
    pub name_to_module: HashMap<String, String>,
    pub glob_modules: Vec<String>,
}

impl ImportTable {
    pub fn new() -> Self {
        Self {
            name_to_module: HashMap::new(),
            glob_modules: Vec::new(),
        }
    }
}

impl Default for ImportTable {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SymbolRegistry {
    qualified_map: HashMap<QualifiedName, SymbolId>,
    #[allow(dead_code)]
    bare_to_qualified: HashMap<SymbolName, Vec<QualifiedName>>,
    #[allow(dead_code)]
    symbol_kinds: HashMap<SymbolId, String>,
    #[allow(dead_code)]
    symbol_languages: HashMap<SymbolId, String>,
    #[allow(dead_code)]
    import_tables: HashMap<String, ImportTable>,
}

impl SymbolRegistry {
    /// Build registry from parsed files. STUB: returns empty registry.
    pub fn build(_parsed_files: &[ParsedFile]) -> Self {
        Self {
            qualified_map: HashMap::new(),
            bare_to_qualified: HashMap::new(),
            symbol_kinds: HashMap::new(),
            symbol_languages: HashMap::new(),
            import_tables: HashMap::new(),
        }
    }

    /// Resolve a SymbolRef to a SymbolId. STUB: always returns None.
    pub fn resolve(&self, _sym_ref: &SymbolRef, _file_path: &str) -> Option<SymbolId> {
        None
    }

    /// Get resolution statistics.
    pub fn stats(&self) -> ResolveStats {
        ResolveStats {
            symbols_registered: self.qualified_map.len(),
            edges_resolved: 0,
            edges_dropped: 0,
            imports_resolved: 0,
        }
    }
}
