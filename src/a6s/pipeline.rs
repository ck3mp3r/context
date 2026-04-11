use super::error::A6sError;
use super::extract;
use super::extract::LanguageExtractor;
use super::registry::SymbolRegistry;
use super::store::CodeGraph;
use super::types::*;
use std::path::{Path, PathBuf};
use tokio::task::JoinSet;
use tracing::{debug, info};

/// Main entry point for code analysis pipeline (production).
pub async fn analyze(
    repo_path: &Path,
    analysis_path: &Path,
    commit_hash: &str,
    progress_tx: Option<tokio::sync::mpsc::Sender<PipelineProgress>>,
) -> Result<ResolveStats, A6sError> {
    // Remove existing database if it exists (like old analysis layer does)
    let db_path = analysis_path.join("analysis.nano");
    if db_path.exists() {
        info!("Removing existing graph at {:?}", db_path);
        std::fs::remove_dir_all(&db_path)?;
    }

    let graph = CodeGraph::new(analysis_path.to_path_buf());
    analyze_with_graph(repo_path, commit_hash, progress_tx, graph).await
}

/// Main entry point for code analysis pipeline with injectable graph.
///
/// Orchestrates the full analysis flow:
/// 1. Scan repository for supported files
/// 2. Extract symbols/edges in parallel (Layer 1)
/// 3. Build symbol registry
/// 4. Resolve edges and imports (Layer 2)
/// 5. Load graph and commit
pub async fn analyze_with_graph<C: super::store::NanoGraphCli>(
    repo_path: &Path,
    commit_hash: &str,
    progress_tx: Option<tokio::sync::mpsc::Sender<PipelineProgress>>,
    mut graph: super::store::CodeGraph<C>,
) -> Result<ResolveStats, A6sError> {
    info!("=== a6s Pipeline Starting ===");
    info!("Repo path: {:?}", repo_path);
    info!("Commit: {}", commit_hash);

    // Phase 1: Scan for supported files
    info!("Phase 1: Scanning for supported files (.rs, .go, .nu)...");
    let files = scan_supported_files(repo_path);
    info!("✓ Scanned {} files", files.len());
    if let Some(ref tx) = progress_tx {
        let _ = tx.send(PipelineProgress::Scanned(files.len())).await;
    }

    // Phase 2: Parallel extraction (Layer 1)
    info!("Phase 2: Parallel extraction via spawn_blocking...");
    let mut parsed_files = extract_parallel(files, repo_path).await;
    info!("✓ Extracted {} files (stub: all empty)", parsed_files.len());
    if let Some(ref tx) = progress_tx {
        let _ = tx
            .send(PipelineProgress::Extracted(parsed_files.len()))
            .await;
    }

    // Phase 3: Resolve file modules (per-language)
    info!("Phase 3: Resolving file modules (stub)...");
    resolve_file_modules(&mut parsed_files);

    // Phase 4: Build symbol registry (Layer 2 setup)
    info!("Phase 4: Building symbol registry...");
    let registry = build_registry(&parsed_files);
    info!(
        "✓ Built registry: {} symbols registered",
        registry.stats().symbols_registered
    );

    // Phase 5: Resolve edges (Layer 2 resolution)
    info!("Phase 5: Resolving edges...");
    let (resolved_edges, edges_dropped) = resolve_edges(&parsed_files, &registry);
    info!(
        "✓ Resolved {} edges, dropped {}",
        resolved_edges.len(),
        edges_dropped
    );

    // Phase 6: Resolve imports (Layer 2 resolution)
    info!("Phase 6: Resolving imports (stub)...");
    let import_edges = resolve_imports(&parsed_files, &registry);
    info!("✓ Resolved {} import edges", import_edges.len());

    let stats = ResolveStats {
        symbols_registered: registry.stats().symbols_registered,
        edges_resolved: resolved_edges.len(),
        edges_dropped,
        imports_resolved: import_edges.len(),
    };

    if let Some(ref tx) = progress_tx {
        let _ = tx.send(PipelineProgress::Resolved(stats.clone())).await;
    }

    // Phase 7: Load graph and commit
    info!("Phase 7: Loading graph buffer...");
    load_and_commit(
        &mut graph,
        &parsed_files,
        &resolved_edges,
        &import_edges,
        commit_hash,
        &progress_tx,
    )?;
    info!("=== a6s Pipeline Complete ===");
    info!(
        "Final stats: {} symbols, {} edges resolved, {} dropped, {} imports",
        stats.symbols_registered, stats.edges_resolved, stats.edges_dropped, stats.imports_resolved
    );

    Ok(stats)
}

