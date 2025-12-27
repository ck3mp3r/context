//! Note tool implementations
//!
//! Handles all MCP tools for note management operations.
//! Follows Single Responsibility Principle (SRP).

use crate::db::Database;
use std::sync::Arc;

/// Note management tools
///
/// Generic over `D: Database` for zero-cost abstraction.
///
/// # SOLID Principles
/// - **Single Responsibility**: Only handles note operations
/// - **Dependency Inversion**: Depends on Database trait
#[derive(Clone)]
pub struct NoteTools<D: Database> {
    db: Arc<D>,
}

impl<D: Database> NoteTools<D> {
    /// Create new NoteTools with database
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }
}
