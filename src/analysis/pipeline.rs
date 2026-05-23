//! Multi-phase analysis pipeline
//!
//! Orchestrates: extract -> register -> resolve -> load

use crate::analysis::lang::{Analyser, LanguageAnalyser, supported_extensions};
use crate::analysis::store::CodeGraph;
use crate::analysis::types::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Store error: {0}")]
    Store(#[from] crate::analysis::store::StoreError),
}

pub struct PipelineResult {
    pub files_analyzed: usize,
    pub symbols_extracted: usize,
    pub relationships_created: usize,
}

/// Per-file import table for cross-file name resolution
#[derive(Debug, Default)]
pub struct ImportTable {
    pub name_to_module: HashMap<String, String>,
    pub glob_modules: Vec<String>,
}

/// Global symbol registry built from all parsed files
pub struct SymbolRegistry {
    pub qualified_map: HashMap<QualifiedName, SymbolId>,
    pub bare_to_qualified: HashMap<SymbolName, Vec<QualifiedName>>,
    pub symbol_kinds: HashMap<SymbolId, String>,
    pub symbol_languages: HashMap<SymbolId, String>,
    pub import_tables: HashMap<String, ImportTable>,
}

impl SymbolRegistry {
    pub(crate) fn new() -> Self {
        Self {
            qualified_map: HashMap::new(),
            bare_to_qualified: HashMap::new(),
            symbol_kinds: HashMap::new(),
            symbol_languages: HashMap::new(),
            import_tables: HashMap::new(),
        }
    }

    pub(crate) fn register(&mut self, qn: QualifiedName, id: SymbolId, kind: &str, language: &str) {
        let bare = SymbolName::new(qn.bare_name());
        self.bare_to_qualified
            .entry(bare)
            .or_default()
            .push(qn.clone());
        self.symbol_kinds.insert(id.clone(), kind.to_string());
        self.symbol_languages
            .insert(id.clone(), language.to_string());
        self.qualified_map.insert(qn, id);
    }

    /// Resolve a symbol name with optional kind filter.
    /// When expected_kinds is Some, only symbols with matching kind are considered.
    fn resolve_with_imports_and_kind(
        &self,
        bare_name: &str,
        caller_module: &str,
        file_path: &str,
        caller_language: &str,
        expected_kinds: Option<&[&str]>,
    ) -> Option<&SymbolId> {
        let name = SymbolName::new(bare_name);

        // Helper to check if a symbol matches expected kinds
        let kind_matches = |id: &SymbolId| -> bool {
            match expected_kinds {
                None => true,
                Some(kinds) => self
                    .symbol_kinds
                    .get(id)
                    .is_some_and(|k| kinds.contains(&k.as_str())),
            }
        };

        // 1. Same module (prefer matching kind)
        let same_qn = QualifiedName::new(caller_module, bare_name);
        if let Some(id) = self.qualified_map.get(&same_qn)
            && kind_matches(id)
        {
            return Some(id);
        }

        // 2. Import table
        if let Some(table) = self.import_tables.get(file_path) {
            if let Some(module) = table.name_to_module.get(bare_name) {
                let qn = QualifiedName::new(module, bare_name);
                if let Some(id) = self.qualified_map.get(&qn)
                    && kind_matches(id)
                {
                    return Some(id);
                }
            }
            for glob_mod in &table.glob_modules {
                let qn = QualifiedName::new(glob_mod, bare_name);
                if let Some(id) = self.qualified_map.get(&qn)
                    && kind_matches(id)
                {
                    return Some(id);
                }
            }
        }

        // 3. Bare name fallback — same language only, with kind filter
        if let Some(candidates) = self.bare_to_qualified.get(&name) {
            let matching: Vec<_> = candidates
                .iter()
                .filter(|qn| {
                    self.qualified_map.get(*qn).is_some_and(|id| {
                        self.symbol_languages
                            .get(id)
                            .is_some_and(|lang| lang == caller_language)
                            && kind_matches(id)
                    })
                })
                .collect();

            if matching.len() == 1 {
                return self.qualified_map.get(matching[0]);
            }
        }

        None
    }
}

/// Helper to resolve a SymbolId with optional kind filter
fn resolve_with_kind(
    sid: &SymbolId,
    registry: &SymbolRegistry,
    module_path: &str,
    file_path: &str,
    language: &str,
    expected_kinds: Option<&[&str]>,
) -> Option<SymbolId> {
    if sid.as_str().ends_with(":0") {
        // Extract the name from "symbol:file:name:0"
        let s = sid.as_str().strip_prefix("symbol:")?;
        let last_colon = s.rfind(':')?;
        let before_last = &s[..last_colon];
        let second_last_colon = before_last.rfind(':')?;
        let target_name = &before_last[second_last_colon + 1..];

        registry
            .resolve_with_imports_and_kind(
                target_name,
                module_path,
                file_path,
                language,
                expected_kinds,
            )
            .cloned()
    } else {
        Some(sid.clone())
    }
}