/// Scan repository for files with supported extensions.
///
/// Skips: .git/, target/, node_modules/, vendor/ (respects .gitignore)
/// Returns only files with extensions recognized by registered extractors.
fn scan_supported_files(repo_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let walker = ignore::WalkBuilder::new(repo_path)
        .follow_links(false)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|s| s.to_str())
            && extract::supported_extensions().contains(&ext)
        {
            files.push(path.to_path_buf());
        }
    }

    debug!("Scanned {} supported files", files.len());
    files
}

/// Extract symbols and edges from files in parallel.
///
/// Fans out to `spawn_blocking` tasks, each running tree-sitter extraction.
/// STUB: In scaffolding, extractors return empty ParsedFile, so this will succeed but produce no data.
async fn extract_parallel(files: Vec<PathBuf>, repo_path: &Path) -> Vec<ParsedFile> {
    let mut join_set = JoinSet::new();
    let repo_root = repo_path.to_path_buf();

    for file in files {
        let repo_root_clone = repo_root.clone();
        join_set.spawn_blocking(move || -> Option<ParsedFile> {
            let ext = file.extension()?.to_str()?;
            let extractor = extract::Extractor::for_extension(ext)?;
            let code = std::fs::read_to_string(&file).ok()?;
            let rel_path = file
                .strip_prefix(&repo_root_clone)
                .ok()?
                .to_string_lossy()
                .to_string();

            Some(extractor.extract(&code, &rel_path))
        });
    }

    let mut parsed_files = Vec::new();
    while let Some(result) = join_set.join_next().await {
        if let Ok(Some(parsed)) = result {
            parsed_files.push(parsed);
        }
    }

    parsed_files
}

/// Resolve file-level modules (e.g., Rust `mod` declarations, Go packages).
///
/// STUB: Calls `resolve_file_modules()` on each extractor, grouped by language.
fn resolve_file_modules(parsed_files: &mut [ParsedFile]) {
    use std::collections::HashMap;

    // Group files by language
    let mut by_language: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, pf) in parsed_files.iter().enumerate() {
        by_language
            .entry(pf.language.clone())
            .or_default()
            .push(idx);
    }

    // Call each language's resolve_file_modules
    for (lang, indices) in by_language {
        if let Some(extractor) = extract::Extractor::for_language(&lang) {
            // Collect mutable references for this language
            let mut lang_files: Vec<ParsedFile> = indices
                .iter()
                .map(|&idx| parsed_files[idx].clone())
                .collect();

            // Call language-specific resolver
            extractor.resolve_file_modules(&mut lang_files);

            // Write back
            for (i, &idx) in indices.iter().enumerate() {
                parsed_files[idx] = lang_files[i].clone();
            }
        }
    }
}

/// Build the symbol registry from all parsed files.
fn build_registry(parsed_files: &[ParsedFile]) -> SymbolRegistry {
    SymbolRegistry::build(parsed_files)
}

