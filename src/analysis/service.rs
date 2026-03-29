//! Code analysis service
//!
//! High-level service for analyzing repositories.
//! Always performs clean-slate analysis: deletes existing graph,
//! re-scans all files, rebuilds from scratch. This ensures no
//! stale data accumulates over time.
//!
//! Saved queries live at `{graph_path}/queries/` (adjacent to, not
//! inside `analysis.nano`), so deleting the graph is safe.

use crate::analysis::parser::{GlobalSymbolMap, resolve_deferred_edges};
use crate::analysis::{Go, Language, Nushell, Parser, Rust, store::CodeGraph};
use std::path::{Path, PathBuf};
use thiserror::Error;

// ============================================================================
// CENTRAL LANGUAGE REGISTRY - Add new languages here ONLY
// ============================================================================
// To add a new language:
// 1. Create src/analysis/lang/<language>/
// 2. Implement Language trait
// 3. Add to for_each_language macro below
// 4. Import at top of file

macro_rules! languages {
    ($callback:ident!($file:expr)) => {
        $callback!($file, Rust) || $callback!($file, Nushell) || $callback!($file, Go)
    };

    ($callback:ident!($files:expr, $repo:expr, $graph:expr, $global:expr, $syms:expr, $rels:expr)) => {
        $callback!($files, $repo, $graph, $global, $syms, $rels, Rust)?;
        $callback!($files, $repo, $graph, $global, $syms, $rels, Nushell)?;
        $callback!($files, $repo, $graph, $global, $syms, $rels, Go)?;
    };
}

macro_rules! can_handle {
    ($file:expr, $Lang:ty) => {
        Parser::<$Lang>::can_handle($file)
    };
}

macro_rules! analyze {
    ($files:expr, $repo:expr, $graph:expr, $global:expr, $syms:expr, $rels:expr, $Lang:ty) => {{
        let lang_files: Vec<PathBuf> = $files
            .iter()
            .filter(|f| Parser::<$Lang>::can_handle(f.to_str().unwrap_or("")))
            .cloned()
            .collect();

        if !lang_files.is_empty() {
            let (symbols, rels) = analyze_files::<$Lang>(&lang_files, $repo, $graph, $global)?;
            $syms += symbols;
            $rels += rels;
        }
        Ok::<(), AnalysisError>(())
    }};
}

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Store error: {0}")]
    Store(#[from] crate::analysis::store::StoreError),

    #[error("Parse error: {0}")]
    Parse(#[from] crate::analysis::parser::ParseError),

    #[error("Repository has no local path")]
    NoLocalPath,

    #[error("Unsupported file: {0}")]
    UnsupportedFile(String),
}

pub struct AnalysisResult {
    pub files_analyzed: usize,
    pub symbols_extracted: usize,
    pub relationships_created: usize,
}

/// Analyze a repository and store results in NanoGraph
pub async fn analyze_repository(
    repo_path: &Path,
    repo_id: &str,
    graph_path: &Path,
) -> Result<AnalysisResult, AnalysisError> {
    analyze_repository_with_progress(repo_path, repo_id, graph_path, |_, _| {}).await
}

/// Analyze files with a specific language parser, collecting deferred edges
fn analyze_files<L: Language>(
    files: &[PathBuf],
    repo_path: &Path,
    graph: &mut CodeGraph,
    global: &mut GlobalSymbolMap,
) -> Result<(usize, usize), AnalysisError> {
    let mut parser = Parser::<L>::new();
    let mut total_symbols = 0;
    let mut total_relationships = 0;

    for file_path in files {
        let content = std::fs::read_to_string(file_path)?;
        let relative_path = file_path
            .strip_prefix(repo_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let stats = parser.parse_and_collect(&content, &relative_path, graph, global)?;

        total_symbols += stats.symbols_inserted;
        total_relationships += stats.relationships_inserted;
    }

    Ok((total_symbols, total_relationships))
}

/// Analyze a repository with progress reporting.
/// Always performs a clean-slate analysis: removes any existing graph
/// data before re-scanning all files.
pub async fn analyze_repository_with_progress<F>(
    repo_path: &Path,
    repo_id: &str,
    graph_path: &Path,
    progress_fn: F,
) -> Result<AnalysisResult, AnalysisError>
where
    F: Fn(usize, usize) + Send + Sync,
{
    // Clean slate: delete existing graph data
    let analysis_path = graph_path.join("analysis.nano");
    if analysis_path.exists() {
        tracing::info!("Removing existing graph at {:?}", analysis_path);
        std::fs::remove_dir_all(&analysis_path)?;
    }

    tracing::info!("Creating CodeGraph for repo_id: {}", repo_id);
    let mut graph = CodeGraph::new(graph_path, repo_id)?;

    // Full scan - always process all files
    tracing::info!("Scanning for files to analyze");
    let all_files = scan_supported_files(repo_path)?;

    let total_files = all_files.len();
    tracing::info!("Found {} files to analyze", total_files);

    // Analyze all languages, sharing a global symbol map
    let mut total_symbols = 0;
    let mut total_relationships = 0;
    let mut global = GlobalSymbolMap::new();

    languages!(analyze!(
        all_files,
        repo_path,
        &mut graph,
        &mut global,
        total_symbols,
        total_relationships
    ));

    // Resolve cross-file relationships
    let resolved = resolve_deferred_edges(&global, &mut graph).map_err(AnalysisError::Parse)?;
    total_relationships += resolved;

    tracing::info!(
        "Cross-file resolution: {} edges resolved, {} deferred total",
        resolved,
        global.deferred.len()
    );

    progress_fn(total_files, total_files);

    // Commit
    tracing::info!("Committing all data to nanograph...");
    graph.commit()?;

    Ok(AnalysisResult {
        files_analyzed: total_files,
        symbols_extracted: total_symbols,
        relationships_created: total_relationships,
    })
}

/// Scan directory for supported files (respects .gitignore)
fn scan_supported_files(repo_path: &Path) -> Result<Vec<PathBuf>, AnalysisError> {
    tracing::debug!("Starting scan of {:?}", repo_path);
    let mut supported_files = Vec::new();

    // Use the ignore crate to respect .gitignore
    let walker = ignore::WalkBuilder::new(repo_path)
        .hidden(true) // Skip hidden files/dirs
        .git_ignore(true) // Respect .gitignore
        .git_exclude(true) // Respect .git/info/exclude
        .build();

    for entry in walker {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file()
                    && let Some(file_str) = path.to_str()
                    && languages!(can_handle!(file_str))
                {
                    tracing::trace!("Found supported file: {:?}", path);
                    supported_files.push(path.to_path_buf());
                }
            }
            Err(e) => {
                tracing::warn!("Error walking directory: {}", e);
            }
        }
    }

    tracing::info!("Scan complete: found {} files", supported_files.len());
    Ok(supported_files)
}

#[cfg(test)]
#[path = "service_test.rs"]
mod service_test;
