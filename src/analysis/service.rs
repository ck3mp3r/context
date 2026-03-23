//! Code analysis service
//!
//! High-level service for analyzing repositories.
//! Uses LanguageRegistry to get appropriate extractors per file.

use crate::analysis::{parser::LanguageRegistry, store::CodeGraph};
use serde::{Deserialize, Serialize};
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

    #[error("Git error: {0}")]
    GitError(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct AnalysisMetadata {
    last_analyzed_commit: Option<String>,
    analyzed_at: String,
}

pub struct AnalysisResult {
    pub files_analyzed: usize,
    pub symbols_extracted: usize,
    pub relationships_created: usize,
}

/// Analyze a repository and store results in NanoGraph
///
/// Uses LanguageRegistry to get the right extractor for each file type
pub async fn analyze_repository(
    repo_path: &Path,
    repo_id: &str,
    graph_path: &Path,
) -> Result<AnalysisResult, AnalysisError> {
    analyze_repository_with_progress(repo_path, repo_id, graph_path, |_, _| {}).await
}

/// Analyze a repository with progress reporting and incremental support
///
/// Uses git-based incremental analysis:
/// - First scan: analyzes all files, stores commit SHA
/// - Subsequent scans: only analyzes changed files since last commit
///
/// # Arguments
/// * `repo_path` - Path to repository
/// * `repo_id` - Repository ID for graph
/// * `graph_path` - Path to store analysis data
/// * `progress_fn` - Callback for progress updates (processed_count, total_count)
pub async fn analyze_repository_with_progress<F>(
    repo_path: &Path,
    repo_id: &str,
    graph_path: &Path,
    progress_fn: F,
) -> Result<AnalysisResult, AnalysisError>
where
    F: Fn(usize, usize) + Send + Sync,
{
    tracing::info!("Creating CodeGraph for repo_id: {}", repo_id);
    let mut graph = CodeGraph::new(graph_path, repo_id).await?;
    tracing::info!("CodeGraph created successfully");

    let registry = LanguageRegistry::new();

    // Load metadata to check for incremental analysis
    let metadata_path = graph_path.join("metadata.json");
    tracing::debug!("Getting current commit for {:?}", repo_path);
    let current_commit = get_current_commit(repo_path)?;
    tracing::debug!("Current commit: {}", current_commit);

    let last_commit = load_metadata(&metadata_path)?;
    tracing::debug!("Last commit: {:?}", last_commit);

    // Determine which files to analyze
    tracing::info!("Scanning for files to analyze");
    let files_to_analyze = if let Some(ref last) = last_commit {
        // Incremental: only changed files since last commit
        tracing::info!("Incremental analysis: finding changed files since {}", last);
        get_changed_files(repo_path, last, &current_commit, &registry)?
    } else {
        // Full scan: all supported files
        tracing::info!("Full scan: finding all supported files");
        scan_supported_files(repo_path, &registry)?
    };

    let total_files = files_to_analyze.len();
    tracing::info!("Found {} files to analyze", total_files);
    let mut total_symbols = 0;
    let mut total_relationships = 0;

    // PASS 1: Extract and insert all symbols, build index
    tracing::info!("Pass 1: Extracting and inserting symbols...");
    let mut symbol_index_data = Vec::new();

    for (batch_idx, batch) in files_to_analyze.chunks(50).enumerate() {
        for file_path in batch {
            let content = std::fs::read_to_string(file_path)?;
            let relative_path = file_path
                .strip_prefix(repo_path)
                .unwrap_or(file_path)
                .to_string_lossy()
                .to_string();

            let extractor = registry
                .get_extractor_for_file(&relative_path, &content)
                .ok_or_else(|| AnalysisError::UnsupportedFile(relative_path.clone()))?;

            let symbols = extractor.extract_symbols(&content, &relative_path);
            total_symbols += symbols.len();

            let file_id = graph.insert_file(&relative_path, "rust", "hash").await?;

            for symbol in &symbols {
                let symbol_id = graph.insert_symbol(symbol).await?;
                graph.insert_contains(&file_id, &symbol_id, 1.0).await?;
                total_relationships += 1;

                // Store for building index
                symbol_index_data.push((symbol.clone(), symbol_id));
            }
        }

        let processed = ((batch_idx + 1) * 50).min(total_files);
        progress_fn(processed, total_files);
        tokio::task::yield_now().await;
    }

    // Build symbol index for relationship resolution
    tracing::info!(
        "Building symbol index from {} symbols...",
        symbol_index_data.len()
    );
    let symbol_index = crate::analysis::resolver::SymbolIndex::build(&symbol_index_data);

    // PASS 2: Extract relationships and resolve symbol IDs
    tracing::info!("Pass 2: Extracting and resolving relationships...");
    let mut resolved_count = 0;
    let mut skipped_count = 0;

    for file_path in &files_to_analyze {
        let content = std::fs::read_to_string(file_path)?;
        let relative_path = file_path
            .strip_prefix(repo_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let extractor = registry
            .get_extractor_for_file(&relative_path, &content)
            .ok_or_else(|| AnalysisError::UnsupportedFile(relative_path.clone()))?;

        let relationships = extractor.extract_relationships(&content, &relative_path);

        for relationship in relationships {
            // Parse placeholder IDs: "symbol:file:name:?"
            let from_name = extract_name_from_placeholder(&relationship.from_symbol_id);
            let to_name = extract_name_from_placeholder(&relationship.to_symbol_id);

            if let (Some(from), Some(to)) = (from_name, to_name) {
                // Resolve using index
                if let (Some(from_id), Some(to_id)) = (
                    symbol_index.resolve_best(&from, Some(&relative_path)),
                    symbol_index.resolve_best(&to, Some(&relative_path)),
                ) {
                    let mut resolved_rel = relationship.clone();
                    resolved_rel.from_symbol_id = from_id;
                    resolved_rel.to_symbol_id = to_id;

                    graph.insert_relationship(&resolved_rel).await?;
                    total_relationships += 1;
                    resolved_count += 1;
                } else {
                    skipped_count += 1;
                }
            }
        }
    }

    tracing::info!(
        "Resolved {} relationships, skipped {} (unresolved symbols)",
        resolved_count,
        skipped_count
    );

    // Commit all batched data to nanograph
    tracing::info!("Committing all data to nanograph...");
    graph.commit().await?;
    tracing::info!("Commit complete");

    // Save metadata with current commit
    save_metadata(&metadata_path, &current_commit)?;

    Ok(AnalysisResult {
        files_analyzed: total_files,
        symbols_extracted: total_symbols,
        relationships_created: total_relationships,
    })
}

/// Scan directory for supported files (respects .gitignore)
fn scan_supported_files(
    repo_path: &Path,
    registry: &LanguageRegistry,
) -> Result<Vec<PathBuf>, AnalysisError> {
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
                    && let Some(ext) = path.extension()
                    && registry.supports_extension(ext.to_str().unwrap_or(""))
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

/// Get current git commit SHA (short)
fn get_current_commit(repo_path: &Path) -> Result<String, AnalysisError> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| AnalysisError::GitError(format!("Failed to run git: {}", e)))?;

    if !output.status.success() {
        // Not a git repo or no commits yet - return empty string
        return Ok(String::new());
    }

    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(commit)
}

/// Get list of changed files between two commits
fn get_changed_files(
    repo_path: &Path,
    from_commit: &str,
    to_commit: &str,
    registry: &LanguageRegistry,
) -> Result<Vec<PathBuf>, AnalysisError> {
    if to_commit.is_empty() {
        // Not a git repo - do full scan
        return scan_supported_files(repo_path, registry);
    }

    let output = std::process::Command::new("git")
        .args([
            "diff",
            "--name-status",
            &format!("{}..{}", from_commit, to_commit),
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| AnalysisError::GitError(format!("Failed to run git diff: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AnalysisError::GitError(format!(
            "git diff failed: {}",
            stderr
        )));
    }

    let diff_output = String::from_utf8_lossy(&output.stdout);
    let mut changed_files = Vec::new();

    for line in diff_output.lines() {
        if line.is_empty() {
            continue;
        }

        // Parse git diff --name-status output: "M\tfile.rs" or "A\tfile.rs" or "D\tfile.rs"
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }

        let status = parts[0];
        let file_path = parts[1];

        // Skip deleted files - we'll handle them separately
        if status == "D" {
            // TODO: Remove from graph
            continue;
        }

        // Check if file extension is supported
        let path = repo_path.join(file_path);
        if let Some(ext) = path.extension()
            && registry.supports_extension(ext.to_str().unwrap_or(""))
        {
            changed_files.push(path);
        }
    }

    Ok(changed_files)
}