/// Run the full analysis pipeline
pub fn run(
    repo_path: &Path,
    _repo_id: &str,
    graph: &mut CodeGraph,
) -> Result<PipelineResult, PipelineError> {
    let files = scan_supported_files(repo_path)?;

    // Phase 1: Extract (includes language-specific multi-file resolution)
    let parsed_files = extract_all(&files, repo_path);

    // Phase 2: Register symbols
    let mut registry = SymbolRegistry::new();
    let mut total_symbols = 0;
    let mut total_rels = 0;

    for pf in &parsed_files {
        let analyser = match Analyser::for_language(&pf.language) {
            Some(a) => a,
            None => continue,
        };
        let module_path = analyser.derive_module_path(&pf.file_path);
        let file_id = graph.insert_file(&pf.file_path, &pf.language, "todo")?;

        for sym in &pf.symbols {
            let sid = sym.symbol_id();
            let qn = QualifiedName::new(&module_path, &sym.name);
            registry.register(qn, sid.clone(), &sym.kind, &sym.language);

            graph.insert_symbol(&Symbol {
                name: sym.name.clone(),
                kind: sym.kind.clone(),
                language: sym.language.clone(),
                file_path: sym.file_path.clone(),
                start_line: sym.start_line,
                end_line: sym.end_line,
                content: String::new(),
                signature: sym.signature.clone(),
                visibility: sym.visibility.clone(),
                entry_type: sym.entry_type.clone(),
            })?;
            graph.insert_contains(&file_id, &sid, 1.0)?;
            total_symbols += 1;
        }

        // Build import table
        let mut import_table = ImportTable::default();
        for imp in &pf.imports {
            let internal_module = analyser.normalise_import_path(&imp.entry.module_path);

            if imp.entry.is_glob {
                import_table.glob_modules.push(internal_module.clone());
            }

            // Map imported names to their module path.
            // For glob imports with package names (e.g., Go: import "pkg/common"),
            // this enables scoped call resolution: common.Failure → pkg::common::Failure
            for name in &imp.entry.imported_names {
                import_table
                    .name_to_module
                    .insert(name.clone(), internal_module.clone());
            }
        }
        registry
            .import_tables
            .insert(pf.file_path.clone(), import_table);
    }

    // Phase 2b: Load semantic edges (emitted directly by analysers)
    // These edges have resolved SymbolIds - no lookup needed for most.
    // Heritage edges with line 0 need resolution via the registry.
    for pf in &parsed_files {
        let analyser = match Analyser::for_language(&pf.language) {
            Some(a) => a,
            None => continue,
        };
        let module_path = analyser.derive_module_path(&pf.file_path);

        // Helper to extract name from SymbolId and resolve if needed
        let resolve_if_needed = |sid: &SymbolId| -> Option<SymbolId> {
            resolve_with_kind(
                sid,
                &registry,
                &module_path,
                &pf.file_path,
                &pf.language,
                None,
            )
        };

        // Helper to resolve with expected kinds (for type references)
        let resolve_type_ref = |sid: &SymbolId| -> Option<SymbolId> {
            // For type references, prefer struct/interface/type over field
            static TYPE_KINDS: &[&str] = &["struct", "interface", "type"];
            resolve_with_kind(
                sid,
                &registry,
                &module_path,
                &pf.file_path,
                &pf.language,
                Some(TYPE_KINDS),
            )
        };

        for edge in &pf.edges {
            match edge.kind {
                EdgeKind::HasField | EdgeKind::HasMethod | EdgeKind::HasMember => {
                    // Containment edges: parent symbolContains child
                    // Resolve from if it has line 0 (e.g., impl methods need type resolution)
                    let from_id = resolve_if_needed(&edge.from);
                    let to_id = resolve_if_needed(&edge.to);

                    if let (Some(from), Some(to)) = (from_id, to_id) {
                        graph.insert_symbol_contains_edge(&from, &to, 1.0)?;
                        total_rels += 1;
                    }
                }
                EdgeKind::Implements | EdgeKind::Extends => {
                    // Heritage edges: type inherits from parent
                    let kind = match edge.kind {
                        EdgeKind::Implements => InheritanceType::Implements,
                        EdgeKind::Extends => InheritanceType::Extends,
                        _ => unreachable!(),
                    };

                    let from_id = resolve_if_needed(&edge.from);
                    let to_id = resolve_if_needed(&edge.to);

                    if let (Some(from), Some(to)) = (from_id, to_id) {
                        graph.insert_inherits_edge(&from, &to, &kind, 1.0)?;
                        total_rels += 1;
                    }
                }
                EdgeKind::Calls => {
                    // Calls edge: caller calls callee
                    let from_id = resolve_if_needed(&edge.from);
                    let to_id = resolve_if_needed(&edge.to);

                    if let (Some(from), Some(to)) = (from_id, to_id) {
                        // Extract line from original from SymbolId
                        let line = edge
                            .from
                            .as_str()
                            .rsplit(':')
                            .next()
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0);
                        graph.insert_calls_edge(&from, &to, line, 1.0)?;
                        total_rels += 1;
                    }
                }
                EdgeKind::TypeRef
                | EdgeKind::FieldType
                | EdgeKind::ParamType
                | EdgeKind::ReturnType => {
                    // Type reference edges: prefer struct/interface/type over field
                    let from_id = resolve_if_needed(&edge.from);
                    let to_id = resolve_type_ref(&edge.to);

                    if let (Some(from), Some(to)) = (from_id, to_id) {
                        graph.insert_references_edge(&from, &to, &edge.kind, 1.0)?;
                        total_rels += 1;
                    }
                }
                EdgeKind::Usage | EdgeKind::Import => {
                    // Value/import reference edges: any symbol kind is valid
                    let from_id = resolve_if_needed(&edge.from);
                    let to_id = resolve_if_needed(&edge.to);

                    if let (Some(from), Some(to)) = (from_id, to_id) {
                        graph.insert_references_edge(&from, &to, &edge.kind, 1.0)?;
                        total_rels += 1;
                    }
                }
                EdgeKind::FileImports => {
                    // File-level imports are handled separately in Phase 3
                    // (File -> Symbol edges, not Symbol -> Symbol)
                    // RawEdge is Symbol -> Symbol, so this case shouldn't occur
                    unreachable!(
                        "FileImports edges should not appear in RawEdge; \
                         they are emitted via insert_file_imports_edge in Phase 3"
                    );
                }
            }
        }
    }

    // Phase 3: Resolve imports and create FileImports edges
    // File-level imports use FileImports (File -> Symbol)
    // Scoped imports would use Import (Symbol -> Symbol) - not yet implemented
    for pf in &parsed_files {
        let analyser = match Analyser::for_language(&pf.language) {
            Some(a) => a,
            None => continue,
        };

        // Create FileId for this file (matches the one created in Phase 1)
        let file_id = FileId::new(&pf.file_path);

        for imp in &pf.imports {
            // Normalize the import path (strips crate::, self::, super:: for Rust, etc.)
            let normalized_path = analyser.normalise_import_path(&imp.entry.module_path);

            // Resolve import targets
            // For Go (is_glob=true): resolve_import_targets finds matching package symbols
            // For Rust/others: resolve_import_targets uses imported_names
            let targets = analyser.resolve_import_targets(
                &normalized_path,
                &imp.entry.imported_names,
                &registry.qualified_map,
                &registry.symbol_languages,
                &registry.symbol_kinds,
            );

            for target_id in targets {
                // File-level import: File -> Symbol
                graph.insert_file_imports_edge(&file_id, &target_id, 1.0)?;
                total_rels += 1;
            }
        }
    }

    Ok(PipelineResult {
        files_analyzed: parsed_files.len(),
        symbols_extracted: total_symbols,
        relationships_created: total_rels,
    })
}

fn extract_all(files: &[PathBuf], repo_path: &Path) -> Vec<ParsedFile> {
    // Disabled during a6s migration: use crate::analysis::lang::nushell::Nushell;
    use crate::analysis::lang::rust::Rust;

    let mut results = Vec::new();

    for file_path in files {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let relative = file_path
            .strip_prefix(repo_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let analyser = match Analyser::for_extension(ext) {
            Some(a) => a,
            None => continue,
        };

        results.push(analyser.extract(&content, &relative));
    }

    // Language-specific multi-file resolution
    // These are static methods that operate on all files at once
    // Disabled during a6s migration: Nushell::resolve_file_modules(&mut results);
    Rust::resolve_file_modules(&mut results);

    results
}

fn scan_supported_files(repo_path: &Path) -> Result<Vec<PathBuf>, PipelineError> {
    let mut files = Vec::new();
    let walker = ignore::WalkBuilder::new(repo_path)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .build();

    let exts = supported_extensions();

    for entry in walker {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && exts.contains(&ext)
                {
                    files.push(path.to_path_buf());
                }
            }
            Err(e) => {
                tracing::warn!("Error walking directory: {}", e);
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
#[path = "pipeline_test.rs"]
mod tests;
