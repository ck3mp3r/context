//! Code analysis service
//!
//! High-level service for analyzing repositories.
//! Follows SOLID principles - depends on abstractions, not implementations.

use crate::analysis::{extractor::SymbolExtractor, parser::LanguageRegistry, store::CodeGraph};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Store error: {0}")]
    Store(#[from] crate::analysis::store::StoreError),

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
///
/// # SOLID Principles
/// - **Dependency Inversion**: Depends on SymbolExtractor trait
/// - **Single Responsibility**: Only coordinates analysis workflow
pub async fn analyze_repository<E: SymbolExtractor>(
    repo_path: &Path,
    repo_id: &str,
    graph_path: &Path,
    extractor: &E,
) -> Result<AnalysisResult, AnalysisError> {
    let mut graph = CodeGraph::new(graph_path, repo_id).await?;

    let registry = LanguageRegistry::new();
    let files = scan_supported_files(repo_path, &registry)?;

    let mut total_symbols = 0;
    let mut total_relationships = 0;

    for file_path in &files {
        let content = std::fs::read_to_string(file_path)?;

        let relative_path = file_path
            .strip_prefix(repo_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // Extract symbols using the provided extractor
        let symbols = extractor.extract_symbols(&content, &relative_path);
        total_symbols += symbols.len();

        // Insert into graph
        let file_id = graph.insert_file(&relative_path, "rust", "hash").await?;

        for symbol in &symbols {
            let symbol_id = graph.insert_symbol(symbol).await?;
            graph.insert_contains(&file_id, &symbol_id, 1.0).await?;
            total_relationships += 1;
        }
    }

    Ok(AnalysisResult {
        files_analyzed: files.len(),
        symbols_extracted: total_symbols,
        relationships_created: total_relationships,
    })
}

/// Scan directory for supported files
fn scan_supported_files(
    repo_path: &Path,
    registry: &LanguageRegistry,
) -> Result<Vec<PathBuf>, AnalysisError> {
    let mut supported_files = Vec::new();

    fn visit_dirs(
        dir: &Path,
        registry: &LanguageRegistry,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), AnalysisError> {
        if !dir.is_dir() {
            return Ok(());
        }

        // Skip common directories
        if let Some(name) = dir.file_name() {
            let name = name.to_string_lossy();
            if name == "target" || name == ".git" || name == "node_modules" {
                return Ok(());
            }
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                visit_dirs(&path, registry, files)?;
            } else if let Some(ext) = path.extension()
                && registry.supports_extension(ext.to_str().unwrap_or(""))
            {
                files.push(path);
            }
        }

        Ok(())
    }

    visit_dirs(repo_path, registry, &mut supported_files)?;
    Ok(supported_files)
}