/// Resolve all edges by looking up SymbolRefs in the registry.
///
/// Returns: (resolved_edges, dropped_count)
/// STUB: Since extractors return empty edges, this will return (vec![], 0).
fn resolve_edges(
    parsed_files: &[ParsedFile],
    registry: &SymbolRegistry,
) -> (Vec<ResolvedEdge>, usize) {
    let mut resolved = Vec::new();
    let mut dropped = 0;

    for parsed in parsed_files {
        for edge in &parsed.edges {
            let from_id = match &edge.from {
                SymbolRef::Resolved(id) => Some(id.clone()),
                SymbolRef::Unresolved { name, file_path } => registry.resolve(
                    &SymbolRef::Unresolved {
                        name: name.clone(),
                        file_path: file_path.clone(),
                    },
                    file_path,
                ),
            };

            let to_id = match &edge.to {
                SymbolRef::Resolved(id) => Some(id.clone()),
                SymbolRef::Unresolved { name, file_path } => registry.resolve(
                    &SymbolRef::Unresolved {
                        name: name.clone(),
                        file_path: file_path.clone(),
                    },
                    file_path,
                ),
            };

            match (from_id, to_id) {
                (Some(from), Some(to)) => {
                    resolved.push(ResolvedEdge {
                        from,
                        to,
                        kind: edge.kind.clone(),
                        line: edge.line,
                    });
                }
                _ => {
                    dropped += 1;
                }
            }
        }
    }

    (resolved, dropped)
}

/// Resolve imports to target symbols.
fn resolve_imports(parsed_files: &[ParsedFile], registry: &SymbolRegistry) -> Vec<ResolvedImport> {
    let mut all_resolved = Vec::new();

    for parsed in parsed_files {
        if let Some(extractor) = extract::Extractor::for_language(&parsed.language) {
            let resolved = extractor.resolve_imports(&parsed.imports, registry);
            all_resolved.extend(resolved);
        }
    }

    debug!("Resolved {} imports", all_resolved.len());
    all_resolved
}

/// Load all nodes and edges into the CodeGraph buffer and commit.
fn load_and_commit<C: super::store::NanoGraphCli>(
    graph: &mut super::store::CodeGraph<C>,
    parsed_files: &[ParsedFile],
    resolved_edges: &[ResolvedEdge],
    import_edges: &[ResolvedImport],
    commit_hash: &str,
    progress_tx: &Option<tokio::sync::mpsc::Sender<PipelineProgress>>,
) -> Result<(), A6sError> {
    // Insert File nodes
    for parsed in parsed_files {
        graph.insert_file(&parsed.file_path, &parsed.language, commit_hash);
    }

    // Insert Symbol nodes + Contains edges
    for parsed in parsed_files {
        let file_id = FileId::new(&parsed.file_path);
        for symbol in &parsed.symbols {
            graph.insert_symbol(symbol);
            let symbol_id = symbol.symbol_id();
            graph.insert_contains(&file_id, &symbol_id);
        }
    }

    // Insert resolved edges
    for edge in resolved_edges {
        match edge.kind {
            EdgeKind::Calls => {
                graph.insert_calls_edge(&edge.from, &edge.to, edge.line);
            }
            EdgeKind::Implements => {
                graph.insert_implements_edge(&edge.from, &edge.to);
            }
            EdgeKind::Extends => {
                graph.insert_extends_edge(&edge.from, &edge.to);
            }
            EdgeKind::HasField => {
                graph.insert_has_field_edge(&edge.from, &edge.to);
            }
            EdgeKind::HasMethod => {
                graph.insert_has_method_edge(&edge.from, &edge.to);
            }
            EdgeKind::HasMember => {
                graph.insert_has_member_edge(&edge.from, &edge.to);
            }
            _ => {
                graph.insert_references_edge(&edge.from, &edge.to, &edge.kind, edge.line);
            }
        }
    }

    // Insert import edges
    for import in import_edges {
        graph.insert_file_imports_edge(&import.file_id, &import.target_symbol_id);
    }

    info!("✓ Loaded {} JSONL lines into buffer", graph.buffer_len());

    if let Some(tx) = progress_tx {
        let _ = tx.try_send(PipelineProgress::Loaded);
    }

    // Commit
    info!("Phase 8: Committing to graph...");
    graph.commit()?;

    Ok(())
}
