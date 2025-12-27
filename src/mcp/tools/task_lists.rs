//! Task list tool implementations
//!
//! Handles all MCP tools for task list management operations.
//! Follows Single Responsibility Principle (SRP).

use crate::db::Database;
use std::sync::Arc;

/// Task list management tools
///
/// Generic over `D: Database` for zero-cost abstraction.
///
/// # SOLID Principles
/// - **Single Responsibility**: Only handles task list operations
/// - **Dependency Inversion**: Depends on Database trait
#[derive(Clone)]
pub struct TaskListTools<D: Database> {
    db: Arc<D>,
}

impl<D: Database> TaskListTools<D> {
    /// Create new TaskListTools with database
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }
}
