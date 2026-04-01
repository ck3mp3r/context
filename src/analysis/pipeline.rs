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

    fn resolve_with_imports(
        &self,
        bare_name: &str,
        caller_module: &str,
        file_path: &str,
        caller_language: &str,
    ) -> Option<&SymbolId> {
        let name = SymbolName::new(bare_name);

        // 1. Same module
        let same_qn = QualifiedName::new(caller_module, bare_name);
        if let Some(id) = self.qualified_map.get(&same_qn) {
            return Some(id);
        }

        // 2. Import table
        if let Some(table) = self.import_tables.get(file_path) {
            if let Some(module) = table.name_to_module.get(bare_name) {
                let qn = QualifiedName::new(module, bare_name);
                if let Some(id) = self.qualified_map.get(&qn) {
                    return Some(id);
                }
            }
            for glob_mod in &table.glob_modules {
                let qn = QualifiedName::new(glob_mod, bare_name);
                if let Some(id) = self.qualified_map.get(&qn) {
                    return Some(id);
                }
            }
        }

        // 3. Bare name fallback — same language only
        if let Some(candidates) = self.bare_to_qualified.get(&name) {
            let same_lang: Vec<_> = candidates
                .iter()
                .filter(|qn| {
                    self.qualified_map
                        .get(*qn)
                        .and_then(|id| self.symbol_languages.get(id))
                        .is_some_and(|lang| lang == caller_language)
                })
                .collect();

            if same_lang.len() == 1 {
                return self.qualified_map.get(same_lang[0]);
            }
        }

        None
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

    // Phase 3: Resolve containments
    for pf in &parsed_files {
        let analyser = match Analyser::for_language(&pf.language) {
            Some(a) => a,
            None => continue,
        };
        let module_path = analyser.derive_module_path(&pf.file_path);
        for cont in &pf.containments {
            let parent_qn = QualifiedName::new(&module_path, &cont.parent_name);
            if let Some(parent_id) = registry.qualified_map.get(&parent_qn) {
                let child_sym = &pf.symbols[cont.child_symbol_idx];
                let child_id = child_sym.symbol_id();
                graph.insert_symbol_contains_edge(parent_id, &child_id, 1.0)?;
                total_rels += 1;
            }
        }
    }

    // Phase 4: Resolve heritage
    for pf in &parsed_files {
        let analyser = match Analyser::for_language(&pf.language) {
            Some(a) => a,
            None => continue,
        };
        let module_path = analyser.derive_module_path(&pf.file_path);
        for h in &pf.heritage {
            let type_id = registry.resolve_with_imports(
                &h.type_name,
                &module_path,
                &pf.file_path,
                &pf.language,
            );
            let parent_id = registry.resolve_with_imports(
                &h.parent_name,
                &module_path,
                &pf.file_path,
                &pf.language,
            );
            if let (Some(tid), Some(pid)) = (type_id, parent_id) {
                graph.insert_inherits_edge(tid, pid, &h.kind, 1.0)?;
                total_rels += 1;
            }
        }
    }

    // Phase 5: Resolve imports
    for pf in &parsed_files {
        let analyser = match Analyser::for_language(&pf.language) {
            Some(a) => a,
            None => continue,
        };
        let module_path = analyser.derive_module_path(&pf.file_path);

        // Use trait method to find import source symbol
        let source_id = analyser.find_import_source(
            &pf.symbols,
            &pf.file_path,
            &module_path,
            &registry.qualified_map,
        );

        if let Some(source_id) = source_id {
            for imp in &pf.imports {
                // Use trait method to resolve import targets
                // For Go (is_glob=true): resolve_import_targets finds matching package
                // For Rust/others: resolve_import_targets uses imported_names
                let targets = analyser.resolve_import_targets(
                    &imp.entry.module_path,
                    &imp.entry.imported_names,
                    &registry.qualified_map,
                    &registry.symbol_languages,
                    &registry.symbol_kinds,
                );

                for target_id in targets {
                    graph.insert_references_edge(
                        &source_id,
                        &target_id,
                        &ReferenceType::Import,
                        1.0,
                    )?;
                    total_rels += 1;
                }
            }
        }
    }

    // Phase 6: Resolve calls
    for pf in &parsed_files {
        let analyser = match Analyser::for_language(&pf.language) {
            Some(a) => a,
            None => continue,
        };
        let module_path = analyser.derive_module_path(&pf.file_path);
        for call in &pf.calls {
            // Find enclosing symbol (caller)
            let caller_id = call
                .enclosing_symbol_idx
                .and_then(|idx| pf.symbols.get(idx))
                .map(|s| s.symbol_id())
                .or_else(|| find_enclosing_symbol(&pf.symbols, call.call_site_line));

            let caller_id = match caller_id {
                Some(id) => id,
                None => continue,
            };

            let callee_id = match call.call_form {
                CallForm::Free => registry.resolve_with_imports(
                    &call.callee_name,
                    &module_path,
                    &pf.file_path,
                    &pf.language,
                ),
                CallForm::Scoped => {
                    if let Some(qualifier) = &call.qualifier {
                        // Translate qualifier via import table (e.g., "common" → "pkg::common")
                        let resolved_module = registry
                            .import_tables
                            .get(&pf.file_path)
                            .and_then(|t| t.name_to_module.get(qualifier))
                            .map(|s| s.as_str())
                            .unwrap_or(qualifier);

                        let qn = QualifiedName::new(resolved_module, &call.callee_name);
                        registry.qualified_map.get(&qn)
                    } else {
                        registry.resolve_with_imports(
                            &call.callee_name,
                            &module_path,
                            &pf.file_path,
                            &pf.language,
                        )
                    }
                }
                CallForm::Method => registry.resolve_with_imports(
                    &call.callee_name,
                    &module_path,
                    &pf.file_path,
                    &pf.language,
                ),
            };

            if let Some(callee_id) = callee_id {
                graph.insert_calls_edge(&caller_id, callee_id, call.call_site_line, 1.0)?;
                total_rels += 1;
            }
        }
    }

    // Phase 7: Resolve type refs
    for pf in &parsed_files {
        let analyser = match Analyser::for_language(&pf.language) {
            Some(a) => a,
            None => continue,
        };
        let module_path = analyser.derive_module_path(&pf.file_path);
        for tr in &pf.type_refs {
            let from_sym = &pf.symbols[tr.from_symbol_idx];
            let from_id = from_sym.symbol_id();
            if let Some(to_id) = registry.resolve_with_imports(
                &tr.type_name,
                &module_path,
                &pf.file_path,
                &pf.language,
            ) {
                graph.insert_references_edge(&from_id, to_id, &tr.ref_kind, 1.0)?;
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

fn find_enclosing_symbol(symbols: &[RawSymbol], line: usize) -> Option<SymbolId> {
    symbols
        .iter()
        .filter(|s| s.start_line <= line && s.end_line >= line)
        .min_by_key(|s| s.end_line - s.start_line)
        .map(|s| s.symbol_id())
}

fn extract_all(files: &[PathBuf], repo_path: &Path) -> Vec<ParsedFile> {
    use crate::analysis::lang::nushell::Nushell;
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
    Nushell::resolve_file_modules(&mut results);
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
