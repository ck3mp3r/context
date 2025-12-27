//! Project tool implementations
//!
//! Handles all MCP tools for project management operations.
//! Follows Single Responsibility Principle (SRP).

use crate::db::Database;
use std::sync::Arc;

/// Project management tools
///
/// Generic over `D: Database` for zero-cost abstraction.
///
/// # SOLID Principles
/// - **Single Responsibility**: Only handles project operations
/// - **Dependency Inversion**: Depends on Database trait
#[derive(Clone)]
pub struct ProjectTools<D: Database> {
    db: Arc<D>,
}

impl<D: Database> ProjectTools<D> {
    /// Create new ProjectTools with database
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }
}
