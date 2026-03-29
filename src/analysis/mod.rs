//! Code Analysis Module
//!
//! Multi-phase pipeline for analyzing source code into a knowledge graph:
//!
//! 1. **Extract** — Tree-sitter queries extract raw data (symbols, calls,
//!    imports, heritage) from each file into `ParsedFile` structs
//! 2. **Register** — Build a global symbol map from all extracted symbols
//! 3. **Resolve imports** — Map import statements to graph symbols
//! 4. **Resolve heritage** — Resolve inheritance/implementation edges
//! 5. **Resolve calls** — Resolve function/method calls using import
//!    tables and symbol map
//! 6. **Load** — Batch insert all resolved data into NanoGraph

#[cfg(feature = "backend")]
pub mod types;

#[cfg(feature = "backend")]
pub mod store;

#[cfg(feature = "backend")]
pub mod lang;

#[cfg(feature = "backend")]
pub mod pipeline;

#[cfg(feature = "backend")]
pub mod service;

#[cfg(feature = "backend")]
pub use store::CodeGraph;

/// Get the analysis directory path for a repository
#[cfg(feature = "backend")]
pub fn get_analysis_path(repo_id: &str) -> std::path::PathBuf {
    crate::sync::get_data_dir().join("repos").join(repo_id)
}

// Tests
