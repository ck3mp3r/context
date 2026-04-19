use super::error::A6sError;
use super::extract;
use super::extract::LanguageExtractor;
use super::store::CodeGraph;
use super::types::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, info};

use super::store::surrealdb;

/// Main entry point for code analysis pipeline (production).
#[cfg(feature = "backend")]
pub async fn analyze(
    repo_path: &Path,
    repo_id: &str,
    commit_hash: &str,
    progress_tx: Option<tokio::sync::mpsc::Sender<PipelineProgress>>,
    analysis_db: Arc<surrealdb::SurrealDbConnection>, // ADD THIS
) -> Result<ResolveStats, A6sError> {
    // Create graph for this repo (truncates existing data safely)
    let graph = CodeGraph::with_connection(repo_id.to_string(), analysis_db).await?;
    analyze_with_graph(repo_path, commit_hash, progress_tx, graph).await
}

/// Main entry point for code analysis pipeline with injectable graph.
///
/// Orchestrates the full analysis flow:
/// 1. Scan repository for supported files
/// 2. Extract symbols/edges in parallel (Layer 1)
/// 3. Resolve file modules (per-language fixups)
/// 4. Per-language cross-file resolution (edges + imports)
/// 5. Load graph and commit
#[cfg(feature = "backend")]
pub async fn analyze_with_graph(
    repo_path: &Path,
    commit_hash: &str,
    progress_tx: Option<tokio::sync::mpsc::Sender<PipelineProgress>>,
    graph: super::store::CodeGraph,
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

    // Phase 3b: Per-language cross-file resolution
    info!("Phase 3b: Per-language cross-file resolution...");
    let (resolved_edges, import_edges) = resolve_cross_file_per_language(&mut parsed_files);
    info!(
        "✓ Resolved {} edges, {} imports",
        resolved_edges.len(),
        import_edges.len()
    );

    // Compute symbol count directly from parsed files
    let symbols_registered: usize = parsed_files.iter().map(|pf| pf.symbols.len()).sum();

    let stats = ResolveStats {
        symbols_registered,
        edges_resolved: resolved_edges.len(),
        edges_dropped: 0,
        imports_resolved: import_edges.len(),
    };

    if let Some(ref tx) = progress_tx {
        let _ = tx.send(PipelineProgress::Resolved(stats.clone())).await;
    }

    // Phase 4: Load graph and commit
    info!("Phase 4: Loading graph buffer...");
    load_and_commit(
        &graph,
        &parsed_files,
        &resolved_edges,
        &import_edges,
        commit_hash,
        &progress_tx,
    )
    .await?;
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

/// Per-language cross-file resolution.
/// Each extractor resolves imports and edges using its own symbol index.
fn resolve_cross_file_per_language(
    parsed_files: &mut [ParsedFile],
) -> (Vec<ResolvedEdge>, Vec<ResolvedImport>) {
    use std::collections::HashMap;

    let mut by_language: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, pf) in parsed_files.iter().enumerate() {
        by_language
            .entry(pf.language.clone())
            .or_default()
            .push(idx);
    }

    let mut all_edges = Vec::new();
    let mut all_imports = Vec::new();

    for (lang, indices) in by_language {
        if let Some(extractor) = extract::Extractor::for_language(&lang) {
            let mut lang_files: Vec<ParsedFile> =
                indices.iter().map(|&i| parsed_files[i].clone()).collect();
            let (edges, imports) = extractor.resolve_cross_file(&mut lang_files);
            // Write back modified files
            for (i, &idx) in indices.iter().enumerate() {
                parsed_files[idx] = lang_files[i].clone();
            }
            all_edges.extend(edges);
            all_imports.extend(imports);
        }
    }

    (all_edges, all_imports)
}

/// Load all nodes and edges into the CodeGraph and commit.
#[cfg(feature = "backend")]
async fn load_and_commit(
    graph: &super::store::CodeGraph,
    parsed_files: &[ParsedFile],
    resolved_edges: &[ResolvedEdge],
    import_edges: &[ResolvedImport],
    commit_hash: &str,
    progress_tx: &Option<tokio::sync::mpsc::Sender<PipelineProgress>>,
) -> Result<(), A6sError> {
    // Batch insert File nodes
    let files: Vec<(&str, &str)> = parsed_files
        .iter()
        .map(|p| (p.file_path.as_str(), p.language.as_str()))
        .collect();
    graph.insert_files_batch(&files, commit_hash).await?;

    // Batch insert Symbol nodes + Contains edges
    let all_symbols: Vec<RawSymbol> = parsed_files
        .iter()
        .flat_map(|p| p.symbols.iter().cloned())
        .collect();
    graph.insert_symbols_batch(&all_symbols).await?;

    // Batch insert resolved edges
    graph.insert_edges_batch(resolved_edges).await?;

    // Batch insert import edges
    graph.insert_imports_batch(import_edges).await?;

    info!("✓ Loaded all nodes and edges into SurrealDB");

    if let Some(tx) = progress_tx {
        let _ = tx.try_send(PipelineProgress::Loaded);
    }

    // Commit
    info!("Phase 5: Committing to graph...");
    graph.commit().await?;

    Ok(())
}
