//! Code analysis service
//!
//! High-level service for analyzing repositories.
//! Uses generic Parser with Language trait.

use crate::analysis::{Language, Parser, Rust, store::CodeGraph};
use serde::{Deserialize, Serialize};
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
        $callback!($file, Rust)
        // Add more: || $callback!($file, TypeScript)
        // Add more: || $callback!($file, Python)
    };

    ($callback:ident!($files:expr, $repo:expr, $graph:expr, $syms:expr, $rels:expr)) => {
        $callback!($files, $repo, $graph, $syms, $rels, Rust)?;
        // Add more: $callback!(..., TypeScript)?;
        // Add more: $callback!(..., Python)?;
    };
}

macro_rules! can_handle {
    ($file:expr, $Lang:ty) => {
        Parser::<$Lang>::can_handle($file)
    };
}

macro_rules! analyze {
    ($files:expr, $repo:expr, $graph:expr, $syms:expr, $rels:expr, $Lang:ty) => {{
        let lang_files: Vec<PathBuf> = $files
            .iter()
            .filter(|f| Parser::<$Lang>::can_handle(f.to_str().unwrap_or("")))
            .cloned()
            .collect();

        if !lang_files.is_empty() {
            let (symbols, rels) = analyze_files::<$Lang>(&lang_files, $repo, $graph)?;
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
pub async fn analyze_repository(
    repo_path: &Path,
    repo_id: &str,
    graph_path: &Path,
) -> Result<AnalysisResult, AnalysisError> {
    analyze_repository_with_progress(repo_path, repo_id, graph_path, |_, _| {}).await
}

/// Analyze files with a specific language parser
fn analyze_files<L: Language>(
    files: &[PathBuf],
    repo_path: &Path,
    graph: &mut CodeGraph,
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

        let stats = parser.parse_and_analyze(&content, &relative_path, graph)?;

        total_symbols += stats.symbols_inserted;
        total_relationships += stats.relationships_inserted;
    }

    Ok((total_symbols, total_relationships))
}

/// Analyze a repository with progress reporting and incremental support
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
    let mut graph = CodeGraph::new(graph_path, repo_id)?;
    tracing::info!("CodeGraph created successfully");

    // Load metadata
    let metadata_path = graph_path.join("metadata.json");
    tracing::debug!("Getting current commit for {:?}", repo_path);
    let current_commit = get_current_commit(repo_path)?;
    let last_commit = load_metadata(&metadata_path)?;

    // Scan for files
    tracing::info!("Scanning for files to analyze");
    let all_files = if let Some(ref last) = last_commit {
        tracing::info!("Incremental analysis: finding changed files since {}", last);
        get_changed_files(repo_path, last, &current_commit)?
    } else {
        tracing::info!("Full scan: finding all supported files");
        scan_supported_files(repo_path)?
    };

    let total_files = all_files.len();
    tracing::info!("Found {} files to analyze", total_files);

    // Analyze all languages using central registry
    let mut total_symbols = 0;
    let mut total_relationships = 0;

    languages!(analyze!(
        all_files,
        repo_path,
        &mut graph,
        total_symbols,
        total_relationships
    ));

    progress_fn(total_files, total_files);

    // Commit
    tracing::info!("Committing all data to nanograph...");
    graph.commit()?;

    // Save metadata
    save_metadata(&metadata_path, &current_commit)?;

    Ok(AnalysisResult {
        files_analyzed: total_files,
        symbols_extracted: total_symbols,
        relationships_created: total_relationships,
    })
}

/// Macro to check if any registered language can handle a file
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
                {
                    // Check if any parser can handle this file
                    if languages!(can_handle!(file_str)) {
                        tracing::trace!("Found supported file: {:?}", path);
                        supported_files.push(path.to_path_buf());
                    }
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
) -> Result<Vec<PathBuf>, AnalysisError> {
    if to_commit.is_empty() {
        // Not a git repo - do full scan
        return scan_supported_files(repo_path);
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

        // Check if file is supported
        let path = repo_path.join(file_path);
        if let Some(file_str) = path.to_str()
            && languages!(can_handle!(file_str))
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

#[cfg(test)]
#[path = "service_test.rs"]
mod service_test;
