// Code Analysis Module
//
// This module provides code analysis capabilities using:
// - Tree-sitter for parsing source code
// - NanoGraph for storing and querying code graphs
//
// Design:
// - Simple schema: File + Symbol nodes with kind discriminator
// - Language-agnostic: Same graph model for all languages
// - Unified CodeParser: parses once, inserts directly into graph

#[cfg(feature = "backend")]
pub mod store;

#[cfg(feature = "backend")]
pub mod types;

#[cfg(feature = "backend")]
pub mod lang;

#[cfg(feature = "backend")]
pub mod parser;

#[cfg(feature = "backend")]
pub mod service;

// Re-exports for convenience
#[cfg(feature = "backend")]
pub use store::CodeGraph;

#[cfg(feature = "backend")]
pub use parser::{Language, Parser};

#[cfg(feature = "backend")]
pub use lang::rust::Rust;

/// Get the analysis directory path for a repository
///
/// Uses the XDG-compliant data directory from sync::paths
#[cfg(feature = "backend")]
pub fn get_analysis_path(repo_id: &str) -> std::path::PathBuf {
    crate::sync::get_data_dir().join("repos").join(repo_id)
}

// Tests
#[cfg(all(test, feature = "backend"))]
mod store_test;

#[cfg(all(test, feature = "backend"))]
mod integration_test;
