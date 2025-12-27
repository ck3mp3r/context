//! Repository tool implementations
//!
//! Handles all MCP tools for repository management operations.
//! Follows Single Responsibility Principle (SRP).

use crate::db::Database;
use std::sync::Arc;

/// Repository management tools
///
/// Generic over `D: Database` for zero-cost abstraction.
///
/// # SOLID Principles
/// - **Single Responsibility**: Only handles repository operations
/// - **Dependency Inversion**: Depends on Database trait
#[derive(Clone)]
pub struct RepoTools<D: Database> {
    db: Arc<D>,
}

impl<D: Database> RepoTools<D> {
    /// Create new RepoTools with database
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }
}
