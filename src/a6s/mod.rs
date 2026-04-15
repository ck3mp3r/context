//! a6s - Analysis rewrite with parallel extraction
//!
//! This module is a complete rewrite of the code analysis pipeline.
//! It will eventually replace `src/analysis/` once proven correct.
//!
//! Architecture:
//! - Layer 1: Parallel per-file tree-sitter extraction via spawn_blocking
//! - Layer 2: Per-language cross-file resolution (edges + imports)
//! - Buffered graph writes (single nanograph load at commit)

#[cfg(feature = "backend")]
pub mod types;

#[cfg(feature = "backend")]
pub mod extract;

#[cfg(feature = "backend")]
pub mod store;

#[cfg(feature = "backend")]
pub mod pipeline;

#[cfg(feature = "backend")]
pub mod lang;

#[cfg(feature = "backend")]
pub mod queries;

#[cfg(feature = "backend")]
pub mod error;

// Public API
#[cfg(feature = "backend")]
pub use pipeline::analyze;

#[cfg(test)]
mod types_test;

#[cfg(test)]
mod store_test;

#[cfg(test)]
mod pipeline_test;

#[cfg(test)]
mod schema_test;

#[cfg(test)]
mod queries_test;

#[cfg(test)]
mod truncate_test;
