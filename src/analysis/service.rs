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
    let mut graph = CodeGraph::new(graph_path, repo_id).await?;
    let registry = LanguageRegistry::new();

    // Load metadata to check for incremental analysis
    let metadata_path = graph_path.join("metadata.json");
    let current_commit = get_current_commit(repo_path)?;
    let last_commit = load_metadata(&metadata_path)?;

    // Determine which files to analyze
    let files_to_analyze = if let Some(ref last) = last_commit {
        // Incremental: only changed files since last commit
        get_changed_files(repo_path, last, &current_commit, &registry)?
    } else {
        // Full scan: all supported files
        scan_supported_files(repo_path, &registry)?
    };

    let total_files = files_to_analyze.len();
    let mut total_symbols = 0;
    let mut total_relationships = 0;

    // Process files in batches of 50
    const BATCH_SIZE: usize = 50;

    for (batch_idx, batch) in files_to_analyze.chunks(BATCH_SIZE).enumerate() {
        for file_path in batch {
            let content = std::fs::read_to_string(file_path)?;

            let relative_path = file_path
                .strip_prefix(repo_path)
                .unwrap_or(file_path)
                .to_string_lossy()
                .to_string();

            // Detect language from content using Tree-sitter
            let extractor = registry
                .get_extractor_for_file(&relative_path, &content)
                .ok_or_else(|| AnalysisError::UnsupportedFile(relative_path.clone()))?;

            // Extract symbols using the detected language's extractor
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

        // Report progress after each batch
        let processed = ((batch_idx + 1) * BATCH_SIZE).min(total_files);
        progress_fn(processed, total_files);

        // Yield to runtime to allow other tasks to run
        tokio::task::yield_now().await;
    }

    // Save metadata with current commit
    save_metadata(&metadata_path, &current_commit)?;

    Ok(AnalysisResult {
        files_analyzed: total_files,
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

/// Get current git commit SHA (short)
fn get_current_commit(repo_path: &Path) -> Result<String, AnalysisError> {
    let output = std::process::Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
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
        .args(&[
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
        if let Some(ext) = path.extension() {
            if registry.supports_extension(ext.to_str().unwrap_or("")) {
                changed_files.push(path);
            }
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

#[cfg(test)]
#[path = "service_test.rs"]
mod service_test;