/// Load metadata from file
fn load_metadata(metadata_path: &Path) -> Result<Option<String>, AnalysisError> {
    if !metadata_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(metadata_path)?;
    let metadata: AnalysisMetadata = serde_json::from_str(&content)
        .map_err(|e| AnalysisError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

    Ok(metadata.last_analyzed_commit)
}

/// Save metadata to file
fn save_metadata(metadata_path: &Path, commit_sha: &str) -> Result<(), AnalysisError> {
    if commit_sha.is_empty() {
        // Not a git repo - don't save metadata
        return Ok(());
    }

    let metadata = AnalysisMetadata {
        last_analyzed_commit: Some(commit_sha.to_string()),
        analyzed_at: chrono::Utc::now().to_rfc3339(),
    };

    let content = serde_json::to_string_pretty(&metadata)
        .map_err(|e| AnalysisError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

    // Ensure directory exists
    if let Some(parent) = metadata_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(metadata_path, content)?;
    Ok(())
}

/// Extract symbol name from placeholder ID
/// Format: "symbol:{file_path}:{name}:?" → returns name
fn extract_name_from_placeholder(placeholder_id: &str) -> Option<String> {
    let parts: Vec<&str> = placeholder_id.split(':').collect();
    if parts.len() >= 3 && parts[0] == "symbol" {
        // Handle "impl Foo" style names
        let name_parts = &parts[2..parts.len() - 1];
        Some(name_parts.join(":"))
    } else {
        None
    }
}

#[cfg(test)]
#[path = "service_test.rs"]
mod service_test;
